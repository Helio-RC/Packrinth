// === AI-WORKSHOP START ===
// AI 工作台处于开发中：stub 接口与后续任务的填充面在此阶段允许 dead_code
#![allow(dead_code)]
pub mod chat_history;
pub mod config;
pub mod context_guard;
pub mod git_ops;
pub mod inference;
pub mod keystore;
pub mod knowledge;
pub mod mcp_client;
pub mod providers;
pub mod skills;
pub mod toolchain;
pub mod tools;
pub mod troubleshooter;
pub mod ui_commands;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use notify::Watcher;
use tauri::{Listener, Manager, Runtime};

use crate::ai_workshop::chat_history::repository::ChatHistoryRepository;
use crate::api::Result;
use chat_history::models::NewMessage;
use config::ConfigManager;
use knowledge::KnowledgeRouter;
use providers::provider_trait::StreamEvent;
use skills::SkillLoader;
use toolchain::ToolchainRegistry;
use tools::context::{InstanceLockManager, TaskRegistry};
use tools::registry::ToolRegistry;
use troubleshooter::LogBuffer;

pub(crate) fn other_err(
    msg: impl Into<String>,
) -> crate::api::TheseusSerializableError {
    crate::api::TheseusSerializableError::Theseus(theseus::Error::from(
        theseus::ErrorKind::OtherError(msg.into()),
    ))
}

fn serde_err(e: serde_json::Error) -> crate::api::TheseusSerializableError {
    other_err(e.to_string())
}

#[derive(Clone)]
pub struct AiWorkshopState {
    pub config_manager: Arc<ConfigManager>,
    pub chat_history: Arc<ChatHistoryRepository>,
    pub tool_registry: Arc<ToolRegistry>,
    pub toolchain_registry: Arc<ToolchainRegistry>,
    pub skill_loader: Arc<SkillLoader>,
    pub knowledge_router: Arc<KnowledgeRouter>,
    pub instance_lock_manager: Arc<InstanceLockManager>,
    pub log_buffer: Arc<LogBuffer>,
    pub task_registry: Arc<TaskRegistry>,
    /// 工具确认的 out-of-band 存储：tool_call_id → 用户是否批准。
    /// 不持久化，重启后待确认的工具调用自动失效（符合"挂起/恢复"简化裁决）。
    pub pending_confirmations: Arc<Mutex<HashMap<String, bool>>>,
}

impl AiWorkshopState {
    /// 记录一次工具确认结果（key = tool_call_id）。
    pub fn store_confirmation(&self, tool_call_id: &str, approved: bool) {
        self.pending_confirmations
            .lock()
            .unwrap()
            .insert(tool_call_id.to_string(), approved);
    }

    /// 读取并移除一次确认结果；不存在则返回 None。
    pub fn take_confirmation(&self, tool_call_id: &str) -> Option<bool> {
        self.pending_confirmations
            .lock()
            .unwrap()
            .remove(tool_call_id)
    }
}

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("ai_workshop")
        .setup(|app, _api| {
            tauri::async_runtime::block_on(async {
                // theseus State 由前端 webview 加载后的 initialize_state 命令初始化，
                // 而插件 setup 先于它运行：此处轮询等待（最多 30 秒），
                // 否则 State::get 会报 "state before initialized" 并导致应用启动失败。
                let mut waited_secs = 0u32;
                loop {
                    if theseus::prelude::State::get().await.is_ok() {
                        break;
                    }
                    if waited_secs >= 30 {
                        tracing::warn!(
                            "ai_workshop: theseus state not initialized after 30s; AI workbench skipped"
                        );
                        return Ok(());
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    waited_secs += 1;
                }

                let config_manager = ConfigManager::load(app).await?;
                let chat_history = Arc::new(ChatHistoryRepository::open(
                    &config_manager.chat_db_path(),
                )?);
                let tool_registry = Arc::new(ToolRegistry::new());
                tools::register_builtin_tools(&tool_registry);
                let toolchain_registry = Arc::new(ToolchainRegistry::new());
                toolchain::register_builtin_toolchains(&toolchain_registry);
                let skill_loader =
                    Arc::new(SkillLoader::new(config_manager.skills_dir()));
                let failed_skills = skill_loader.load_all().await;
                if !failed_skills.is_empty() {
                    tracing::warn!(
                        "ai_workshop: failed to load skills: {failed_skills:?}"
                    );
                }
                let knowledge_router = Arc::new(KnowledgeRouter::new(
                    config_manager.bm25_index_dir(),
                ));
                knowledge_router.register_source(Arc::new(
                    knowledge::source::SkillsSource::new(
                        config_manager.skills_dir(),
                    ),
                ));
                let log_buffer =
                    Arc::new(LogBuffer::new(config_manager.config().log_lines));
                // 行数阈值落盘：log_lines / 10（默认 500/10=50 行），与 §C.5 一致。
                log_buffer.attach_dest(
                    config_manager.logs_dir(),
                    (config_manager.config().log_lines / 10).max(1),
                );
                let instance_lock_manager =
                    Arc::new(InstanceLockManager::default());
                let task_registry = Arc::new(TaskRegistry::default());

                app.manage(AiWorkshopState {
                    config_manager: config_manager.clone(),
                    chat_history: chat_history.clone(),
                    tool_registry: tool_registry.clone(),
                    toolchain_registry: toolchain_registry.clone(),
                    skill_loader: skill_loader.clone(),
                    knowledge_router,
                    instance_lock_manager,
                    log_buffer: log_buffer.clone(),
                    task_registry: task_registry.clone(),
                    pending_confirmations: Arc::new(Mutex::new(HashMap::new())),
                });

                spawn_log_persist_loop(&config_manager, &log_buffer);
                spawn_skill_watcher(&config_manager, &skill_loader);

                let mcp_cfg = config_manager.config().mcp;
                if mcp_cfg.enabled {
                    // MCP 客户端仅在配置启用时拉起（默认禁用）；工具注册进共享 ToolRegistry。
                    mcp_client::McpClient::spawn(
                        mcp_cfg.command,
                        mcp_cfg.args,
                        mcp_cfg.health_check_interval_secs,
                        tool_registry.clone(),
                    );
                }

                let close_task_registry = task_registry;
                app.listen("tauri://close-requested", move |_| {
                    close_task_registry.cancel_all();
                });

                Ok(())
            })
        })
        .invoke_handler(tauri::generate_handler![
            ai_chat,
            ai_stream,
            ai_confirm_tool,
            set_provider_api_key,
            test_provider_connection,
            tool_execute,
            cancel_task,
            list_tools,
            get_tool_schema,
            list_toolchains,
            execute_toolchain_command,
            list_conversations,
            get_conversation,
            create_conversation,
            rename_conversation,
            delete_conversation,
            export_conversation,
            clear_all_conversations,
            list_skills,
            get_skill_content,
            enable_skill,
            disable_skill,
            force_load_skill,
            refresh_skills,
            import_skill,
            search_knowledge,
            refresh_knowledge,
            get_ai_config,
            set_ai_config,
            get_ai_status,
            analyze_crash,
            get_logs_for_ai,
            suggest_fix,
            apply_fix,
            inject_crash_log,
        ])
        .build()
}

/// 日志环形缓冲区周期性落盘（间隔可由配置 `log_flush_interval_secs` 调整，默认 120 秒），
/// 失败仅记录警告。行数阈值落盘由 LogBuffer::push 内部触发（见 §C.5）。
fn spawn_log_persist_loop(
    config_manager: &Arc<ConfigManager>,
    log_buffer: &Arc<LogBuffer>,
) {
    let log_buffer = log_buffer.clone();
    let logs_dir = config_manager.logs_dir();
    let interval = config_manager.config().log_flush_interval_secs.max(1);
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
            if let Err(e) = log_buffer.flush_to_disk(&logs_dir) {
                tracing::warn!(
                    "ai_workshop: failed to persist log buffer: {e}"
                );
            }
        }
    });
}

/// 技能目录热加载监听；初始化失败仅记录警告并降级为手动刷新，不阻塞启动。
fn spawn_skill_watcher(
    config_manager: &Arc<ConfigManager>,
    skill_loader: &Arc<SkillLoader>,
) {
    let skills_dir = config_manager.skills_dir();
    let skill_loader = skill_loader.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = std::fs::create_dir_all(&skills_dir) {
            tracing::warn!("ai_workshop: failed to create skills dir: {e}");
            return;
        }
        let (event_tx, event_rx) =
            std::sync::mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = match notify::recommended_watcher(event_tx) {
            Ok(watcher) => watcher,
            Err(e) => {
                tracing::warn!(
                    "ai_workshop: failed to init skills watcher, falling back to manual refresh: {e}"
                );
                return;
            }
        };
        if let Err(e) =
            watcher.watch(&skills_dir, notify::RecursiveMode::Recursive)
        {
            tracing::warn!(
                "ai_workshop: failed to watch skills dir, falling back to manual refresh: {e}"
            );
            return;
        }
        // notify 是阻塞 API，事件转发放到 spawn_blocking，避免占用 async 线程
        let (signal_tx, mut signal_rx) = tokio::sync::mpsc::channel::<()>(16);
        tauri::async_runtime::spawn_blocking(move || {
            while event_rx.recv().is_ok() && signal_tx.blocking_send(()).is_ok()
            {
            }
        });
        // watcher 存活于本任务以保持注册，目录变化时触发技能重载
        while signal_rx.recv().await.is_some() {
            let failed = skill_loader.refresh().await;
            if !failed.is_empty() {
                tracing::warn!(
                    "ai_workshop: skills refresh failed for: {failed:?}"
                );
            }
        }
    });
}

// AI 对话

/// 单轮非流式对话。执行推理、持久化 user/assistant 消息，返回 `{ reply, usage }`。
#[tauri::command]
pub async fn ai_chat<R: Runtime>(
    app: tauri::AppHandle<R>,
    conversation_id: String,
    content: String,
) -> Result<serde_json::Value> {
    let state = app.state::<AiWorkshopState>();
    let engine = inference::engine::InferenceEngine::new(Arc::new(
        state.inner().clone(),
    ));
    let result = engine.run_single_turn(&conversation_id, &content).await?;

    state
        .chat_history
        .add_message(NewMessage {
            conversation_id: conversation_id.clone(),
            role: "user".to_string(),
            content: content.clone(),
            tool_calls: None,
            tool_call_id: None,
        })
        .await?;
    state
        .chat_history
        .add_message(NewMessage {
            conversation_id: conversation_id.clone(),
            role: "assistant".to_string(),
            content: result.reply.clone(),
            tool_calls: None,
            tool_call_id: None,
        })
        .await?;

    Ok(serde_json::json!({ "reply": result.reply, "usage": result.usage }))
}

/// 流式多轮对话。后台执行推理循环（含工具调用轮次），事件经 Channel 推送前端。
#[tauri::command]
pub async fn ai_stream<R: Runtime>(
    app: tauri::AppHandle<R>,
    conversation_id: String,
    content: String,
    on_event: tauri::ipc::Channel<StreamEvent>,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>().inner().clone();
    state
        .chat_history
        .add_message(NewMessage {
            conversation_id: conversation_id.clone(),
            role: "user".to_string(),
            content: content.clone(),
            tool_calls: None,
            tool_call_id: None,
        })
        .await?;

    tauri::async_runtime::spawn(async move {
        let engine =
            inference::engine::InferenceEngine::new(Arc::new(state.clone()));
        let event_sender = on_event.clone();
        let mut done_event = StreamEvent {
            delta: None,
            tool_calls: None,
            usage: None,
            done: true,
            error: None,
        };
        if let Err(e) = engine
            .run_multi_turn(
                &conversation_id,
                &content,
                Box::new(move |event: StreamEvent| {
                    let _ = event_sender.send(event);
                }),
            )
            .await
        {
            let _ = on_event.send(StreamEvent {
                delta: None,
                tool_calls: None,
                usage: None,
                done: false,
                error: Some(e.to_string()),
            });
            done_event.error = Some(e.to_string());
        }
        let _ = on_event.send(done_event);
    });

    Ok(())
}

/// 记录用户对某次工具调用的确认结果（out-of-band，不写入 chat_history），
/// 推理引擎轮询 `pending_confirmations` 读取，避免污染工具消息历史。
#[tauri::command]
pub async fn ai_confirm_tool<R: Runtime>(
    app: tauri::AppHandle<R>,
    conversation_id: String,
    tool_call_id: String,
    approved: bool,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    // 确认以 tool_call_id 为 key 暂存于内存 map，不落库；引擎消费后即移除。
    // 重启后待确认的工具调用失效（符合"挂起/恢复"简化裁决）。
    let _ = conversation_id;
    state.store_confirmation(&tool_call_id, approved);
    Ok(())
}

/// 为指定提供商保存真实 API Key：写入系统密钥环（keyring），并同步掩码提示到 config.json。
#[tauri::command]
pub async fn set_provider_api_key<R: Runtime>(
    app: tauri::AppHandle<R>,
    provider: String,
    api_key: String,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    state.config_manager.set_api_key(&provider, &api_key)
}

/// 连接测试（设置页"连接测试"按钮）：用给定提供商发一条极简短消息验证 Key / 端点有效。
#[tauri::command]
pub async fn test_provider_connection<R: Runtime>(
    app: tauri::AppHandle<R>,
    provider: String,
) -> Result<serde_json::Value> {
    use providers::provider_trait::AiMessage;

    let state = app.state::<AiWorkshopState>();
    let provider_instance = providers::factory::create_provider(
        &state.config_manager,
        Some(&provider),
    )
    .map_err(other_err)?;
    let test_messages = vec![AiMessage::user("请只回复：OK".to_string())];
    match provider_instance.chat(&test_messages, &[]).await {
        Ok(response) => Ok(serde_json::json!({
            "ok": true,
            "reply": response.content.unwrap_or_default(),
        })),
        Err(e) => Ok(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
        })),
    }
}

// 工具

/// 手动执行工具（供前端工具面板调用），返回 ToolResponse（含 task_id 便于取消）。
#[tauri::command]
pub async fn tool_execute<R: Runtime>(
    app: tauri::AppHandle<R>,
    name: String,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let state = app.state::<AiWorkshopState>();
    let task_id = uuid::Uuid::new_v4().to_string();
    let mut response = ui_commands::execute_tool(
        state.inner(),
        Some(&app),
        &task_id,
        &name,
        params,
    )
    .await?;
    if let Some(obj) = response.as_object_mut() {
        obj.insert("task_id".to_string(), serde_json::Value::String(task_id));
    }
    Ok(response)
}

/// 通过 task_id 取消进行中的工具任务。
#[tauri::command]
pub async fn cancel_task<R: Runtime>(
    app: tauri::AppHandle<R>,
    task_id: String,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    let _ = state.task_registry.cancel(&task_id);
    Ok(())
}

/// 列出全部已注册工具（含参数 Schema）。
#[tauri::command]
pub async fn list_tools<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<serde_json::Value>> {
    let state = app.state::<AiWorkshopState>();
    let mut tools = Vec::new();
    for tool in state.tool_registry.list() {
        tools.push(serde_json::json!({
            "name": tool.name,
            "description": tool.description,
            "domain": format!("{:?}", tool.domain),
            "requires_confirmation": tool.requires_confirmation,
            "is_readonly": tool.is_readonly,
            "params_schema": tool.params_schema,
        }));
    }
    Ok(tools)
}

/// 获取单个工具的 JSON Schema（供前端动态渲染表单）。
#[tauri::command]
pub async fn get_tool_schema<R: Runtime>(
    app: tauri::AppHandle<R>,
    name: String,
) -> Result<serde_json::Value> {
    let state = app.state::<AiWorkshopState>();
    let schema = state
        .tool_registry
        .schema(&name)
        .ok_or_else(|| other_err(format!("Unknown tool: {name}")))?;
    serde_json::to_value(schema).map_err(serde_err)
}

/// 列出全部工具链（名称 + 描述）。
#[tauri::command]
pub async fn list_toolchains<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<serde_json::Value>> {
    let state = app.state::<AiWorkshopState>();
    let mut chains = Vec::new();
    for name in state.toolchain_registry.list() {
        if let Some(toolchain) = state.toolchain_registry.get(name) {
            chains.push(serde_json::json!({
                "name": toolchain.name(),
                "description": toolchain.description(),
            }));
        }
    }
    Ok(chains)
}

/// 执行工具链（供手动面板调用）。
#[tauri::command]
pub async fn execute_toolchain_command<R: Runtime>(
    app: tauri::AppHandle<R>,
    name: String,
    instance_id: Option<String>,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let state = app.state::<AiWorkshopState>();
    ui_commands::execute_toolchain(&state, &name, instance_id, params).await
}

// 对话历史

/// 列出会话，按 `updated_at` 降序分页。
#[tauri::command]
pub async fn list_conversations<R: Runtime>(
    app: tauri::AppHandle<R>,
    instance_id: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<chat_history::models::Conversation>> {
    let state = app.state::<AiWorkshopState>();
    state
        .chat_history
        .list_conversations(
            instance_id.as_deref(),
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
        .await
}

/// 获取单个会话及其消息，返回 `{ conversation, messages }`。
#[tauri::command]
pub async fn get_conversation<R: Runtime>(
    app: tauri::AppHandle<R>,
    conversation_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Option<serde_json::Value>> {
    let state = app.state::<AiWorkshopState>();
    let Some((conversation, messages)) = state
        .chat_history
        .get_conversation(
            &conversation_id,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(serde_json::json!({
        "conversation": conversation,
        "messages": messages,
    })))
}

/// 新建会话。
#[tauri::command]
pub async fn create_conversation<R: Runtime>(
    app: tauri::AppHandle<R>,
    title: String,
    instance_id: Option<String>,
) -> Result<chat_history::models::Conversation> {
    let state = app.state::<AiWorkshopState>();
    state
        .chat_history
        .create_conversation(&title, instance_id.as_deref())
        .await
}

/// 重命名会话。
#[tauri::command]
pub async fn rename_conversation<R: Runtime>(
    app: tauri::AppHandle<R>,
    conversation_id: String,
    new_title: String,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    state
        .chat_history
        .rename_conversation(&conversation_id, &new_title)
        .await
}

/// 删除会话及其全部消息。
#[tauri::command]
pub async fn delete_conversation<R: Runtime>(
    app: tauri::AppHandle<R>,
    conversation_id: String,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    state
        .chat_history
        .delete_conversation(&conversation_id)
        .await
}

/// 导出会话为 `json` 或 `markdown`。
#[tauri::command]
pub async fn export_conversation<R: Runtime>(
    app: tauri::AppHandle<R>,
    conversation_id: String,
    format: String,
) -> Result<String> {
    let state = app.state::<AiWorkshopState>();
    state
        .chat_history
        .export_conversation(&conversation_id, &format)
        .await
}

/// 清空全部会话（需 `confirm=true`），返回删除的会话数。
#[tauri::command]
pub async fn clear_all_conversations<R: Runtime>(
    app: tauri::AppHandle<R>,
    confirm: bool,
) -> Result<usize> {
    if !confirm {
        return Err(other_err("clear_all_conversations requires confirm=true"));
    }
    let state = app.state::<AiWorkshopState>();
    state.chat_history.clear_all(true).await
}

// 技能

/// 列出全部技能（含启用状态）。
#[tauri::command]
pub async fn list_skills<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<serde_json::Value> {
    let state = app.state::<AiWorkshopState>();
    let mut skills = Vec::new();
    for skill in state.skill_loader.skills() {
        skills.push(serde_json::to_value(skill).map_err(serde_err)?);
    }
    let failed = state
        .skill_loader
        .failed_skills()
        .iter()
        .map(|f| {
            serde_json::json!({
                "dir_name": f.dir_name,
                "reason": f.reason,
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({ "skills": skills, "failed": failed }))
}

/// 获取技能详情及 guide.md 全文，返回 `{ skill, guide_md }`。
#[tauri::command]
pub async fn get_skill_content<R: Runtime>(
    app: tauri::AppHandle<R>,
    skill_name: String,
) -> Result<serde_json::Value> {
    let state = app.state::<AiWorkshopState>();
    let skill = state
        .skill_loader
        .get_skill(&skill_name)
        .ok_or_else(|| other_err(format!("Unknown skill: {skill_name}")))?;
    let guide_md = state.skill_loader.guide_md(&skill_name).unwrap_or_default();
    Ok(serde_json::json!({
        "skill": skill,
        "guide_md": guide_md,
    }))
}

/// 启用技能。
#[tauri::command]
pub async fn enable_skill<R: Runtime>(
    app: tauri::AppHandle<R>,
    skill_name: String,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    state.skill_loader.set_enabled(&skill_name, true)
}

/// 禁用技能。
#[tauri::command]
pub async fn disable_skill<R: Runtime>(
    app: tauri::AppHandle<R>,
    skill_name: String,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    state.skill_loader.set_enabled(&skill_name, false)
}

/// 强制加载单个技能（绕过自动匹配）。
#[tauri::command]
pub async fn force_load_skill<R: Runtime>(
    app: tauri::AppHandle<R>,
    skill_name: String,
) -> Result<serde_json::Value> {
    let state = app.state::<AiWorkshopState>();
    let skill = state.skill_loader.force_load(&skill_name)?;
    serde_json::to_value(skill).map_err(serde_err)
}

/// 重新扫描技能目录，返回加载失败的技能名列表。
#[tauri::command]
pub async fn refresh_skills<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>> {
    let state = app.state::<AiWorkshopState>();
    Ok(state.skill_loader.refresh().await)
}

/// 将用户提供的技能目录复制到 skills 目录（safe_path 校验由 loader 完成）。
#[tauri::command]
pub async fn import_skill<R: Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    state.skill_loader.import_skill(&path).await
}

// 知识

/// BM25 知识检索。
#[tauri::command]
pub async fn search_knowledge<R: Runtime>(
    app: tauri::AppHandle<R>,
    query: String,
    top_k: Option<usize>,
    source: Option<String>,
) -> Result<Vec<serde_json::Value>> {
    let state = app.state::<AiWorkshopState>();
    state
        .knowledge_router
        .search(&query, top_k.unwrap_or(3), source.as_deref())
        .await
}

/// 手动刷新知识索引。
#[tauri::command]
pub async fn refresh_knowledge<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    state.knowledge_router.refresh().await
}

// 配置与状态

/// 获取当前 AI 工作台配置（IPC 边界为 camelCase DTO；文件格式保持 snake_case）。
#[tauri::command]
pub async fn get_ai_config<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<config::AiWorkshopConfigDto> {
    let state = app.state::<AiWorkshopState>();
    Ok(config::AiWorkshopConfigDto::from(
        &state.config_manager.config(),
    ))
}

/// 保存 AI 工作台配置（接收 camelCase DTO，落盘前转为 snake_case 文件格式）。
#[tauri::command]
pub async fn set_ai_config<R: Runtime>(
    app: tauri::AppHandle<R>,
    config: config::AiWorkshopConfigDto,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    state.config_manager.save_config(config.into()).await
}

/// 获取 AI 工作台运行状态摘要。
#[tauri::command]
pub async fn get_ai_status<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<serde_json::Value> {
    let state = app.state::<AiWorkshopState>();
    let config = state.config_manager.config();
    let provider_configured = config
        .default_provider
        .as_deref()
        .and_then(|name| config.providers.get(name))
        .map(|provider| provider.enabled)
        .unwrap_or(false);
    let conversation_count = state.chat_history.count_conversations().await?;
    Ok(serde_json::json!({
        "enabled": config.enabled,
        "mock_enabled": config.mock_enabled,
        "default_provider": config.default_provider,
        "provider_configured": provider_configured,
        "skill_count": state.skill_loader.skills().len(),
        "conversation_count": conversation_count,
        "log_buffer_capacity": state.log_buffer.capacity(),
    }))
}

// 排障

/// 分析崩溃日志（当前返回原始日志，AI 分析由高级场景层实现）。
#[tauri::command]
pub async fn analyze_crash<R: Runtime>(
    app: tauri::AppHandle<R>,
    instance_id: Option<String>,
) -> Result<serde_json::Value> {
    let state = app.state::<AiWorkshopState>();
    let config = state.config_manager.config();
    Ok(serde_json::json!({
        "instance_id": instance_id,
        "log_lines": state.log_buffer.content(),
        "ai_enabled": config.enabled,
        "analysis": null,
    }))
}

/// 获取日志缓冲区内容（供 AI 分析）。
#[tauri::command]
pub async fn get_logs_for_ai<R: Runtime>(
    app: tauri::AppHandle<R>,
    limit: Option<usize>,
) -> Result<Vec<String>> {
    let state = app.state::<AiWorkshopState>();
    Ok(state.log_buffer.tail(limit.unwrap_or(500)))
}

/// 生成修复建议（当前返回空列表，AI 建议由高级场景层实现）。
#[tauri::command]
pub async fn suggest_fix<R: Runtime>(
    app: tauri::AppHandle<R>,
    crash_log: Option<String>,
) -> Result<serde_json::Value> {
    let state = app.state::<AiWorkshopState>();
    let logs = match crash_log {
        Some(log) => log,
        None => state.log_buffer.content().join("\n"),
    };
    Ok(serde_json::json!({
        "suggestions": [],
        "crash_log_length": logs.len(),
    }))
}

/// 应用修复建议（尚未实现，返回明确错误）。
#[tauri::command]
pub async fn apply_fix<R: Runtime>(
    app: tauri::AppHandle<R>,
    fix_id: String,
) -> Result<()> {
    let _ = app;
    Err(other_err(format!(
        "apply_fix({fix_id}) is not implemented yet"
    )))
}

/// 仅测试用：向日志缓冲区注入崩溃日志。
/// 发布构建提供同名命令但直接报错，保证 handler 宏在两个 profile 下都能生成。
#[cfg(not(debug_assertions))]
#[tauri::command]
pub async fn inject_crash_log<R: Runtime>(
    _: tauri::AppHandle<R>,
    _: String,
) -> Result<()> {
    Err(other_err("inject_crash_log 仅在调试构建可用"))
}

#[cfg(debug_assertions)]
#[tauri::command]
pub async fn inject_crash_log<R: Runtime>(
    app: tauri::AppHandle<R>,
    log_content: String,
) -> Result<()> {
    let state = app.state::<AiWorkshopState>();
    state.log_buffer.inject(log_content);
    Ok(())
}
// === AI-WORKSHOP END ===
