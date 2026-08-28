// === AI-WORKSHOP START ===
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;

use super::context::ExecutionContext;

/// 工具领域分类（用于前端分组展示）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ToolDomain {
	Mods,
	Config,
	Script,
	Instance,
	Git,
	Knowledge,
	System,
	Mcp,
}

/// 工具的静态描述信息（含参数 Schema，供前端动态渲染表单）。
#[derive(Clone, Debug, Serialize)]
pub struct ToolInfo {
	pub name: String,
	pub description: String,
	pub domain: ToolDomain,
	pub requires_confirmation: bool,
	pub is_readonly: bool,
	pub params_schema: Value,
}

/// 原子工具抽象：单一职责的最小可执行单元，供 AI 引擎与前端 UI 共用。
#[async_trait]
pub trait Tool: Send + Sync {
	fn info(&self) -> ToolInfo;
	fn requires_confirmation(&self) -> bool {
		false
	}
	async fn execute(
		&self,
		arguments: Value,
		ctx: &ExecutionContext,
	) -> Result<Value, String>;
}

/// 工具注册表：AI 引擎与手动工具面板共用。
pub struct ToolRegistry {
	inner: Mutex<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
	pub fn new() -> Self {
		Self {
			inner: Mutex::new(HashMap::new()),
		}
	}

	pub fn register(&self, tool: Arc<dyn Tool>) {
		let info = tool.info();
		self.inner.lock().unwrap().insert(info.name.clone(), tool);
	}

	pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
		self.inner.lock().unwrap().get(name).cloned()
	}

	pub fn list(&self) -> Vec<ToolInfo> {
		self.inner
			.lock()
			.unwrap()
			.values()
			.map(|tool| tool.info())
			.collect()
	}

	pub fn schema(&self, name: &str) -> Option<Value> {
		self.inner
			.lock()
			.unwrap()
			.get(name)
			.map(|tool| tool.info().params_schema)
	}

	/// 移除指定工具（供 MCP 热刷新等动态注册场景）；返回是否存在。
	pub fn remove(&self, name: &str) -> bool {
		self.inner.lock().unwrap().remove(name).is_some()
	}
}
// === AI-WORKSHOP END ===