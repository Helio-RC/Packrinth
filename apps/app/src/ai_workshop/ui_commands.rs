// === AI-WORKSHOP START ===
use serde_json::Value;

use crate::api::Result;
use crate::ai_workshop::other_err;
use crate::ai_workshop::AiWorkshopState;
use crate::ai_workshop::tools::context::ExecutionContext;

/// 手动执行工具（供前端工具面板调用）：与 AI 引擎共用同一 `ToolRegistry`。
/// 流 C.6 完善：进度上报、取消机制、超时兜底。
pub async fn execute_tool(
	state: &AiWorkshopState,
	task_id: &str,
	name: &str,
	params: Value,
) -> Result<Value> {
	let Some(tool) = state.tool_registry.get(name) else {
		return Err(other_err(format!("Unknown tool: {name}")));
	};

	let context = ExecutionContext {
		instance_id: None,
		cancellation_token: state
			.task_registry
			.new_token(task_id)
			.unwrap_or_default(),
	};

	match tool.execute(params, &context).await {
		Ok(value) => Ok(serde_json::json!({ "success": true, "data": value })),
		Err(e) => Ok(serde_json::json!({ "success": false, "error": { "code": "TOOL_ERROR", "message": e } })),
	}
}
// === AI-WORKSHOP END ===