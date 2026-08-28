// === AI-WORKSHOP START ===
// L2 工具链：为实例模组生成配置骨架（config/<mod_id>/ai_workshop.json）。
use async_trait::async_trait;

use super::super::toolchain_trait::{ExecutableToolchain, string_arg};
use crate::ai_workshop::other_err;
use crate::ai_workshop::tools::context::ExecutionContext;
use crate::api::Result;

/// 纯函数：生成配置骨架 JSON 内容（便于单测）。
pub fn skeleton_json(mod_id: &str) -> serde_json::Value {
	serde_json::json!({
		"modPack": {
			"projectId": mod_id,
			"loadedAt": chrono::Utc::now().to_rfc3339(),
			"note": "AI 生成的默认配置骨架，可手动编辑"
		}
	})
}

/// 生成模组配置文件骨架的工具链。参数：mod_id 必填（Modrinth 项目 ID）。
pub struct ModConfigToolchain;

#[async_trait]
impl ExecutableToolchain for ModConfigToolchain {
	fn name(&self) -> &'static str {
		"mod_config"
	}

	fn description(&self) -> &'static str {
		"为实例生成模组默认配置文件骨架（config/<mod_id>/ai_workshop.json）"
	}

	async fn execute(
		&self,
		instance_id: Option<&str>,
		params: serde_json::Value,
		ctx: &ExecutionContext,
	) -> Result<serde_json::Value> {
		let instance_id = instance_id.ok_or_else(|| other_err("缺少 instance_id"))?;
		let mod_id = string_arg(&params, "mod_id")?;

		let root = theseus::instance::get_full_path(instance_id).await?;
		let dir = root.join("config").join(&mod_id);
		std::fs::create_dir_all(&dir)?;
		let path = dir.join("ai_workshop.json");
		let content = skeleton_json(&mod_id);
		let raw = serde_json::to_string_pretty(&content)
			.map_err(|e| other_err(format!("配置序列化失败: {e}")))?;
		ctx.report_progress("mod_config".to_string(), Some(50.0), None);
		std::fs::write(&path, raw)?;

		Ok(serde_json::json!({
			"path": path.to_string_lossy(),
			"content": content,
		}))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn skeleton_contains_project_id() {
		let value = skeleton_json("jei");
		assert_eq!(value["modPack"]["projectId"], serde_json::json!("jei"));
		assert!(value["modPack"]["loadedAt"].as_str().is_some());
	}

	#[test]
	fn metadata() {
		assert_eq!(ModConfigToolchain.name(), "mod_config");
	}
}
// === AI-WORKSHOP END ===
