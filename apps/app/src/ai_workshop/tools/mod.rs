// === AI-WORKSHOP START ===
use std::sync::Arc;

use registry::ToolRegistry;

pub mod context;
pub mod registry;

/// 注册内置原子工具（模组操作、配置读写、脚本生成、Git 等）。
/// 流 C 实现：直接调用 theseus API 完成实际功能。
pub fn register_builtin_tools(_registry: &Arc<ToolRegistry>) {
	// TODO(流C): 注册 search_mods / install_mod / read_config / generate_kubejs_script 等原子工具
}
// === AI-WORKSHOP END ===