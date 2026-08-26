// === AI-WORKSHOP START ===
use std::sync::Arc;

use crate::ai_workshop::tools::context::ExecutionContext;
use crate::api::Result;

/// 可执行工具链抽象：由多个原子工具组合的复合流程（L2，编译时固化）。
#[async_trait::async_trait]
pub trait ExecutableToolchain: Send + Sync {
	fn name(&self) -> &'static str;
	fn description(&self) -> &'static str;
	async fn execute(
		&self,
		instance_id: Option<&str>,
		ctx: &ExecutionContext,
	) -> Result<serde_json::Value>;
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
}
// === AI-WORKSHOP END ===