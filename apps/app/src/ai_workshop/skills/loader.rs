// === AI-WORKSHOP START ===
// SkillLoader 实现（流 D.1/D.2）：扫描技能目录、解析 skill.toml + 净化 guide.md、
// 路径遍历防护（safe_path）、热加载与匹配数据来源。
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::ai_workshop::other_err;
use crate::api::Result;
use super::sanitizer::sanitize_guide_md;

/// 技能元信息（对应 `skill.toml`）。
#[derive(Clone, Debug, Serialize)]
pub struct Skill {
	pub name: String,
	pub description: String,
	pub keywords: Vec<String>,
	pub priority: u8,
	pub version: String,
	pub author: String,
	pub enabled: bool,
	pub guide_md: String,
}

/// skill.toml 的原始字段（未含 enabled/guide_md，它们在加载时合成）。
#[derive(Deserialize)]
struct SkillToml {
	name: String,
	description: String,
	#[serde(default)]
	keywords: Vec<String>,
	#[serde(default)]
	priority: u8,
	#[serde(default = "default_version")]
	version: String,
	#[serde(default = "default_author")]
	author: String,
}

fn default_version() -> String {
	"1.0".to_string()
}

fn default_author() -> String {
	"user".to_string()
}

/// 技能加载器：扫描 `<data_dir>/ai-workshop/skills/`，解析 `skill.toml` + `guide.md`。
pub struct SkillLoader {
	base_path: PathBuf,
	skills: Mutex<HashMap<String, Skill>>,
}

impl SkillLoader {
	pub fn new(base_path: PathBuf) -> Self {
		Self {
			base_path,
			skills: Mutex::new(HashMap::new()),
		}
	}

	/// 扫描并加载全部技能，返回加载失败的技能（目录名: 原因）列表。
	pub async fn load_all(&self) -> Vec<String> {
		let _ = std::fs::create_dir_all(&self.base_path);
		self.refresh().await
	}

	/// 重新扫描技能目录（热加载），返回加载失败的技能（目录名: 原因）列表。
	pub async fn refresh(&self) -> Vec<String> {
		let mut failed = Vec::new();
		let mut new_skills: HashMap<String, Skill> = HashMap::new();

		let entries = match std::fs::read_dir(&self.base_path) {
			Ok(entries) => entries,
			Err(e) => {
				failed.push(format!("skills: 无法读取技能目录: {e}"));
				return failed;
			}
		};
		for entry in entries.flatten() {
			let path = entry.path();
			if !path.is_dir() || !path.join("skill.toml").is_file() {
				continue;
			}
			match load_skill_from_dir(&path) {
				Ok(skill) => {
					new_skills.insert(skill.name.clone(), skill);
				}
				Err(reason) => {
					let dir_name = path
						.file_name()
						.map(|name| name.to_string_lossy().to_string())
						.unwrap_or_default();
					failed.push(format!("{dir_name}: {reason}"));
				}
			}
		}

		// 保留已存在技能的 enabled 状态，避免热加载时用户设置被重置。
		{
			let old = self.skills.lock().unwrap();
			for skill in new_skills.values_mut() {
				if let Some(old_skill) = old.get(&skill.name) {
					skill.enabled = old_skill.enabled;
				}
			}
		}
		*self.skills.lock().unwrap() = new_skills;
		failed
	}

	pub fn skills(&self) -> Vec<Skill> {
		self.skills.lock().unwrap().values().cloned().collect()
	}

	/// 已启用的技能（供推理引擎注入上下文）。
	pub fn enabled_skills(&self) -> Vec<Skill> {
		self.skills
			.lock()
			.unwrap()
			.values()
			.filter(|skill| skill.enabled)
			.cloned()
			.collect()
	}

	pub fn get_skill(&self, name: &str) -> Option<Skill> {
		self.skills.lock().unwrap().get(name).cloned()
	}

	pub fn guide_md(&self, name: &str) -> Option<String> {
		self.skills
			.lock()
			.unwrap()
			.get(name)
			.map(|skill| skill.guide_md.clone())
	}

	pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
		if let Some(skill) = self.skills.lock().unwrap().get_mut(name) {
			skill.enabled = enabled;
			Ok(())
		} else {
			Err(other_err(format!("Unknown skill: {name}")))
		}
	}

	pub fn force_load(&self, name: &str) -> Result<Skill> {
		self.get_skill(name).ok_or_else(|| other_err(format!("Unknown skill: {name}")))
	}

	/// 将用户提供的技能目录复制到 base_path（safe_path 校验），随后重新加载。
	pub async fn import_skill(&self, path: &str) -> Result<()> {
		let src = PathBuf::from(path);
		// 路径遍历防护：canonicalize 后必须仍位于 base_path 内。
		let canonical_src = safe_path(&self.base_path, &src).map_err(other_err)?;
		let dir_name = canonical_src
			.file_name()
			.ok_or_else(|| other_err("无效技能目录"))?
			.to_string_lossy()
			.to_string();
		let dest = self.base_path.join(&dir_name);
		copy_dir_all(&canonical_src, &dest)
			.map_err(|e| other_err(format!("导入技能失败: {e}")))?;
		let _failed = self.refresh().await;
		Ok(())
	}
}

/// 从单个技能目录加载技能（解析 skill.toml + 净化 guide.md）。任一校验失败返回 Err。
fn load_skill_from_dir(dir: &Path) -> std::result::Result<Skill, String> {
	let raw = std::fs::read_to_string(dir.join("skill.toml"))
		.map_err(|e| format!("读取 skill.toml 失败: {e}"))?;
	let parsed: SkillToml = toml::from_str(&raw).map_err(|e| format!("解析 skill.toml 失败: {e}"))?;
	validate_toml(&parsed)?;

	let guide_path = dir.join("guide.md");
	let guide_md = if guide_path.is_file() {
		let raw_guide =
			std::fs::read_to_string(&guide_path).map_err(|e| format!("读取 guide.md 失败: {e}"))?;
		sanitize_guide_md(&raw_guide)?
	} else {
		String::new()
	};

	Ok(Skill {
		name: parsed.name,
		description: parsed.description,
		keywords: parsed.keywords,
		priority: parsed.priority,
		version: parsed.version,
		author: parsed.author,
		enabled: true,
		guide_md,
	})
}

/// skill.toml 字段校验（计划 §7.4）：任一不合法则跳过整个技能。
fn validate_toml(parsed: &SkillToml) -> std::result::Result<(), String> {
	if parsed.name.is_empty()
		|| !parsed
			.name
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-')
	{
		return Err(format!("name 仅允许字母、数字、空格和连字符: {:?}", parsed.name));
	}
	if parsed.priority > 100 {
		return Err(format!("priority 必须在 0~100 之间: {}", parsed.priority));
	}
	if parsed.keywords.is_empty() || parsed.keywords.len() > 20 {
		return Err(format!("keywords 必须为 1~20 个: {}", parsed.keywords.len()));
	}
	Ok(())
}

/// 路径遍历防护：canonicalize 用户路径后与 base_path 前缀比较，`../` 逃逸拒绝。
fn safe_path(base_path: &Path, candidate: &Path) -> std::result::Result<PathBuf, String> {
	let canonical_base = std::fs::canonicalize(base_path)
		.map_err(|e| format!("无法解析技能根目录 {}: {e}", base_path.display()))?;
	let canonical_candidate = std::fs::canonicalize(candidate)
		.map_err(|e| format!("无法解析路径 {}: {e}", candidate.display()))?;
	if !canonical_candidate.starts_with(&canonical_base) {
		return Err(format!("不安全路径，已逃逸技能目录: {}", candidate.display()));
	}
	Ok(canonical_candidate)
}

/// 递归复制目录内容。
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
	std::fs::create_dir_all(dst)?;
	for entry in std::fs::read_dir(src)? {
		let entry = entry?;
		let file_type = entry.file_type()?;
		let src_path = entry.path();
		let dst_path = dst.join(entry.file_name());
		if file_type.is_dir() {
			copy_dir_all(&src_path, &dst_path)?;
		} else {
			std::fs::copy(&src_path, &dst_path)?;
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn temp_base() -> PathBuf {
		let nanos = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_nanos();
		let base = std::env::temp_dir().join(format!("skills_test_{nanos}"));
		std::fs::create_dir_all(&base).unwrap();
		base
	}

	fn write_skill(base: &Path, dir: &str, toml: &str, guide: Option<&str>) {
		let skill_dir = base.join(dir);
		std::fs::create_dir_all(&skill_dir).unwrap();
		std::fs::write(skill_dir.join("skill.toml"), toml).unwrap();
		if let Some(g) = guide {
			std::fs::write(skill_dir.join("guide.md"), g).unwrap();
		}
	}

	#[tokio::test]
	async fn loads_valid_skill() {
		let base = temp_base();
		write_skill(
			&base,
			"minecraft-helper",
			r#"
name = "Minecraft Helper"
description = "A helper"
keywords = ["mc", "minecraft"]
priority = 80
"#,
			Some("# Guide\n\nSome text."),
		);
		let loader = SkillLoader::new(base);
		let failed = loader.load_all().await;
		assert!(failed.is_empty(), "failed: {failed:?}");
		let skills = loader.skills();
		assert_eq!(skills.len(), 1);
		assert_eq!(skills[0].name, "Minecraft Helper");
		assert_eq!(skills[0].priority, 80);
		assert_eq!(skills[0].version, "1.0");
		assert_eq!(skills[0].author, "user");
		assert!(skills[0].enabled);
		assert!(!skills[0].guide_md.is_empty());
	}

	#[tokio::test]
	async fn rejects_priority_out_of_range() {
		let base = temp_base();
		write_skill(
			&base,
			"bad",
			r#"
name = "bad"
description = "x"
keywords = ["a"]
priority = 200
"#,
			None,
		);
		let loader = SkillLoader::new(base);
		let failed = loader.load_all().await;
		assert_eq!(failed.len(), 1);
		assert!(failed[0].starts_with("bad"));
		assert!(loader.skills().is_empty());
	}

	#[tokio::test]
	async fn rejects_name_with_special_chars() {
		let base = temp_base();
		write_skill(
			&base,
			"bad",
			r#"
name = "bad/name"
description = "x"
keywords = ["a"]
"#,
			None,
		);
		let loader = SkillLoader::new(base);
		let failed = loader.load_all().await;
		assert_eq!(failed.len(), 1);
		assert!(loader.skills().is_empty());
	}

	#[tokio::test]
	async fn rejects_empty_keywords() {
		let base = temp_base();
		write_skill(
			&base,
			"bad",
			r#"
name = "bad"
description = "x"
keywords = []
"#,
			None,
		);
		let loader = SkillLoader::new(base);
		let failed = loader.load_all().await;
		assert_eq!(failed.len(), 1);
		assert!(loader.skills().is_empty());
	}

	#[tokio::test]
	async fn rejects_unsafe_guide() {
		let base = temp_base();
		write_skill(
			&base,
			"bad",
			r#"
name = "bad"
description = "x"
keywords = ["a"]
"#,
			Some("<script>alert(1)</script>"),
		);
		let loader = SkillLoader::new(base);
		let failed = loader.load_all().await;
		assert_eq!(failed.len(), 1);
		assert!(failed[0].starts_with("bad"));
		assert!(loader.skills().is_empty());
	}

	#[tokio::test]
	async fn preserves_enabled_state_on_refresh() {
		let base = temp_base();
		write_skill(
			&base,
			"a",
			r#"
name = "a"
description = "x"
keywords = ["a"]
"#,
			None,
		);
		let loader = SkillLoader::new(base);
		assert!(loader.load_all().await.is_empty());
		loader.set_enabled("a", false).unwrap();
		let failed = loader.refresh().await;
		assert!(failed.is_empty());
		assert!(!loader.get_skill("a").unwrap().enabled);
	}

	#[tokio::test]
	async fn import_rejects_path_traversal() {
		let base = temp_base();
		write_skill(
			&base,
			"src-skill",
			r#"
name = "src"
description = "x"
keywords = ["a"]
"#,
			None,
		);
		// base 之外的目录
		let outside = base.parent().unwrap().join("outside");
		std::fs::create_dir_all(&outside).unwrap();
		let loader = SkillLoader::new(base.clone());
		// 通过 ../ 逃逸到 base 之外。
		let escaped = base.join("src-skill").join("../../outside");
		let res = loader.import_skill(escaped.to_str().unwrap()).await;
		assert!(res.is_err(), "逃逸路径应被拒绝，实际: {res:?}");
	}
}
// === AI-WORKSHOP END ===