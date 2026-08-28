// === AI-WORKSHOP START ===
// MCP（Model Context Protocol）客户端：独立子进程 stdio JSON-RPC。
// 默认 enabled: false（见配置）；启动后自动 initialize → tools/list 并注册为原子工具，
// 周期 ping 健康检查，进程崩溃自动重启并刷新工具注册表。
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

use crate::ai_workshop::other_err;
use crate::ai_workshop::tools::context::ExecutionContext;
use crate::ai_workshop::tools::registry::{
    Tool, ToolDomain, ToolInfo, ToolRegistry,
};

/// MCP 呼叫超时（tools/call）。
const CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// initialize / tools/list / ping 通用请求超时。
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// MCP 服务端共享句柄：待响应表 + 请求写入通道。
struct McpShared {
    event_tx: mpsc::Sender<Value>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    next_id: AtomicU64,
}

impl McpShared {
    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

/// 单次请求辅助：写入请求并等待响应（带超时）。
async fn request_once(
    tx: &mpsc::Sender<Value>,
    pending: &Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    next_id: &AtomicU64,
    method: String,
    params: Value,
    timeout: std::time::Duration,
) -> std::result::Result<Value, String> {
    let id = next_id.fetch_add(1, Ordering::Relaxed);
    let (send, recv) = oneshot::channel();
    pending.lock().unwrap().insert(id, send);
    tx.send(json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    }))
    .await
    .map_err(|e| format!("MCP 通道关闭: {e}"))?;
    match tokio::time::timeout(timeout, recv).await {
        Ok(Ok(response)) => response_take_result(&response),
        Ok(Err(_)) => Err("MCP 请求结束但无响应".to_string()),
        Err(_) => Err("MCP 请求超时".to_string()),
    }
}

/// 响应提取：优先 error，否则 result。
fn response_take_result(
    response: &Value,
) -> std::result::Result<Value, String> {
    if let Some(error) = response.get("error") {
        return Err(format!(
            "MCP error: {}",
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| "MCP 响应缺少 result".to_string())
}

/// MCP 动态工具：转发到子进程 tools/call。
pub struct McpTool {
    info: ToolInfo,
    shared: std::sync::Arc<McpShared>,
    is_readonly: bool,
}

impl McpTool {
    fn new(tool: &McpToolSchema, shared: std::sync::Arc<McpShared>) -> Self {
        let schema_json = serde_json::to_value(&tool.input_schema).ok();
        Self {
			info: ToolInfo {
				name: tool.name.clone(),
				description: tool.description.clone().unwrap_or_default(),
				domain: ToolDomain::Mcp,
				requires_confirmation: false,
				is_readonly: !tool.annotations.iter().any(|a| a == "write"),
				params_schema: schema_json.unwrap_or_else(|| {
					json!({ "type": "object", "properties": {}, "required": [] })
				}),
			},
			shared,
			is_readonly: !tool.annotations.iter().any(|a| a == "write"),
		}
    }
}

#[async_trait]
impl Tool for McpTool {
    fn info(&self) -> ToolInfo {
        self.info.clone()
    }

    fn requires_confirmation(&self) -> bool {
        // 未知读写语义时要求确认，避免 AI 直接改写外部系统。
        !self.info.is_readonly
    }

    async fn execute(
        &self,
        arguments: Value,
        _ctx: &ExecutionContext,
    ) -> std::result::Result<Value, String> {
        let result = request_once(
            &self.shared.event_tx,
            &self.shared.pending,
            &self.shared.next_id,
            "tools/call".to_string(),
            json!({ "name": self.info.name, "arguments": arguments }),
            CALL_TIMEOUT,
        )
        .await?;
        // MCP 约定：result.content 为 [{type:"text",text:...}, ...]
        let mut texts = Vec::new();
        if let Some(content) = result.get("content").and_then(Value::as_array) {
            for block in content {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    texts.push(text.to_string());
                }
            }
        }
        if result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(if texts.is_empty() {
                "MCP 工具返回错误".to_string()
            } else {
                texts.join("\n")
            });
        }
        Ok(json!({ "content": texts.join("\n") }))
    }
}

/// tools/list 返回的工具描述（选择所需字段）。
#[derive(Clone, Debug, serde::Deserialize)]
pub struct McpToolSchema {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub annotations: Vec<String>,
}

/// MCP 子进程生命周期管理：maintains 进程、消息循环与工具注册表同步。
pub struct McpClient {
    shared: std::sync::Arc<McpShared>,
    command: String,
    args: Vec<String>,
    interval: std::time::Duration,
    registered: Mutex<Vec<String>>,
}

impl McpClient {
    /// 启动 MCP 客户端：立即 spawn 子进程并进入健康检查循环（默认 enabled: false 时不调用）。
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        command: String,
        args: Vec<String>,
        interval_secs: u64,
        tool_registry: Arc<ToolRegistry>,
    ) -> std::sync::Arc<Self> {
        let (event_tx, event_rx) = mpsc::channel::<Value>(256);
        let shared = std::sync::Arc::new(McpShared {
            event_tx,
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        });
        let this = std::sync::Arc::new(Self {
            shared,
            command,
            args,
            interval: std::time::Duration::from_secs(interval_secs),
            registered: Mutex::new(Vec::new()),
        });
        let weak = std::sync::Arc::downgrade(&this);
        tauri::async_runtime::spawn(async move {
            if let Some(client) = weak.upgrade() {
                client.run_loop(event_rx, tool_registry).await;
            }
        });
        this
    }

    /// 主循环：spawn 进程 → 读写泵 → 健康检查；崩溃/失败整体重启。
    ///
    /// 进程重启时 stdin/stdout 在每轮重新接管：Writer 任务常驻，通过
    /// `stdin_slot` 追踪当前子进程的 stdin，避免跨重启移动 channel。
    async fn run_loop(
        self: std::sync::Arc<Self>,
        event_rx: mpsc::Receiver<Value>,
        tool_registry: Arc<ToolRegistry>,
    ) {
        let stdin_slot = Arc::new(tokio::sync::Mutex::new(
            None::<tokio::process::ChildStdin>,
        ));
        let writer_slot = stdin_slot.clone();
        let _writer_task = tokio::spawn(async move {
            let mut event_rx = event_rx;
            while let Some(message) = event_rx.recv().await {
                let Ok(line) = serde_json::to_string(&message) else {
                    continue;
                };
                let mut line = line;
                line.push('\n');
                let mut guard = writer_slot.lock().await;
                if let Some(stdin) = guard.as_mut() {
                    stdin.write_all(line.as_bytes()).await.ok();
                    stdin.flush().await.ok();
                }
            }
        });

        loop {
            let mut child = match self.spawn_child().await {
                Ok(child) => child,
                Err(e) => {
                    tracing::warn!("mcp: failed to spawn child: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            // 先各自接管 stdout/stdin，子进程句柄仍可用于 kill/wait。
            let stdout = child.stdout.take().expect("piped stdout");
            let stdin = child.stdin.take().expect("piped stdin");

            // 读泵：stdout 行 → 响应路由
            let reader_shared = self.shared.clone();
            let reader_task = tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let Ok(message) = serde_json::from_str::<Value>(&line)
                    else {
                        continue;
                    };
                    let Some(id) = message.get("id").and_then(Value::as_u64)
                    else {
                        continue; // notifications 忽略
                    };
                    let sender =
                        reader_shared.pending.lock().unwrap().remove(&id);
                    if let Some(sender) = sender {
                        let _ = sender.send(message);
                    }
                }
            });

            // 本轮 stdin 注入写泵槽位。
            {
                let mut slot = stdin_slot.lock().await;
                *slot = Some(stdin);
            }

            // 初始化 + 工具发现（注入 event_tx 到调用端）：先补齐握手，失败则重启。
            let initialized = self.initialize().await;
            if let Err(e) = initialized {
                tracing::warn!("mcp: initialize failed: {e}");
                self.clear_stdin_slot(&stdin_slot).await;
                reader_task.abort();
                let _ = child.kill().await;
                let _ = child.wait().await;
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
            self.sync_tools(&tool_registry).await;

            // 健康检查循环：ping 成功则继续；失败或进程退出 → 重启。
            loop {
                tokio::time::sleep(self.interval).await;
                match request_once(
                    &self.shared.event_tx,
                    &self.shared.pending,
                    &self.shared.next_id,
                    "ping".to_string(),
                    json!({}),
                    REQUEST_TIMEOUT,
                )
                .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(
                            "mcp: health check failed: {e}; restarting mcp process"
                        );
                        break;
                    }
                }
            }

            // 重启：先清理已注册工具，等子进程彻底退出。
            self.unregister_tools(&tool_registry);
            self.clear_stdin_slot(&stdin_slot).await;
            let _ = child.kill().await;
            let _ = child.wait().await;
            reader_task.abort();
        }
        // writer 常驻；run_loop 永不退出（应用退出随进程终止）。
    }

    async fn clear_stdin_slot(
        &self,
        slot: &Arc<tokio::sync::Mutex<Option<tokio::process::ChildStdin>>>,
    ) {
        let mut guard = slot.lock().await;
        *guard = None;
    }

    async fn spawn_child(&self) -> std::result::Result<Child, String> {
        let mut command = Command::new(&self.command);
        command
            .args(&self.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());
        command
            .spawn()
            .map_err(|e| format!("MCP 子进程启动失败: {e}"))
    }

    async fn initialize(&self) -> crate::api::Result<Value> {
        let result = request_once(
			&self.shared.event_tx,
			&self.shared.pending,
			&self.shared.next_id,
			"initialize".to_string(),
			json!({
				"protocolVersion": "2024-11-05",
				"capabilities": {},
				"clientInfo": { "name": "packrinth", "version": env!("CARGO_PKG_VERSION") }
			}),
			REQUEST_TIMEOUT,
		)
		.await
		.map_err(other_err)?;
        Ok(result)
    }

    /// tools/list → 与注册表同步（新增/替换，消失的移除）。
    async fn sync_tools(&self, registry: &ToolRegistry) {
        let result = match request_once(
            &self.shared.event_tx,
            &self.shared.pending,
            &self.shared.next_id,
            "tools/list".to_string(),
            json!({}),
            REQUEST_TIMEOUT,
        )
        .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!("mcp: tools/list failed: {e}");
                return;
            }
        };
        let Some(tools) = result.get("tools").and_then(Value::as_array) else {
            tracing::warn!("mcp: tools/list missing tools array");
            return;
        };

        let schemas: Vec<McpToolSchema> = tools
            .iter()
            .filter_map(|tool| serde_json::from_value(tool.clone()).ok())
            .collect();
        let mut names = Vec::new();
        for schema in &schemas {
            names.push(schema.name.clone());
            registry.register(std::sync::Arc::new(McpTool::new(
                schema,
                self.shared.clone(),
            )));
        }
        let mut registered = self.registered.lock().unwrap();
        for old in registered.iter() {
            if !names.contains(old) {
                registry.remove(old);
            }
        }
        tracing::info!("mcp: registered {} tools: {names:?}", schemas.len());
        *registered = names;
    }

    fn unregister_tools(&self, registry: &ToolRegistry) {
        let mut registered = self.registered.lock().unwrap();
        for name in registered.iter() {
            registry.remove(name);
        }
        registered.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_error_is_mapped() {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": { "code": -32601, "message": "Method not found" }
        });
        let err = response_take_result(&response).unwrap_err();
        assert!(err.contains("Method not found"));
    }

    #[test]
    fn response_result_extracted() {
        let response =
            json!({ "jsonrpc": "2.0", "id": 1, "result": { "answer": 42 } });
        assert_eq!(
            response_take_result(&response).unwrap()["answer"],
            json!(42)
        );
    }

    #[test]
    fn missing_result_is_error() {
        let response = json!({ "jsonrpc": "2.0", "id": 1 });
        assert!(response_take_result(&response).is_err());
    }

    #[test]
    fn tool_schema_parse_defaults() {
        let value = json!({
            "name": "hello",
            "inputSchema": { "type": "object", "properties": {} }
        });
        let schema: McpToolSchema = serde_json::from_value(value).unwrap();
        assert_eq!(schema.name, "hello");
        assert_eq!(schema.description, None);
        assert_eq!(schema.annotations.len(), 0);
    }

    #[test]
    fn mcp_tool_requires_confirmation_when_not_readonly() {
        let shared = std::sync::Arc::new(McpShared {
            event_tx: mpsc::channel(1).0,
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        });
        let read = McpToolSchema {
            name: "read_thing".to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            annotations: vec!["read".to_string()],
        };
        let tool = McpTool::new(&read, shared.clone());
        assert!(!tool.requires_confirmation());

        let write = McpToolSchema {
            name: "write_thing".to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            annotations: vec!["write".to_string()],
        };
        let tool = McpTool::new(&write, shared);
        assert!(tool.requires_confirmation());
    }
}
// === AI-WORKSHOP END ===
