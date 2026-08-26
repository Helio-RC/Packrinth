// === AI-WORKSHOP START ===
use std::collections::HashMap;
use std::sync::Mutex;

/// 可执行工具链抽象：由多个原子工具组合的复合流程（L2，编译时固化）。
/// 流 D.6 实现具体工具链（KubeJsGen、FtbRecipe 等）。
pub struct ToolchainRegistry {
	inner: Mutex<HashMap<String, String>>,
}

impl ToolchainRegistry {
	pub fn new() -> Self {
		Self {
			inner: Mutex::new(HashMap::new()),
		}
	}

	pub fn register(&self, name: &str, description: &str) {
		self.inner
			.lock()
			.unwrap()
			.insert(name.to_string(), description.to_string());
	}
}
// === AI-WORKSHOP END ===