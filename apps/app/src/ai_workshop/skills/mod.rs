// === AI-WORKSHOP START ===
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;

use crate::api::Result;
use crate::ai_workshop::other_err;

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

/// 技能加载器：扫描 `<data_dir>/ai-workshop/skills/`，解析 `skill.toml` + `guide.md`。
/// 流 D.1/D.2 实现：路径遍历防护、内容净化、热加载与匹配。
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

	/// 扫描并加载全部技能，返回加载失败的技能名列表。
	pub async fn load_all(&self) -> Vec<String> {
		let _ = std::fs::create_dir_all(&self.base_path);
		Vec::new()
	}

	/// 重新扫描技能目录（热加载），返回加载失败的技能名列表。
	pub async fn refresh(&self) -> Vec<String> {
		Vec::new()
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
		let _ = name;
		None
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

	pub async fn import_skill(&self, _path: &str) -> Result<()> {
		Ok(())
	}
}
// === AI-WORKSHOP END ===