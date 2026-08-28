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

/// 加载失败的技能条目（目录名 + 原因），供 SkillsView "加载失败的技能" 列表展示。
#[derive(Clone, Debug, serde::Serialize)]
pub struct FailedSkill {
	pub dir_name: String,
	pub reason: String,
}

/// 技能加载器：扫描 `<data_dir>/ai-workshop/skills/`，解析 `skill.toml` + `guide.md`。
pub struct SkillLoader {
	base_path: PathBuf,
	skills: Mutex<HashMap<String, Skill>>,
	failed: Mutex<Vec<FailedSkill>>,
}

impl SkillLoader {
	pub fn new(base_path: PathBuf) -> Self {
		Self {
			base_path,
			skills: Mutex::new(HashMap::new()),
			failed: Mutex::new(Vec::new()),
		}
	}

	/// 最近一次扫描中的失败清单（加载失败即整体跳过，见 §7.4）。
	pub fn failed_skills(&self) -> Vec<FailedSkill> {
		self.failed.lock().unwrap().clone()
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
		// 持久化失败清单供前端查询（跳过：不部分加载）。
		let failed_skills = failed
			.iter()
			.filter_map(|entry| {
				let (dir_name, reason) = entry.split_once(": ")?;
				Some(FailedSkill {
					dir_name: dir_name.to_string(),
					reason: reason.to_string(),
				})
			})
			.collect();
		*self.failed.lock().unwrap() = failed_skills;
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

	/// 将用户提供的技能目录复制到 base_path，随后重新加载。
	/// 来源可为任意已存在的目录（canonicalize + 存在性/目录校验，不做前缀包含限制）；
	/// 目标始终为 `base_path.join(dir_name)`，其中 dir_name 取自规范化后来源的叶名，
	/// 因此不会逃逸 base_path。
	pub async fn import_skill(&self, path: &str) -> Result<()> {
		let src = PathBuf::from(path);
		let canonical_src = std::fs::canonicalize(&src)
			.map_err(|e| other_err(format!("无法解析来源路径 {}: {e}", src.display())))?;
		if !canonical_src.is_dir() {
			return Err(other_err(format!(
				"技能来源不是目录: {}",
				canonical_src.display()
			)));
		}
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
		enabled: false,
		guide_md,
	})
}

/// skill.toml 字段校验（计划 §7.4）：任一不合法则跳过整个技能。
fn validate_toml(parsed: &SkillToml) -> std::result::Result<(), String> {
	if parsed.name.is_empty()
		|| !parsed
			.name
			.chars()
			.all(|c| c.is_alphanumeric() || c == ' ' || c == '-')
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
		assert!(!skills[0].enabled, "newly loaded skills must default to disabled");
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
	async fn accepts_unicode_name() {
		let base = temp_base();
		write_skill(
			&base,
			"中文技能",
			r#"
name = "中文技能 助手"
description = "x"
keywords = ["中文"]
"#,
			None,
		);
		let loader = SkillLoader::new(base);
		let failed = loader.load_all().await;
		assert!(failed.is_empty(), "中文名技能应加载成功，失败: {failed:?}");
		let skills = loader.skills();
		assert_eq!(skills.len(), 1);
		assert_eq!(skills[0].name, "中文技能 助手");
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
	async fn new_skill_added_on_refresh_defaults_to_disabled() {
		let base = temp_base();
		write_skill(
			&base,
			"existing",
			r#"
name = "existing"
description = "x"
keywords = ["a"]
"#,
			None,
		);
		let loader = SkillLoader::new(base);
		assert!(loader.load_all().await.is_empty());
		// 用户手动启用已有技能。
		loader.set_enabled("existing", true).unwrap();

		// 热加载时新增一个技能：应默认禁用，且不重置已有技能的手动状态。
		write_skill(
			&loader.base_path,
			"new-skill",
			r#"
name = "new-skill"
description = "y"
keywords = ["b"]
"#,
			None,
		);
		let failed = loader.refresh().await;
		assert!(failed.is_empty());
		assert!(
			!loader.get_skill("new-skill").unwrap().enabled,
			"new skill on refresh must default to disabled"
		);
		assert!(
			loader.get_skill("existing").unwrap().enabled,
			"existing skill must preserve manual enabled state"
		);
	}

	#[tokio::test]
	async fn import_accepts_external_dir_without_escape() {
		let base = temp_base();
		// base 之外的目录，作为导入来源。
		let outside = base.parent().unwrap().join("outside");
		write_skill(
			&outside,
			"external",
			r#"
name = "external"
description = "x"
keywords = ["a"]
"#,
			None,
		);
		let loader = SkillLoader::new(base.clone());
		// 来源不受 base_path 限制；即使通过 ../ 拼出外部路径，canonicalize 也会解析到
		// 该外部目录，其叶名作为目标，落点在 base_path 内，不会逃逸。
		let external_path = base.join("..").join("outside").join("external");
		let res = loader.import_skill(external_path.to_str().unwrap()).await;
		assert!(res.is_ok(), "导入外部目录应成功，实际: {res:?}");
		// 目标位于 base_path 内（叶名），而非写入 base_path 之外。
		// 技能目录被复制到 base_path 内（叶名），而非写入 base_path 之外。
		assert!(base.join("external").is_dir());
		let skills = loader.skills();
		assert_eq!(skills.len(), 1);
		assert_eq!(skills[0].name, "external");
	}

	#[tokio::test]
	async fn import_rejects_nonexistent_source() {
		let base = temp_base();
		let loader = SkillLoader::new(base.clone());
		let missing = base.parent().unwrap().join("does_not_exist");
		let res = loader.import_skill(missing.to_str().unwrap()).await;
		assert!(res.is_err(), "不存在的来源应被拒绝，实际: {res:?}");
	}

	#[tokio::test]
	async fn import_rejects_non_directory_source() {
		let base = temp_base();
		// 目标是普通文件而非目录。
		let file = base.parent().unwrap().join("plain_file");
		std::fs::write(&file, "x").unwrap();
		let loader = SkillLoader::new(base);
		let res = loader.import_skill(file.to_str().unwrap()).await;
		assert!(res.is_err(), "文件来源应被拒绝，实际: {res:?}");
	}
}
// === AI-WORKSHOP END ===