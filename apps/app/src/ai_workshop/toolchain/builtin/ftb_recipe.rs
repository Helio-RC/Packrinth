// === AI-WORKSHOP START ===
// L2 工具链：从配方 JSON 渲染 KubeJS / CraftTweaker 配方代码并写入实例。
use async_trait::async_trait;

use super::super::toolchain_trait::{ExecutableToolchain, string_arg};
use crate::ai_workshop::other_err;
use crate::ai_workshop::tools::context::ExecutionContext;
use crate::api::Result;

/// 纯函数：将配方 JSON 渲染为 KubeJS / CraftTweaker 配方代码（shaped 3x3）。
pub fn render_recipe(recipe: &serde_json::Value, format: &str) -> Result<String> {
	let output = string_arg(recipe, "output")?;
	let ingredients: Vec<Option<String>> = recipe
		.get("ingredients")
		.and_then(serde_json::Value::as_array)
		.map(|arr| arr.iter().map(|slot| slot.as_str().map(str::to_string)).collect())
		.unwrap_or_default();
	if output.is_empty() {
		return Err(other_err("配方缺少 output（物品 ID）"));
	}
	if ingredients.len() != 9 {
		return Err(other_err("配方需要 9 个槽位（3x3），缺口用 null 占位"));
	}

	let header = match format {
		"kubejs" => format!(
			"// AI generated recipe\nServerEvents.recipes(event => {{\n  event.shaped('{output}', [\n"
		),
		"crafttweaker" => {
			format!("# AI generated recipe\nrecipe = crafting_builder.shaped('{output}', [\n")
		}
		other => return Err(other_err(format!("format 仅支持 kubejs / crafttweaker，收到: {other}"))),
	};
	let mut rendered_rows = Vec::new();
	for row in 0..3 {
		let cells: Vec<String> = (0..3)
			.map(|col| {
				ingredients
					.get(row * 3 + col)
					.and_then(|slot| slot.as_deref())
					.unwrap_or("minecraft:air")
					.to_string()
			})
			.collect();
		rendered_rows.push(format!("    [{}],", cells.join(", ")));
	}
	let tail = match format {
		"kubejs" => "  ]\n});\n".to_string(),
		"crafttweaker" => "  ]\n  .build(recipe);\n".to_string(),
		_ => unreachable!(),
	};
	let mut result = header;
	result.push_str(&rendered_rows.join("\n"));
	result.push('\n');
	result.push_str(&tail);
	Ok(result)
}

/// 从配方 JSON 生成脚本的工具链。参数：recipe（{output, ingredients[9]}）必填、
/// format（kubejs / crafttweaker，默认 kubejs）。
pub struct FtbRecipeToolchain;

#[async_trait]
impl ExecutableToolchain for FtbRecipeToolchain {
	fn name(&self) -> &'static str {
		"ftb_recipe"
	}

	fn description(&self) -> &'static str {
		"从配方 JSON 渲染并写入 KubeJS / CraftTweaker 配方脚本"
	}

	async fn execute(
		&self,
		instance_id: Option<&str>,
		params: serde_json::Value,
		ctx: &ExecutionContext,
	) -> Result<serde_json::Value> {
		let instance_id = instance_id.ok_or_else(|| other_err("缺少 instance_id"))?;
		let format = params
			.get("format")
			.and_then(serde_json::Value::as_str)
			.unwrap_or("kubejs");
		let recipe = params
			.get("recipe")
			.ok_or_else(|| other_err("缺少配方参数: recipe"))?;
		recipe.as_object().ok_or_else(|| other_err("recipe 必须为 JSON 对象"))?;
		let content = render_recipe(recipe, format)?;

		let root = theseus::instance::get_full_path(instance_id).await?;
		let (dir, file_name) = match format {
			"kubejs" => (root.join("kubejs").join("server_scripts"), "ai_recipe.js"),
			"crafttweaker" => (root.join("scripts"), "ai_recipe.zs"),
			_ => unreachable!(),
		};
		std::fs::create_dir_all(&dir)?;
		let path = dir.join(file_name);
		ctx.report_progress("ftb_recipe".to_string(), Some(50.0), None);
		let bytes = content.len();
		std::fs::write(&path, content)?;

		Ok(serde_json::json!({
			"path": path.to_string_lossy(),
			"format": format,
			"bytes": bytes,
		}))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn recipe() -> serde_json::Value {
		serde_json::json!({
			"output": "minecraft:diamond",
			"ingredients": [
				"minecraft:iron_ingot", "minecraft:iron_ingot", "minecraft:iron_ingot",
				"minecraft:iron_ingot", null, "minecraft:iron_ingot",
				"minecraft:iron_ingot", "minecraft:iron_ingot", "minecraft:iron_ingot"
			]
		})
	}

	#[test]
	fn renders_kubejs_shaped_recipe() {
		let code = render_recipe(&recipe(), "kubejs").unwrap();
		assert!(code.contains("event.shaped('minecraft:diamond'"));
		assert!(code.contains("minecraft:iron_ingot"));
		assert!(code.contains("minecraft:air"), "null 槽位回退 air");
	}

	#[test]
	fn renders_crafttweaker_recipe() {
		let code = render_recipe(&recipe(), "crafttweaker").unwrap();
		assert!(code.contains("crafting_builder.shaped('minecraft:diamond'"));
		assert!(code.contains(".build(recipe);"));
	}

	#[test]
	fn rejects_missing_output() {
		let r = serde_json::json!({ "ingredients": [] });
		assert!(render_recipe(&r, "kubejs").is_err());
	}

	#[test]
	fn rejects_bad_format() {
		assert!(render_recipe(&recipe(), "datapack").is_err());
	}

	#[test]
	fn metadata() {
		assert_eq!(FtbRecipeToolchain.name(), "ftb_recipe");
	}
}
// === AI-WORKSHOP END ===
