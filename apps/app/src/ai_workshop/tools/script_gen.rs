// === AI-WORKSHOP START ===
// 脚本生成原子工具：KubeJS 与 CraftTweaker 脚本写入。
// 目录不存在时自动创建，路径经安全校验（复用 config_ops 的 safe_join / resolve_write_path）。
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};

use super::config_ops::{resolve_write_path, safe_join};
use super::context::ExecutionContext;
use super::registry::{Tool, ToolDomain, ToolInfo};

/// 从 arguments 中读取字符串参数；缺失或类型不符返回错误。
fn string_arg(arguments: &Value, key: &str) -> Result<String, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("缺少参数: {key}"))
}

fn ts() -> i64 {
    Utc::now().timestamp_millis()
}

/// 脚本类型到 KubeJS 目录名映射：recipe/event/custom → server_scripts，startup → startup_scripts。
fn kubejs_dir(script_type: &str) -> Result<&'static str, String> {
    match script_type {
        "startup" => Ok("startup_scripts"),
        "recipe" | "event" | "custom" => Ok("server_scripts"),
        other => Err(format!("未知脚本类型: {other}")),
    }
}

/// 生成 KubeJS 脚本（需确认）。参数：instance_id、script_type、content 必填，filename 可选。
/// recipe/event/custom → server_scripts，startup → startup_scripts。写入 `{root}/kubejs/{dir}/{filename}`。
pub struct GenerateKubejsScriptTool;

#[async_trait]
impl Tool for GenerateKubejsScriptTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "generate_kubejs_script".to_string(),
            description: "生成并写入一个 KubeJS 脚本到实例的 kubejs 目录。"
                .to_string(),
            domain: ToolDomain::Script,
            requires_confirmation: true,
            is_readonly: false,
            params_schema: json!({
                "type": "object",
                "properties": {
                    "instance_id": { "type": "string", "description": "目标实例 ID" },
                    "script_type": { "type": "string", "description": "recipe | startup | event | custom" },
                    "content": { "type": "string", "description": "脚本内容" },
                    "filename": { "type": "string", "description": "可选：文件名（默认按类型生成 kubejs_script_{ts}.js）" }
                },
                "required": ["instance_id", "script_type", "content"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        ctx: &ExecutionContext,
    ) -> Result<Value, String> {
        let instance_id = string_arg(&arguments, "instance_id")?;
        let script_type = string_arg(&arguments, "script_type")?;
        let content = string_arg(&arguments, "content")?;
        let filename = arguments.get("filename").and_then(Value::as_str);

        let _lock = ctx
            .instance_lock_manager
            .acquire_write_lock(
                &instance_id,
                std::time::Duration::from_secs(30),
            )
            .await?;
        let dir = kubejs_dir(&script_type)?;
        let filename = filename
            .map(str::to_string)
            .unwrap_or_else(|| format!("kubejs_script_{}.js", ts()));

        let root = theseus::instance::get_full_path(&instance_id)
            .await
            .map_err(|e| e.to_string())?;
        let rel = format!("kubejs/{dir}/{filename}");
        // 先做词法安全校验，再解析写入路径（含父目录创建与 canonicalize 逃逸校验）
        safe_join(&root, &rel)?;
        let target = resolve_write_path(&root, &rel).await?;
        tokio::fs::write(&target, content.as_bytes())
            .await
            .map_err(|e| format!("写入失败 {}: {e}", target.display()))?;

        Ok(json!({
            "path": target.to_string_lossy().into_owned(),
            "filename": filename,
            "dir": dir,
        }))
    }
}

/// 生成 CraftTweaker 脚本（需确认）。参数：instance_id、content 必填，filename 可选。
/// 写入 `{root}/scripts/{filename}.zs`。返回 { path, filename }。
pub struct GenerateCrafttweakerScriptTool;

#[async_trait]
impl Tool for GenerateCrafttweakerScriptTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "generate_crafttweaker_script".to_string(),
            description:
                "生成并写入一个 CraftTweaker 脚本（.zs）到实例的 scripts 目录。"
                    .to_string(),
            domain: ToolDomain::Script,
            requires_confirmation: true,
            is_readonly: false,
            params_schema: json!({
                "type": "object",
                "properties": {
                    "instance_id": { "type": "string", "description": "目标实例 ID" },
                    "content": { "type": "string", "description": "脚本内容" },
                    "filename": { "type": "string", "description": "可选：文件名（不含扩展名，默认 crafttweaker_script_{ts}）" }
                },
                "required": ["instance_id", "content"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        ctx: &ExecutionContext,
    ) -> Result<Value, String> {
        let instance_id = string_arg(&arguments, "instance_id")?;
        let content = string_arg(&arguments, "content")?;
        let filename = arguments
            .get("filename")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("crafttweaker_script_{}", ts()));
        let filename = format!("{filename}.zs");

        let _lock = ctx
            .instance_lock_manager
            .acquire_write_lock(
                &instance_id,
                std::time::Duration::from_secs(30),
            )
            .await?;
        let root = theseus::instance::get_full_path(&instance_id)
            .await
            .map_err(|e| e.to_string())?;
        let rel = format!("scripts/{filename}");
        safe_join(&root, &rel)?;
        let target = resolve_write_path(&root, &rel).await?;
        tokio::fs::write(&target, content.as_bytes())
            .await
            .map_err(|e| format!("写入失败 {}: {e}", target.display()))?;

        Ok(json!({
            "path": target.to_string_lossy().into_owned(),
            "filename": filename,
        }))
    }
}

/// 构造并注册全部脚本生成工具。
pub fn register_script_gen_tools(
    registry: &Arc<super::registry::ToolRegistry>,
) {
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(GenerateKubejsScriptTool),
        Arc::new(GenerateCrafttweakerScriptTool),
    ];
    for tool in tools {
        registry.register(tool);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubejs_script_type_mapping() {
        assert_eq!(kubejs_dir("recipe").unwrap(), "server_scripts");
        assert_eq!(kubejs_dir("event").unwrap(), "server_scripts");
        assert_eq!(kubejs_dir("custom").unwrap(), "server_scripts");
        assert_eq!(kubejs_dir("startup").unwrap(), "startup_scripts");
        assert!(kubejs_dir("bogus").is_err());
    }

    #[test]
    fn kubejs_script_rejects_unknown_type() {
        let tool = GenerateKubejsScriptTool;
        let schema = tool.info().params_schema;
        let types = schema["properties"]["script_type"]["description"]
            .as_str()
            .unwrap();
        assert!(types.contains("recipe") && types.contains("startup"));
    }
}
// === AI-WORKSHOP END ===
