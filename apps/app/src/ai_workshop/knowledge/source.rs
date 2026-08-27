// === AI-WORKSHOP START ===
// 知识源抽象：KnowledgeSource trait 与内置 SkillsSource（索引技能目录中的 .md 文件）。
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use async_trait::async_trait;

use crate::api::Result;

/// 单个待索引文档。
#[derive(Clone)]
pub struct SourceDocument {
	pub title: String,
	pub content: String,
	/// 源内相对路径，用作 mtime 增量检查的键。
	pub path: String,
	pub mtime: Option<SystemTime>,
}

/// 知识源：向检索路由提供文档流。实现须为 Send + Sync，以便被 Arc 共享。
#[async_trait]
pub trait KnowledgeSource: Send + Sync {
	fn id(&self) -> &str;
	fn display_name(&self) -> &str;
	async fn documents(&self) -> Result<Vec<SourceDocument>>;
}

/// 内置源：扫描技能目录（递归）下所有 `.md` 文件，title=文件名、content=原文。
pub struct SkillsSource {
	skills_dir: PathBuf,
}

impl SkillsSource {
	pub fn new(skills_dir: PathBuf) -> Self {
		Self { skills_dir }
	}
}

#[async_trait]
impl KnowledgeSource for SkillsSource {
	fn id(&self) -> &str {
		"skills"
	}

	fn display_name(&self) -> &str {
		"本地技能"
	}

	async fn documents(&self) -> Result<Vec<SourceDocument>> {
		let mut docs = Vec::new();
		collect_markdown(&self.skills_dir, &mut docs)?;
		Ok(docs)
	}
}

fn collect_markdown(dir: &Path, docs: &mut Vec<SourceDocument>) -> Result<()> {
	let entries = std::fs::read_dir(dir)?;
	for entry in entries.flatten() {
		let path = entry.path();
		if path.is_dir() {
			collect_markdown(&path, docs)?;
		} else if path.extension().and_then(|e| e.to_str()) == Some("md") {
			let content = std::fs::read_to_string(&path)?;
			let title = path
				.file_stem()
				.map(|stem| stem.to_string_lossy().to_string())
				.unwrap_or_default();
			let rel = path
				.strip_prefix(dir)
				.map(|p| p.to_string_lossy().to_string())
				.unwrap_or_else(|_| path.to_string_lossy().to_string());
			let mtime = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
			docs.push(SourceDocument {
				title,
				content,
				path: rel,
				mtime,
			});
		}
	}
	Ok(())
}
// === AI-WORKSHOP END ===