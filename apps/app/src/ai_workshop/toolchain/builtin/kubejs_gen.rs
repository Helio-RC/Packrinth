// === AI-WORKSHOP START ===
// L2 工具链：生成 KubeJS 脚本并写入实例脚本目录（server_scripts / client_scripts）。
use std::path::PathBuf;

use async_trait::async_trait;

use super::super::toolchain_trait::{ExecutableToolchain, string_arg};
use crate::ai_workshop::other_err;
use crate::ai_workshop::tools::context::ExecutionContext;
use crate::api::Result;

/// 纯函数：选择 KubeJS 脚本子目录（同步校验类型，便于单测）。
pub fn script_dir_for_type(instance_root: &std::path::Path, script_type: &str) -> Result<PathBuf> {
	match script_type {
		"server" => Ok(instance_root.join("kubejs").join("server_scripts")),
		"client" => Ok(instance_root.join("kubejs").join("client_scripts")),
		other => Err(other_err(format!("script_type 仅支持 server / client，收到: {other}"))),
	}
}

/// 生成 KubeJS 脚本的工具链。参数：script_type（server/client，默认 server）、content 必填。
pub struct KubeJsGenToolchain;

#[async_trait]
impl ExecutableToolchain for KubeJsGenToolchain {
	fn name(&self) -> &'static str {
		"kubejs_gen"
	}

	fn description(&self) -> &'static str {
		"生成 KubeJS 脚本并写入实例 kubejs 目录"
	}

	async fn execute(
		&self,
		instance_id: Option<&str>,
		params: serde_json::Value,
		ctx: &ExecutionContext,
	) -> Result<serde_json::Value> {
		let instance_id = instance_id.ok_or_else(|| other_err("缺少 instance_id"))?;
		let script_type = params
			.get("script_type")
			.and_then(serde_json::Value::as_str)
			.unwrap_or("server");
		let content = string_arg(&params, "content")?;
		if content.trim().is_empty() {
			return Err(other_err("content 不能为空"));
		}

		let root = theseus::instance::get_full_path(instance_id).await?;
		let dir = script_dir_for_type(&root, script_type)?;
		std::fs::create_dir_all(&dir)?;
		let path = dir.join("ai_generated.js");
		ctx.report_progress("kubejs_gen".to_string(), Some(50.0), None);
		let bytes = content.len();
		std::fs::write(&path, content)?;

		Ok(serde_json::json!({
			"path": path.to_string_lossy(),
			"bytes": bytes,
			"script_type": script_type,
		}))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn script_type_mapping() {
		let root = std::path::Path::new("/inst");
		assert_eq!(
			script_dir_for_type(root, "server").unwrap(),
			root.join("kubejs").join("server_scripts")
		);
		assert_eq!(
			script_dir_for_type(root, "client").unwrap(),
			root.join("kubejs").join("client_scripts")
		);
	}

	#[test]
	fn rejects_unknown_type() {
		let root = std::path::Path::new("/inst");
		assert!(script_dir_for_type(root, "module").is_err());
	}

	#[test]
	fn metadata() {
		assert_eq!(KubeJsGenToolchain.name(), "kubejs_gen");
	}
}
// === AI-WORKSHOP END ===
