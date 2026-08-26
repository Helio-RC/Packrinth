// === AI-WORKSHOP START ===
// 模组操作原子工具：搜索 / 详情 / 安装 / 移除 / 更新 / 列出已装 / 列出实例。
// 直接调用 theseus API 完成实际功能，供 AI 引擎与手动工具面板共用。
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::context::ExecutionContext;
use super::registry::{Tool, ToolDomain, ToolInfo};
use theseus::data::{ContentType, ResolutionPreferences};
use theseus::instance::InstallProjectWithDependenciesRequest;

/// 从 arguments 中读取字符串参数；缺失或类型不符返回错误。
fn string_arg(arguments: &Value, key: &str) -> Result<String, String> {
	arguments
		.get(key)
		.and_then(Value::as_str)
		.map(str::to_string)
		.ok_or_else(|| format!("缺少参数: {key}"))
}

/// 搜索模组（readonly）。参数：query 必填，limit 默认 10，loader 可选。
pub struct SearchModsTool;

impl SearchModsTool {
	fn info_impl() -> ToolInfo {
		ToolInfo {
			name: "search_mods".to_string(),
			description: "在 Modrinth 搜索模组，返回匹配项目的摘要列表。".to_string(),
			domain: ToolDomain::Mods,
			requires_confirmation: false,
			is_readonly: true,
			params_schema: json!({
				"type": "object",
				"properties": {
					"query": { "type": "string", "description": "搜索关键词" },
					"limit": { "type": "integer", "default": 10, "description": "返回条数，默认 10" },
					"loader": { "type": "string", "description": "可选：按加载器过滤，如 fabric / forge" }
				},
				"required": ["query"]
			}),
		}
	}
}

#[async_trait]
impl Tool for SearchModsTool {
	fn info(&self) -> ToolInfo {
		Self::info_impl()
	}

	async fn execute(
		&self,
		arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let query = string_arg(&arguments, "query")?;
		let limit = arguments
			.get("limit")
			.and_then(Value::as_u64)
			.unwrap_or(10)
			.min(100);
		let loader = arguments.get("loader").and_then(Value::as_str);

		// 搜索 key 是 v2 search 接口的查询串（?query=...&facets=...&limit=...）
		let mut key = format!("?query={}", urlencoding::encode(&query));
		if let Some(loader) = loader {
			let facets =
				json!([["project_type:mod"], [format!("categories:{loader}")]]);
			key.push_str(&format!(
				"&facets={}",
				urlencoding::encode(&facets.to_string())
			));
		}
		key.push_str(&format!("&limit={limit}"));

		let results =
			theseus::cache::get_search_results(&key, None).await.map_err(|e| e.to_string())?;
		let Some(results) = results else {
			return Ok(json!({ "hits": [], "query": query }));
		};

		let hits: Vec<Value> = results
			.result
			.hits
			.iter()
			.map(|h| {
				json!({
					"project_id": h.project_id,
					"title": h.title,
					"slug": h.slug,
					"description": h.description,
					"downloads": h.downloads,
					"icon_url": h.icon_url,
					"categories": h.categories,
				})
			})
			.collect();

		Ok(json!({ "hits": hits, "query": query, "total_hits": results.result.total_hits }))
	}
}

/// 获取模组详情（readonly）。参数：mod_id 必填。返回 Project 全量。
pub struct GetModDetailsTool;

#[async_trait]
impl Tool for GetModDetailsTool {
	fn info(&self) -> ToolInfo {
		ToolInfo {
			name: "get_mod_details".to_string(),
			description: "获取单个模组的完整信息。".to_string(),
			domain: ToolDomain::Mods,
			requires_confirmation: false,
			is_readonly: true,
			params_schema: json!({
				"type": "object",
				"properties": {
					"mod_id": { "type": "string", "description": "Modrinth 项目 ID" }
				},
				"required": ["mod_id"]
			}),
		}
	}

	async fn execute(
		&self,
		arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let mod_id = string_arg(&arguments, "mod_id")?;
		let project =
			theseus::cache::get_project(&mod_id, None).await.map_err(|e| e.to_string())?;
		let project = project.ok_or_else(|| format!("未找到模组: {mod_id}"))?;
		serde_json::to_value(project).map_err(|e| e.to_string())
	}
}

/// 安装模组（需确认）。参数：mod_id、instance_id 必填，version_id 可选。
/// 带依赖解析，后台执行，返回解析计划。
pub struct InstallModTool;

#[async_trait]
impl Tool for InstallModTool {
	fn info(&self) -> ToolInfo {
		ToolInfo {
			name: "install_mod".to_string(),
			description: "将模组安装到指定实例（含依赖解析）。".to_string(),
			domain: ToolDomain::Mods,
			requires_confirmation: true,
			is_readonly: false,
			params_schema: json!({
				"type": "object",
				"properties": {
					"mod_id": { "type": "string", "description": "Modrinth 项目 ID" },
					"instance_id": { "type": "string", "description": "目标实例 ID" },
					"version_id": { "type": "string", "description": "可选：指定版本 ID" }
				},
				"required": ["mod_id", "instance_id"]
			}),
		}
	}

	async fn execute(
		&self,
		arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let mod_id = string_arg(&arguments, "mod_id")?;
		let instance_id = string_arg(&arguments, "instance_id")?;
		let version_id = arguments.get("version_id").and_then(Value::as_str);

		let request = InstallProjectWithDependenciesRequest {
			project_id: mod_id,
			version_id: version_id.map(str::to_string),
			content_type: ContentType::Mod,
			selected: ResolutionPreferences::default(),
		};
		let plan = theseus::instance::install_project_with_dependencies(
			&instance_id,
			request,
		)
		.await
		.map_err(|e| e.to_string())?;

		Ok(json!({
			"primary": plan.primary,
			"dependencies": plan.dependencies,
			"skipped": plan.skipped,
			"dependencies_count": plan.dependencies.len(),
		}))
	}
}

/// 在实例已安装内容中查找 mod_id 对应的相对路径（如 "mods/foo.jar"）。
async fn find_project_path(
	instance_id: &str,
	mod_id: &str,
) -> Result<String, String> {
	let projects = theseus::instance::get_projects(instance_id, None)
		.await
		.map_err(|e| e.to_string())?;
	for (path, content) in projects.into_iter() {
		if content
			.metadata
			.as_ref()
			.is_some_and(|m| m.project_id == mod_id)
		{
			return Ok(path);
		}
	}
	Err(format!("未在实例中找到该模组: {mod_id}"))
}

/// 移除模组（需确认）。参数：mod_id、instance_id 必填，keep_config 默认 true（仅记录，不删配置）。
pub struct RemoveModTool;

#[async_trait]
impl Tool for RemoveModTool {
	fn info(&self) -> ToolInfo {
		ToolInfo {
			name: "remove_mod".to_string(),
			description: "从实例中移除模组。".to_string(),
			domain: ToolDomain::Mods,
			requires_confirmation: true,
			is_readonly: false,
			params_schema: json!({
				"type": "object",
				"properties": {
					"mod_id": { "type": "string", "description": "Modrinth 项目 ID" },
					"instance_id": { "type": "string", "description": "目标实例 ID" },
					"keep_config": { "type": "boolean", "default": true, "description": "是否保留配置文件（仅记录，移除本身不删除配置）" }
				},
				"required": ["mod_id", "instance_id"]
			}),
		}
	}

	async fn execute(
		&self,
		arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let mod_id = string_arg(&arguments, "mod_id")?;
		let instance_id = string_arg(&arguments, "instance_id")?;
		let keep_config = arguments
			.get("keep_config")
			.and_then(Value::as_bool)
			.unwrap_or(true);

		let path = find_project_path(&instance_id, &mod_id).await?;
		theseus::instance::remove_project(&instance_id, &path)
			.await
			.map_err(|e| e.to_string())?;
		Ok(json!({ "removed": path, "keep_config": keep_config }))
	}
}

/// 更新模组（需确认）。参数：mod_id、instance_id 必填。
pub struct UpdateModTool;

#[async_trait]
impl Tool for UpdateModTool {
	fn info(&self) -> ToolInfo {
		ToolInfo {
			name: "update_mod".to_string(),
			description: "将已安装模组更新到最新可用版本。".to_string(),
			domain: ToolDomain::Mods,
			requires_confirmation: true,
			is_readonly: false,
			params_schema: json!({
				"type": "object",
				"properties": {
					"mod_id": { "type": "string", "description": "Modrinth 项目 ID" },
					"instance_id": { "type": "string", "description": "目标实例 ID" }
				},
				"required": ["mod_id", "instance_id"]
			}),
		}
	}

	async fn execute(
		&self,
		arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let mod_id = string_arg(&arguments, "mod_id")?;
		let instance_id = string_arg(&arguments, "instance_id")?;

		let path = find_project_path(&instance_id, &mod_id).await?;
		let new_path = theseus::instance::update_project(&instance_id, &path, None)
			.await
			.map_err(|e| e.to_string())?;
		Ok(json!({ "updated": path, "new_path": new_path }))
	}
}

/// 列出实例已安装模组（readonly）。参数：instance_id 必填。
pub struct ListInstalledModsTool;

#[async_trait]
impl Tool for ListInstalledModsTool {
	fn info(&self) -> ToolInfo {
		ToolInfo {
			name: "list_installed_mods".to_string(),
			description: "列出实例中已安装的模组。".to_string(),
			domain: ToolDomain::Mods,
			requires_confirmation: false,
			is_readonly: true,
			params_schema: json!({
				"type": "object",
				"properties": {
					"instance_id": { "type": "string", "description": "目标实例 ID" }
				},
				"required": ["instance_id"]
			}),
		}
	}

	async fn execute(
		&self,
		arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let instance_id = string_arg(&arguments, "instance_id")?;
		let projects = theseus::instance::get_projects(&instance_id, None)
			.await
			.map_err(|e| e.to_string())?;

		let mut mods: Vec<Value> = Vec::new();
		for (path, content) in projects.into_iter() {
			mods.push(json!({
				"path": path,
				"file_name": content.file_name,
				"enabled": content.enabled,
				"project_id": content.metadata.as_ref().map(|m| m.project_id.clone()),
				"version_id": content.metadata.as_ref().map(|m| m.version_id.clone()),
			}));
		}
		Ok(json!({ "mods": mods }))
	}
}

/// 列出全部实例（readonly）。无参数。
pub struct ListInstancesTool;

#[async_trait]
impl Tool for ListInstancesTool {
	fn info(&self) -> ToolInfo {
		ToolInfo {
			name: "list_instances".to_string(),
			description: "列出全部实例及其游戏版本与加载器。".to_string(),
			domain: ToolDomain::Instance,
			requires_confirmation: false,
			is_readonly: true,
			params_schema: json!({
				"type": "object",
				"properties": {}
			}),
		}
	}

	async fn execute(
		&self,
		_arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let instances = theseus::instance::list().await.map_err(|e| e.to_string())?;
		let list: Vec<Value> = instances
			.iter()
			.map(|meta| {
				json!({
					"id": meta.instance.id,
					"name": meta.instance.name,
					"game_version": meta.applied_content_set.game_version,
					"loader": meta.applied_content_set.loader,
				})
			})
			.collect();
		Ok(json!({ "instances": list }))
	}
}

/// 构造并注册全部模组操作工具。
pub fn register_mod_ops_tools(registry: &Arc<super::registry::ToolRegistry>) {
	let tools: Vec<Arc<dyn Tool>> = vec![
		Arc::new(SearchModsTool),
		Arc::new(GetModDetailsTool),
		Arc::new(InstallModTool),
		Arc::new(RemoveModTool),
		Arc::new(UpdateModTool),
		Arc::new(ListInstalledModsTool),
		Arc::new(ListInstancesTool),
	];
	for tool in tools {
		registry.register(tool);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[tokio::test]
	async fn search_mods_requires_query() {
		let tool = SearchModsTool;
		let result = tool.execute(json!({}), &ExecutionContext::default()).await;
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), "缺少参数: query");
	}

	#[tokio::test]
	async fn search_mods_rejects_wrong_query_type() {
		let tool = SearchModsTool;
		let result = tool
			.execute(json!({ "query": 42 }), &ExecutionContext::default())
			.await;
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), "缺少参数: query");
	}

	#[tokio::test]
	async fn get_mod_details_requires_mod_id() {
		let tool = GetModDetailsTool;
		let result = tool.execute(json!({}), &ExecutionContext::default()).await;
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), "缺少参数: mod_id");
	}

	#[tokio::test]
	async fn install_mod_requires_mod_id_and_instance_id() {
		let tool = InstallModTool;
		let err = tool
			.execute(json!({ "mod_id": "abc" }), &ExecutionContext::default())
			.await
			.unwrap_err();
		assert_eq!(err, "缺少参数: instance_id");
		let err = tool
			.execute(json!({ "instance_id": "i1" }), &ExecutionContext::default())
			.await
			.unwrap_err();
		assert_eq!(err, "缺少参数: mod_id");
	}

	#[tokio::test]
	async fn remove_mod_requires_mod_id() {
		let tool = RemoveModTool;
		let result = tool.execute(json!({}), &ExecutionContext::default()).await;
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), "缺少参数: mod_id");
	}

	#[tokio::test]
	async fn update_mod_requires_instance_id() {
		let tool = UpdateModTool;
		let result = tool
			.execute(json!({ "mod_id": "abc" }), &ExecutionContext::default())
			.await;
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), "缺少参数: instance_id");
	}

	#[tokio::test]
	async fn list_installed_mods_requires_instance_id() {
		let tool = ListInstalledModsTool;
		let result = tool.execute(json!({}), &ExecutionContext::default()).await;
		assert!(result.is_err());
		assert_eq!(result.unwrap_err(), "缺少参数: instance_id");
	}

	#[test]
	fn search_mods_default_limit_and_schema() {
		let tool = SearchModsTool;
		assert!(!tool.requires_confirmation());
		assert!(tool.info().is_readonly);
		let schema = tool.info().params_schema;
		assert_eq!(schema["required"][0], json!("query"));
		assert_eq!(schema["properties"]["limit"]["default"], json!(10));
	}
}
// === AI-WORKSHOP END ===