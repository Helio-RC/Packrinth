// === AI-WORKSHOP START ===
use std::sync::Arc;

pub mod builtin;
pub mod toolchain_trait;

pub use toolchain_trait::ToolchainRegistry;

/// 注册内置可执行工具链（KubeJS 脚本生成、FTB 配方、模组配置、打包导出等）。
/// 流 D.6 实现。
pub fn register_builtin_toolchains(registry: &Arc<ToolchainRegistry>) {
	registry.register(Arc::new(builtin::export_mods::ExportModsToolchain));
	registry.register(Arc::new(builtin::kubejs_gen::KubeJsGenToolchain));
	registry.register(Arc::new(builtin::ct_gen::CtGenToolchain));
	registry.register(Arc::new(builtin::ftb_recipe::FtbRecipeToolchain));
	registry.register(Arc::new(builtin::mod_config::ModConfigToolchain));
}
// === AI-WORKSHOP END ===