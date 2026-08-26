// === AI-WORKSHOP START ===
use std::sync::Arc;

pub mod toolchain_trait;

pub use toolchain_trait::ToolchainRegistry;

/// 注册内置可执行工具链（KubeJS 脚本生成、FTB 配方、模组配置、打包导出等）。
/// 流 D.6 实现。
pub fn register_builtin_toolchains(_registry: &Arc<ToolchainRegistry>) {
	// TODO(流D): 注册内置工具链
}
// === AI-WORKSHOP END ===