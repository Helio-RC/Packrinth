// === AI-WORKSHOP START ===
// 配置读写原子工具：读取 / 写入 / 回滚 / 列出 / 对比实例配置文件。
// 全部文件操作经路径安全检查（canonicalize + 实例根前缀校验），防 ../ 逃逸。
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};

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

/// 词法规范化路径：折叠 `.` 与 `..` 段，便于在文件不存在时做前缀逃逸检测。
fn normalize_path(path: &Path) -> PathBuf {
	let mut out = PathBuf::new();
	for comp in path.components() {
		match comp {
			Component::CurDir => {}
			Component::ParentDir => {
				out.pop();
			}
			other => out.push(other.as_os_str()),
		}
	}
	out
}

/// 在实例根内安全拼接相对路径：拒绝绝对路径与逃逸实例根的 `..`，返回规范化后的绝对路径。
/// 不要求目标存在（供写入新文件使用）。
pub(crate) fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
	let rel_path = Path::new(rel);
	if rel_path.is_absolute() {
		return Err("路径不能为绝对路径".to_string());
	}
	let joined = root.join(rel_path);
	let normalized = normalize_path(&joined);
	if !normalized.starts_with(root) {
		return Err(format!("不安全路径，已逃逸实例根目录: {rel}"));
	}
	Ok(normalized)
}

/// 解析实例根目录为规范（canonical）路径，供前缀包含校验使用。
async fn canonical_root(root: &Path) -> Result<PathBuf, String> {
	tokio::fs::canonicalize(root)
		.await
		.map_err(|e| format!("无法解析实例根目录 {}: {e}", root.display()))
}

/// canonicalize 给定路径并校验其仍位于实例根内，防止符号链接逃逸。
async fn canonicalize_within(root: &Path, path: &Path) -> Result<PathBuf, String> {
	let canonical_root = canonical_root(root).await?;
	let canonical = tokio::fs::canonicalize(path)
		.await
		.map_err(|e| format!("无法解析路径 {}: {e}", path.display()))?;
	if !canonical.starts_with(&canonical_root) {
		return Err(format!("不安全路径，已逃逸实例根目录: {}", path.display()));
	}
	Ok(canonical)
}

/// 解析到实例根的绝对路径（供读取已存在文件的工具使用）。
/// canonicalize 后必须仍位于实例根内，防止符号链接逃逸。
async fn resolve_instance_path(root: &Path, rel_path: &str) -> Result<PathBuf, String> {
	let joined = safe_join(root, rel_path)?;
	canonicalize_within(root, &joined).await
}

/// 解析到实例根的写入路径：目标可不存在，先创建父目录并 canonicalize 父目录做逃逸校验。
/// 返回 canonicalize 父目录 + 文件名 后的绝对路径。
pub(crate) async fn resolve_write_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
	let target = safe_join(root, rel)?;
	let parent = target
		.parent()
		.ok_or_else(|| format!("无效路径: {rel}"))?;
	tokio::fs::create_dir_all(parent)
		.await
		.map_err(|e| format!("无法创建目录 {}: {e}", parent.display()))?;
	let canonical_root = canonical_root(root).await?;
	let canonical_parent = tokio::fs::canonicalize(parent)
		.await
		.map_err(|e| format!("无法解析目录 {}: {e}", parent.display()))?;
	if !canonical_parent.starts_with(&canonical_root) {
		return Err(format!("不安全路径，已逃逸实例根目录: {rel}"));
	}
	let file_name = target
		.file_name()
		.ok_or_else(|| format!("无效路径: {rel}"))?;
	let full = canonical_parent.join(file_name);
	// 若目标最终组件已存在，拒绝符号链接并校验其仍在实例根内；
	// 若不存在（全新文件），父目录已 canonical，直接返回即可。
	match tokio::fs::symlink_metadata(&full).await {
		Ok(meta) => {
			if meta.file_type().is_symlink() {
				return Err(format!(
					"目标为符号链接，拒绝写入以防范逃逸: {rel}"
				));
			}
			canonicalize_within(&canonical_root, &full).await
		}
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(full),
		Err(e) => Err(format!("无法检查目标 {}: {e}", full.display())),
	}
}

/// 读取文件内容（UTF-8，非 UTF-8 时按 lossy 处理）。
async fn read_text(path: &Path) -> Result<String, String> {
	match tokio::fs::read_to_string(path).await {
		Ok(s) => Ok(s),
		Err(_) => {
			let bytes = tokio::fs::read(path)
				.await
				.map_err(|e| format!("无法读取 {}: {e}", path.display()))?;
			Ok(String::from_utf8_lossy(&bytes).into_owned())
		}
	}
}

/// 备份文件名前缀：`{file_name}.backup.bak-`。
fn backup_prefix(file_name: &str) -> String {
	format!("{file_name}.backup.bak-")
}

/// 列出某配置文件的所有备份文件（按时间戳降序，最新的在前）。
async fn list_backups(root: &Path, rel: &str) -> Result<Vec<PathBuf>, String> {
	let target = safe_join(root, rel)?;
	let parent = target
		.parent()
		.ok_or_else(|| format!("无效路径: {rel}"))?;
	let file_name = target
		.file_name()
		.ok_or_else(|| format!("无效路径: {rel}"))?;
	let prefix = backup_prefix(&file_name.to_string_lossy());

	let mut dir = tokio::fs::read_dir(parent)
		.await
		.map_err(|e| format!("无法读取目录 {}: {e}", parent.display()))?;
	let mut backups = Vec::new();
	while let Some(entry) = dir.next_entry().await.map_err(|e| e.to_string())? {
		let name = entry.file_name();
		let name = name.to_string_lossy();
		if name.starts_with(&prefix) && entry.path().is_file() {
			backups.push(entry.path());
		}
	}
	backups.sort_by_key(|p| p.to_string_lossy().to_string());
	backups.reverse();
	Ok(backups)
}

/// 在目标同目录生成备份：复制原文件到 `{file_name}.backup.bak-{ts}`。
/// 若目标文件不存在（全新文件），跳过备份并返回 None。
async fn make_backup(root: &Path, rel: &str) -> Result<Option<PathBuf>, String> {
	let target = safe_join(root, rel)?;
	match tokio::fs::symlink_metadata(&target).await {
		Ok(_) => {}
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
		Err(e) => return Err(format!("无法检查目标 {}: {e}", target.display())),
	}
	let parent = target
		.parent()
		.ok_or_else(|| format!("无效路径: {rel}"))?;
	let file_name = target
		.file_name()
		.ok_or_else(|| format!("无效路径: {rel}"))?;
	let ts = Utc::now().timestamp_millis();
	let backup = parent.join(format!("{}{ts}", backup_prefix(&file_name.to_string_lossy())));
	tokio::fs::copy(&target, &backup)
		.await
		.map_err(|e| format!("备份失败 {}: {e}", backup.display()))?;
	Ok(Some(backup))
}

/// 简单逐行 diff（LCS 简化版），返回带 + / - / 前缀的行与是否发生变更。
fn diff_lines(a: &[String], b: &[String]) -> (Vec<String>, bool) {
	let n = a.len();
	let m = b.len();
	// dp[i][j] = a[0..i] 与 b[0..j] 的 LCS 长度
	let mut dp = vec![vec![0usize; m + 1]; n + 1];
	for i in (0..n).rev() {
		for j in (0..m).rev() {
			dp[i][j] = if a[i] == b[j] {
				dp[i + 1][j + 1] + 1
			} else {
				dp[i + 1][j].max(dp[i][j + 1])
			};
		}
	}
	let mut out = Vec::new();
	let mut i = 0;
	let mut j = 0;
	let mut changed = false;
	while i < n && j < m {
		if a[i] == b[j] {
			out.push(format!("  {}", a[i]));
			i += 1;
			j += 1;
		} else if dp[i + 1][j] >= dp[i][j + 1] {
			out.push(format!("- {}", a[i]));
			changed = true;
			i += 1;
		} else {
			out.push(format!("+ {}", b[j]));
			changed = true;
			j += 1;
		}
	}
	while i < n {
		out.push(format!("- {}", a[i]));
		changed = true;
		i += 1;
	}
	while j < m {
		out.push(format!("+ {}", b[j]));
		changed = true;
		j += 1;
	}
	(out, changed)
}

/// 读取配置文件（readonly）。参数：instance_id、path 必填。返回 { content, size }。
pub struct ReadConfigTool;

impl ReadConfigTool {
	fn info_impl() -> ToolInfo {
		ToolInfo {
			name: "read_config".to_string(),
			description: "读取实例内配置文件的文本内容与字节大小。".to_string(),
			domain: ToolDomain::Config,
			requires_confirmation: false,
			is_readonly: true,
			params_schema: json!({
				"type": "object",
				"properties": {
					"instance_id": { "type": "string", "description": "目标实例 ID" },
					"path": { "type": "string", "description": "相对实例根的配置路径，如 config/jei.toml" }
				},
				"required": ["instance_id", "path"]
			}),
		}
	}
}

#[async_trait]
impl Tool for ReadConfigTool {
	fn info(&self) -> ToolInfo {
		Self::info_impl()
	}

	async fn execute(
		&self,
		arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let instance_id = string_arg(&arguments, "instance_id")?;
		let path = string_arg(&arguments, "path")?;
		let root = theseus::instance::get_full_path(&instance_id)
			.await
			.map_err(|e| e.to_string())?;
		let full = resolve_instance_path(&root, &path).await?;
		let content = read_text(&full).await?;
		let size = tokio::fs::metadata(&full)
			.await
			.map(|m| m.len())
			.unwrap_or(0);
		Ok(json!({ "content": content, "size": size, "path": path }))
	}
}

/// 写入配置文件（需确认）。参数：instance_id、path、content 必填，backup 默认 true。
/// 写入前可选备份原文件到 `{path}.backup.bak-{ts}`。返回 { content, backup_path }。
pub struct WriteConfigTool;

#[async_trait]
impl Tool for WriteConfigTool {
	fn info(&self) -> ToolInfo {
		ToolInfo {
			name: "write_config".to_string(),
			description: "写入配置文件内容；可选先备份原文件。".to_string(),
			domain: ToolDomain::Config,
			requires_confirmation: true,
			is_readonly: false,
			params_schema: json!({
				"type": "object",
				"properties": {
					"instance_id": { "type": "string", "description": "目标实例 ID" },
					"path": { "type": "string", "description": "相对实例根的配置路径，如 config/jei.toml" },
					"content": { "type": "string", "description": "要写入的完整文件内容" },
					"backup": { "type": "boolean", "default": true, "description": "是否在覆盖前备份原文件" }
				},
				"required": ["instance_id", "path", "content"]
			}),
		}
	}

	async fn execute(
		&self,
		arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let instance_id = string_arg(&arguments, "instance_id")?;
		let path = string_arg(&arguments, "path")?;
		let content = string_arg(&arguments, "content")?;
		let backup = arguments.get("backup").and_then(Value::as_bool).unwrap_or(true);

		let root = theseus::instance::get_full_path(&instance_id)
			.await
			.map_err(|e| e.to_string())?;
		let backup_path = if backup {
			make_backup(&root, &path).await?
		} else {
			None
		};

		let target = resolve_write_path(&root, &path).await?;
		tokio::fs::write(&target, content.as_bytes())
			.await
			.map_err(|e| format!("写入失败 {}: {e}", target.display()))?;

		Ok(json!({
			"content": content,
			"backup_path": backup_path.map(|p| p.to_string_lossy().into_owned()),
			"path": path,
		}))
	}
}

/// 回滚配置文件（需确认）。参数：instance_id、path 必填，backup_id 可选。
/// backup_id 缺省取最新 .bak-*。返回 { restored_from }。
pub struct RollbackConfigTool;

#[async_trait]
impl Tool for RollbackConfigTool {
	fn info(&self) -> ToolInfo {
		ToolInfo {
			name: "rollback_config".to_string(),
			description: "从备份恢复配置文件内容。".to_string(),
			domain: ToolDomain::Config,
			requires_confirmation: true,
			is_readonly: false,
			params_schema: json!({
				"type": "object",
				"properties": {
					"instance_id": { "type": "string", "description": "目标实例 ID" },
					"path": { "type": "string", "description": "相对实例根的配置路径" },
					"backup_id": { "type": "string", "description": "可选：指定备份文件名（缺省取最新备份）" }
				},
				"required": ["instance_id", "path"]
			}),
		}
	}

	async fn execute(
		&self,
		arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let instance_id = string_arg(&arguments, "instance_id")?;
		let path = string_arg(&arguments, "path")?;
		let backup_id = arguments.get("backup_id").and_then(Value::as_str);

		let root = theseus::instance::get_full_path(&instance_id)
			.await
			.map_err(|e| e.to_string())?;
		Self::rollback_config_impl(&root, &path, backup_id).await
	}
}

impl RollbackConfigTool {
	/// 核心回滚逻辑：读取备份（canonicalize + 实例根包含校验后）并写回目标文件。
	async fn rollback_config_impl(
		root: &Path,
		path: &str,
		backup_id: Option<&str>,
	) -> Result<Value, String> {
		let source = if let Some(bid) = backup_id {
			let joined = safe_join(root, bid)?;
			canonicalize_within(root, &joined).await?
		} else {
			let backups = list_backups(root, path).await?;
			let joined = backups
				.first()
				.cloned()
				.ok_or_else(|| format!("无可用备份可回滚: {path}"))?;
			canonicalize_within(root, &joined).await?
		};

		let content = read_text(&source).await?;
		let target = resolve_write_path(root, path).await?;
		tokio::fs::write(&target, content.as_bytes())
			.await
			.map_err(|e| format!("回滚写入失败 {}: {e}", target.display()))?;

		Ok(json!({
			"restored_from": source.to_string_lossy().into_owned(),
			"path": path,
		}))
	}
}

/// 列出配置目录（readonly）。参数：instance_id 必填，dir 默认 "config"。
/// 递归列出文件（相对路径 + 大小），上限 500 个。
pub struct ListConfigsTool;

#[async_trait]
impl Tool for ListConfigsTool {
	fn info(&self) -> ToolInfo {
		ToolInfo {
			name: "list_configs".to_string(),
			description: "递归列出实例内配置目录下的文件（相对路径与大小），上限 500 个。".to_string(),
			domain: ToolDomain::Config,
			requires_confirmation: false,
			is_readonly: true,
			params_schema: json!({
				"type": "object",
				"properties": {
					"instance_id": { "type": "string", "description": "目标实例 ID" },
					"dir": { "type": "string", "default": "config", "description": "相对实例根的目录，默认 config" }
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
		let dir = arguments
			.get("dir")
			.and_then(Value::as_str)
			.unwrap_or("config")
			.to_string();

		let root = theseus::instance::get_full_path(&instance_id)
			.await
			.map_err(|e| e.to_string())?;
		let base = safe_join(&root, &dir)?;

		let mut files = Vec::new();
		let mut stack = vec![(base.clone(), PathBuf::from(&dir))];
		while let Some((abs, rel)) = stack.pop() {
			let mut rd = tokio::fs::read_dir(&abs)
				.await
				.map_err(|e| format!("无法读取目录 {}: {e}", abs.display()))?;
			while let Some(entry) = rd.next_entry().await.map_err(|e| e.to_string())? {
				if files.len() >= 500 {
					break;
				}
				let file_type = entry
					.file_type()
					.await
					.map_err(|e| format!("无法读取文件类型: {e}"))?;
				if file_type.is_symlink() {
					continue;
				}
				let name = entry.file_name().to_string_lossy().into_owned();
				let child_rel = rel.join(&name);
				if file_type.is_dir() {
					stack.push((entry.path(), child_rel));
				} else if file_type.is_file() {
					let size = tokio::fs::metadata(entry.path())
						.await
						.map(|m| m.len())
						.unwrap_or(0);
					files.push(json!({
						"path": child_rel.to_string_lossy().into_owned(),
						"size": size,
					}));
				}
			}
			if files.len() >= 500 {
				break;
			}
		}
		files.sort_by(|a, b| {
			a["path"]
				.as_str()
				.unwrap_or("")
				.cmp(b["path"].as_str().unwrap_or(""))
		});
		Ok(json!({ "files": files, "count": files.len(), "dir": dir }))
	}
}

/// 对比配置文件与最新备份（readonly）。参数：instance_id、path 必填。
/// 返回 { diff: [行], changed: bool }。无备份返回 Err。
pub struct DiffConfigTool;

#[async_trait]
impl Tool for DiffConfigTool {
	fn info(&self) -> ToolInfo {
		ToolInfo {
			name: "diff_config".to_string(),
			description: "对比配置文件当前内容与最新备份，返回逐行差异。".to_string(),
			domain: ToolDomain::Config,
			requires_confirmation: false,
			is_readonly: true,
			params_schema: json!({
				"type": "object",
				"properties": {
					"instance_id": { "type": "string", "description": "目标实例 ID" },
					"path": { "type": "string", "description": "相对实例根的配置路径" }
				},
				"required": ["instance_id", "path"]
			}),
		}
	}

	async fn execute(
		&self,
		arguments: Value,
		_ctx: &ExecutionContext,
	) -> Result<Value, String> {
		let instance_id = string_arg(&arguments, "instance_id")?;
		let path = string_arg(&arguments, "path")?;

		let root = theseus::instance::get_full_path(&instance_id)
			.await
			.map_err(|e| e.to_string())?;
		Self::diff_config_impl(&root, &path).await
	}
}

impl DiffConfigTool {
	/// 核心 diff 逻辑：当前文件与最新备份对比；备份经 canonicalize + 实例根包含校验。
	/// 无备份返回 Err。
	async fn diff_config_impl(root: &Path, path: &str) -> Result<Value, String> {
		let current_path = resolve_instance_path(root, path).await?;
		let backups = list_backups(root, path).await?;
		let joined = backups
			.first()
			.ok_or_else(|| format!("无备份可对比: {path}"))?;
		let backup_path = canonicalize_within(root, joined).await?;

		let current = read_text(&current_path).await?;
		let backup = read_text(&backup_path).await?;
		let a: Vec<String> = current.lines().map(str::to_string).collect();
		let b: Vec<String> = backup.lines().map(str::to_string).collect();
		let (diff, changed) = diff_lines(&a, &b);
		Ok(json!({ "diff": diff, "changed": changed, "path": path }))
	}
}

/// 构造并注册全部配置读写工具。
pub fn register_config_ops_tools(registry: &Arc<super::registry::ToolRegistry>) {
	let tools: Vec<Arc<dyn Tool>> = vec![
		Arc::new(ReadConfigTool),
		Arc::new(WriteConfigTool),
		Arc::new(RollbackConfigTool),
		Arc::new(ListConfigsTool),
		Arc::new(DiffConfigTool),
	];
	for tool in tools {
		registry.register(tool);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	/// 在临时目录下搭一个模拟实例根（config/ 目录），返回根路径。
	async fn temp_instance_root() -> PathBuf {
		let root = std::env::temp_dir().join(format!(
			"config_ops_test_{}",
			Utc::now().timestamp_nanos_opt().unwrap_or_default()
		));
		tokio::fs::create_dir_all(root.join("config")).await.unwrap();
		tokio::fs::create_dir_all(root.join("scripts")).await.unwrap();
		root
	}

	#[test]
	fn safe_join_rejects_absolute_path() {
		let root = Path::new("/tmp/instance");
		assert!(safe_join(root, "/etc/passwd").is_err());
	}

	#[test]
	fn safe_join_rejects_traversal() {
		let root = Path::new("/tmp/instance");
		assert!(safe_join(root, "../outside").is_err());
		assert!(safe_join(root, "config/../../outside").is_err());
	}

	#[test]
	fn safe_join_accepts_within_root() {
		let root = Path::new("/tmp/instance");
		let joined = safe_join(root, "config/jei.toml").unwrap();
		assert_eq!(joined, Path::new("/tmp/instance/config/jei.toml"));
		// 折叠内部 `.`
		let joined = safe_join(root, "config/./jei.toml").unwrap();
		assert_eq!(joined, Path::new("/tmp/instance/config/jei.toml"));
	}

	#[tokio::test]
	async fn diff_config_no_backup_returns_error() {
		let root = temp_instance_root().await;
		tokio::fs::write(root.join("config").join("jei.toml"), "a=1\n")
			.await
			.unwrap();
		// 直接调用工具核心执行路径：临时实例根内无任何备份，应返回 Err。
		let res = DiffConfigTool::diff_config_impl(&root, "config/jei.toml").await;
		assert!(res.is_err(), "无备份时应返回 Err，实际: {res:?}");
	}

	#[tokio::test]
	async fn make_backup_skips_missing_target() {
		let root = temp_instance_root().await;
		// 全新文件（默认 backup=true 分支）不应报错，且不生成备份。
		let backup = make_backup(&root, "config/new.toml").await.unwrap();
		assert!(backup.is_none(), "目标不存在时应跳过备份");
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn canonicalize_within_rejects_symlink_escape() {
		use std::os::unix::fs::symlink;

		let root = temp_instance_root().await;
		let outside = std::env::temp_dir().join(format!(
			"config_ops_outside_{}",
			Utc::now().timestamp_nanos_opt().unwrap_or_default()
		));
		tokio::fs::write(&outside, "secret\n").await.unwrap();
		let link = root.join("config").join("evil.toml.backup.bak-1");
		symlink(&outside, &link).unwrap();

		// 读取备份路径时必须拒绝指向实例根外的符号链接。
		let res = canonicalize_within(&root, &link).await;
		assert!(res.is_err(), "符号链接逃逸应被拒绝，实际: {res:?}");
		let res = DiffConfigTool::diff_config_impl(&root, "config/evil.toml").await;
		assert!(res.is_err(), "diff 遇逃逸备份应报错，实际: {res:?}");
		tokio::fs::remove_file(&outside).await.unwrap();
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn resolve_write_path_rejects_symlink_target() {
		use std::os::unix::fs::symlink;

		let root = temp_instance_root().await;
		let outside = std::env::temp_dir().join(format!(
			"config_ops_outside_{}",
			Utc::now().timestamp_nanos_opt().unwrap_or_default()
		));
		tokio::fs::write(&outside, "secret\n").await.unwrap();
		// 最终写入组件是一个指向外部的符号链接，应被拒绝。
		let link = root.join("config").join("jei.toml");
		symlink(&outside, &link).unwrap();

		let res = resolve_write_path(&root, "config/jei.toml").await;
		assert!(res.is_err(), "最终组件为符号链接时应拒绝，实际: {res:?}");
		tokio::fs::remove_file(&outside).await.unwrap();
	}

	#[tokio::test]
	async fn backup_generate_and_rollback() {
		let root = temp_instance_root().await;
		let rel = "config/jei.toml";
		let target = root.join(rel);
		tokio::fs::write(&target, "original\n").await.unwrap();

		let backup = make_backup(&root, rel).await.unwrap().unwrap();
		assert!(backup.exists());
		let content = read_text(&backup).await.unwrap();
		assert_eq!(content, "original\n");

		// 修改当前文件后回滚到备份内容
		tokio::fs::write(&target, "modified\n").await.unwrap();
		let backups = list_backups(&root, rel).await.unwrap();
		let source = backups.first().unwrap().clone();
		let restored = read_text(&source).await.unwrap();
		assert_eq!(restored, "original\n");

		// 模拟 rollback 写回
		tokio::fs::write(&target, restored.as_bytes()).await.unwrap();
		assert_eq!(read_text(&target).await.unwrap(), "original\n");
	}

	#[tokio::test]
	async fn write_then_read_roundtrip() {
		let root = temp_instance_root().await;
		let rel = "config/test.toml";
		let target = resolve_write_path(&root, rel).await.unwrap();
		tokio::fs::write(&target, "key = \"value\"\n").await.unwrap();
		let content = read_text(&target).await.unwrap();
		assert_eq!(content, "key = \"value\"\n");
		let size = content.len();
		assert!(size > 0);
	}

	#[tokio::test]
	async fn write_create_dir_escape_rejected() {
		let root = temp_instance_root().await;
		// 逃逸路径在 resolve_write_path 前被 safe_join 拒绝
		assert!(safe_join(&root, "config/../../escape").is_err());
	}

	#[test]
	fn diff_lines_reports_changes() {
		let a = vec!["a".to_string(), "b".to_string(), "c".to_string()];
		let b = vec!["a".to_string(), "x".to_string(), "c".to_string()];
		let (out, changed) = diff_lines(&a, &b);
		assert!(changed);
		let joined = out.join("\n");
		assert!(joined.contains("- b"), "应输出删除行: {joined}");
		assert!(joined.contains("+ x"), "应输出新增行: {joined}");
	}

	#[test]
	fn diff_lines_unchanged() {
		let a = vec!["x".to_string(), "y".to_string()];
		let b = vec!["x".to_string(), "y".to_string()];
		let (_out, changed) = diff_lines(&a, &b);
		assert!(!changed);
	}
}
// === AI-WORKSHOP END ===