// === AI-WORKSHOP START ===
use std::sync::Arc;

use crate::ai_workshop::tools::context::ExecutionContext;
use crate::api::Result;

/// 可执行工具链抽象：由多个原子工具组合的复合流程（L2，编译时固化）。
/// `params` 为工具链入参（如脚本内容、配方 JSON），由调用方（AI 引擎 / 手动面板）传入。
#[async_trait::async_trait]
pub trait ExecutableToolchain: Send + Sync {
	fn name(&self) -> &'static str;
	fn description(&self) -> &'static str;
	async fn execute(
		&self,
		instance_id: Option<&str>,
		params: serde_json::Value,
		ctx: &ExecutionContext,
	) -> Result<serde_json::Value>;
}

/// 字符串参数读取辅助（与工具层一致的错误语义）。
pub fn string_arg(arguments: &serde_json::Value, key: &str) -> crate::api::Result<String> {
	arguments
		.get(key)
		.and_then(serde_json::Value::as_str)
		.map(str::to_string)
		.ok_or_else(|| crate::ai_workshop::other_err(format!("缺少参数: {key}")))
}

/// 工具链注册表（供引擎与工具面板查询）。
#[derive(Default)]
pub struct ToolchainRegistry {
	inner: std::sync::Mutex<Vec<Arc<dyn ExecutableToolchain>>>,
}

impl ToolchainRegistry {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn register(&self, toolchain: Arc<dyn ExecutableToolchain>) {
		self.inner.lock().unwrap().push(toolchain);
	}

	pub fn list(&self) -> Vec<&'static str> {
		self.inner.lock().unwrap().iter().map(|t| t.name()).collect()
	}

	/// 按名称查询工具链；重复名称返回首个注册实例。
	pub fn get(&self, name: &str) -> Option<Arc<dyn ExecutableToolchain>> {
		let inner = self.inner.lock().unwrap();
		inner
			.iter()
			.find(|t| t.name() == name)
			.map(Arc::clone)
	}
}
// === AI-WORKSHOP END ===