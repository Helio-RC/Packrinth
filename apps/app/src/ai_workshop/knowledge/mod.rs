// === AI-WORKSHOP START ===
use std::path::PathBuf;

use serde_json::Value;

use crate::api::Result;

/// 知识检索路由：BM25 检索（tantivy）+ 内容爬取与分块。
/// 流 D.3/D.4 实现：mtime 增量索引、域名白名单爬虫、智能分块。
pub struct KnowledgeRouter {
	index_dir: PathBuf,
}

impl KnowledgeRouter {
	pub fn new(index_dir: PathBuf) -> Self {
		Self { index_dir }
	}

	/// BM25 检索，返回命中文档列表（含来源与摘要）。
	pub async fn search(
		&self,
		_query: &str,
		_top_k: usize,
		_source: Option<&str>,
	) -> Result<Vec<Value>> {
		let _ = &self.index_dir;
		Ok(Vec::new())
	}

	/// 重新构建/刷新索引（耗时任务，后台执行）。
	pub async fn refresh(&self) -> Result<()> {
		Ok(())
	}
}
// === AI-WORKSHOP END ===