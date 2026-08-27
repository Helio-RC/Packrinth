// === AI-WORKSHOP START ===
use serde_json::Value;
use tauri::Emitter;
use tauri::Runtime;

use crate::api::Result;
use crate::ai_workshop::other_err;
use crate::ai_workshop::AiWorkshopState;
use crate::ai_workshop::tools::context::{ExecutionContext, ProgressPayload};

/// 工具默认超时（300 秒），配置后续可接入。
const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// 手动执行工具（供前端工具面板调用）：与 AI 引擎共用同一 `ToolRegistry`。
/// 流 C.6 完善：进度上报、取消机制、超时兜底。
pub async fn execute_tool<R: Runtime>(
	state: &AiWorkshopState,
	app: Option<&tauri::AppHandle<R>>,
	task_id: &str,
	name: &str,
	params: Value,
) -> Result<Value> {
	execute_tool_with_timeout(state, app, task_id, name, params, TOOL_TIMEOUT).await
}

/// 内部实现，超时可注入（供测试缩短）。
async fn execute_tool_with_timeout<R: Runtime>(
	state: &AiWorkshopState,
	app: Option<&tauri::AppHandle<R>>,
	task_id: &str,
	name: &str,
	params: Value,
	timeout: std::time::Duration,
) -> Result<Value> {
	let Some(tool) = state.tool_registry.get(name) else {
		return Err(other_err(format!("Unknown tool: {name}")));
	};

	let emit_progress = app.map(|app| {
		let app = app.clone();
		Box::new(move |payload: &ProgressPayload| {
			let _ = app.emit("tool-progress", payload);
		}) as Box<dyn Fn(&ProgressPayload) + Send + Sync>
	});

	let context = ExecutionContext {
		instance_id: None,
		cancellation_token: state
			.task_registry
			.new_token(task_id)
			.unwrap_or_default(),
		// 与 AI 引擎共享同一实例写锁管理器，保证写操作跨入口串行化。
		instance_lock_manager: state.instance_lock_manager.clone(),
		emit_progress,
	};

	match tokio::time::timeout(timeout, tool.execute(params, &context)).await {
		Ok(Ok(value)) => Ok(serde_json::json!({ "success": true, "data": value })),
		Ok(Err(e)) => Ok(serde_json::json!({ "success": false, "error": { "code": "TOOL_ERROR", "message": e } })),
		Err(_) => {
			// 超时兜底：尽力取消对应任务令牌，让仍在运行的循环尽快退出。
			context.cancellation_token.cancel();
			Ok(serde_json::json!({ "success": false, "error": { "code": "TOOL_TIMEOUT", "message": "工具执行超时" } }))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashMap;
	use std::sync::Arc;
	use std::sync::Mutex;

	use async_trait::async_trait;

	use crate::ai_workshop::tools::context::{InstanceLockManager, TaskRegistry};
	use crate::ai_workshop::tools::registry::{Tool, ToolDomain, ToolInfo, ToolRegistry};
	use crate::ai_workshop::toolchain::ToolchainRegistry;
	use crate::ai_workshop::SkillLoader;
	use crate::ai_workshop::troubleshooter::LogBuffer;
	use crate::ai_workshop::{config::ConfigManager, config::AiWorkshopConfig, AiWorkshopState};

	type ToolResult = std::result::Result<serde_json::Value, String>;

	struct SleepTool;
	#[async_trait]
	impl Tool for SleepTool {
		fn info(&self) -> ToolInfo {
			ToolInfo {
				name: "sleep_tool".to_string(),
				description: "sleeps to trigger timeout".to_string(),
				domain: ToolDomain::System,
				requires_confirmation: false,
				is_readonly: true,
				params_schema: Value::Null,
			}
		}
		async fn execute(
			&self,
			_arguments: Value,
			_ctx: &ExecutionContext,
		) -> ToolResult {
			tokio::time::sleep(std::time::Duration::from_secs(60)).await;
			Ok(Value::Null)
		}
	}

	struct FailingTool;
	#[async_trait]
	impl Tool for FailingTool {
		fn info(&self) -> ToolInfo {
			ToolInfo {
				name: "failing_tool".to_string(),
				description: "always fails".to_string(),
				domain: ToolDomain::System,
				requires_confirmation: false,
				is_readonly: true,
				params_schema: Value::Null,
			}
		}
		async fn execute(
			&self,
			_arguments: Value,
			_ctx: &ExecutionContext,
		) -> ToolResult {
			Err("boom".to_string())
		}
	}

	fn test_state() -> AiWorkshopState {
		let dir = std::env::temp_dir().join(format!("packrinth-ai-uicmd-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&dir);
		let config_manager = ConfigManager::for_tests(AiWorkshopConfig::default(), dir.clone());
		let tool_registry = Arc::new(ToolRegistry::new());
		AiWorkshopState {
			config_manager,
			chat_history: Arc::new(
				crate::ai_workshop::chat_history::repository::ChatHistoryRepository::open(
					&dir.join("chat.db"),
				)
				.expect("open temp db"),
			),
			tool_registry,
			toolchain_registry: Arc::new(ToolchainRegistry::new()),
			skill_loader: Arc::new(SkillLoader::new(dir.join("skills"))),
			knowledge_router: Arc::new(crate::ai_workshop::knowledge::KnowledgeRouter::new(
				dir.join("bm25"),
			)),
			instance_lock_manager: Arc::new(InstanceLockManager::default()),
			log_buffer: Arc::new(LogBuffer::new(100)),
			task_registry: Arc::new(TaskRegistry::default()),
			pending_confirmations: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	#[tokio::test]
	async fn execute_tool_times_out_and_cancels_token() {
		let state = test_state();
		state.tool_registry.register(Arc::new(SleepTool));
		let task_id = "timeout-task";

		let result = execute_tool_with_timeout(
			&state,
			None::<&tauri::AppHandle<tauri::Wry>>,
			task_id,
			"sleep_tool",
			Value::Null,
			std::time::Duration::from_millis(50),
		)
		.await
		.expect("timeout returns Ok wrapper");

		let obj = result.as_object().expect("response should be object");
		assert_eq!(obj["success"], Value::Bool(false));
		assert_eq!(obj["error"]["code"], Value::String("TOOL_TIMEOUT".to_string()));

		// 超时兜底应尽力取消任务令牌：注册表中该任务对应令牌仍可被取消。
		assert!(
			state.task_registry.cancel("timeout-task"),
			"timeout-task token should still be registered"
		);
	}

	#[tokio::test]
	async fn execute_tool_propagates_tool_error() {
		let state = test_state();
		state.tool_registry.register(Arc::new(FailingTool));

		let result = execute_tool_with_timeout(
			&state,
			None::<&tauri::AppHandle<tauri::Wry>>,
			"fail-task",
			"failing_tool",
			Value::Null,
			std::time::Duration::from_secs(10),
		)
		.await
		.expect("error path returns Ok wrapper");

		let obj = result.as_object().expect("response should be object");
		assert_eq!(obj["success"], Value::Bool(false));
		assert_eq!(obj["error"]["code"], Value::String("TOOL_ERROR".to_string()));
		assert_eq!(obj["error"]["message"], Value::String("boom".to_string()));
	}

	#[tokio::test]
	async fn execute_tool_unknown_tool_returns_error() {
		let state = test_state();
		let result = execute_tool(
			&state,
			None::<&tauri::AppHandle<tauri::Wry>>,
			"x",
			"does_not_exist",
			Value::Null,
		)
		.await;
		assert!(result.is_err(), "unknown tool should be an Err");
	}
}
// === AI-WORKSHOP END ===