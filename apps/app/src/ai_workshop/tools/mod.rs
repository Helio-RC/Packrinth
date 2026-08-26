// === AI-WORKSHOP START ===
use std::sync::Arc;

use registry::ToolRegistry;

pub mod config_ops;
pub mod context;
pub mod mod_ops;
pub mod registry;
pub mod script_gen;

/// 注册内置原子工具（模组操作、配置读写、脚本生成、Git 等）。
/// 流 C 实现：直接调用 theseus API 完成实际功能。
pub fn register_builtin_tools(registry: &Arc<ToolRegistry>) {
	mod_ops::register_mod_ops_tools(registry);
	config_ops::register_config_ops_tools(registry);
	script_gen::register_script_gen_tools(registry);
	// TODO(流C): 注册 git 等原子工具
}
// === AI-WORKSHOP END ===