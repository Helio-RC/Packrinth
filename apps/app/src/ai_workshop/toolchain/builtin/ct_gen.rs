// === AI-WORKSHOP START ===
// L2 工具链：生成 CraftTweaker 脚本并写入实例 scripts 目录。
use async_trait::async_trait;

use super::super::toolchain_trait::{ExecutableToolchain, string_arg};
use crate::ai_workshop::other_err;
use crate::ai_workshop::tools::context::ExecutionContext;
use crate::api::Result;

/// 生成 CraftTweaker 脚本的工具链。参数：content 必填（.zs 文件内容）。
pub struct CtGenToolchain;

#[async_trait]
impl ExecutableToolchain for CtGenToolchain {
	fn name(&self) -> &'static str {
		"ct_gen"
	}

	fn description(&self) -> &'static str {
		"生成 CraftTweaker 脚本并写入实例 scripts 目录"
	}

	async fn execute(
		&self,
		instance_id: Option<&str>,
		params: serde_json::Value,
		ctx: &ExecutionContext,
	) -> Result<serde_json::Value> {
		let instance_id = instance_id.ok_or_else(|| other_err("缺少 instance_id"))?;
		let content = string_arg(&params, "content")?;
		if content.trim().is_empty() {
			return Err(other_err("content 不能为空"));
		}

		let root = theseus::instance::get_full_path(instance_id).await?;
		let dir = root.join("scripts");
		std::fs::create_dir_all(&dir)?;
		let path = dir.join("ai_generated.zs");
		ctx.report_progress("ct_gen".to_string(), Some(50.0), None);
		let bytes = content.len();
		std::fs::write(&path, content)?;

		Ok(serde_json::json!({
			"path": path.to_string_lossy(),
			"bytes": bytes,
		}))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn metadata() {
		assert_eq!(CtGenToolchain.name(), "ct_gen");
	}
}
// === AI-WORKSHOP END ===
