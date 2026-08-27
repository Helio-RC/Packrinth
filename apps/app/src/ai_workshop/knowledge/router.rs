// === AI-WORKSHOP START ===
// 知识检索路由：mtime 增量检查 + 分发给 bm25 索引。KnowledgeRouter 主体。
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde_json::Value;

use crate::api::Result;
use super::bm25::Bm25Index;
use super::source::KnowledgeSource;

pub struct KnowledgeRouter {
	index_dir: PathBuf,
	/// 懒初始化的 BM25 索引（new 保持无 fallible 签名，索引构建延迟到首次 refresh）。
	index: Mutex<Option<Bm25Index>>,
	sources: Mutex<Vec<Arc<dyn KnowledgeSource>>>,
	/// 每个源最近一次索引时各文档的 mtime（path -> mtime）。
	last_checked: Mutex<HashMap<String, SystemTime>>,
}

impl KnowledgeRouter {
	/// 建空 sources；索引目录的创建/打开延迟到首次 refresh（目录不存在时 create_in_dir）。
	pub fn new(index_dir: PathBuf) -> Self {
		Self {
			index_dir,
			index: Mutex::new(None),
			sources: Mutex::new(Vec::new()),
			last_checked: Mutex::new(HashMap::new()),
		}
	}

	pub fn register_source(&self, source: Arc<dyn KnowledgeSource>) {
		self.sources.lock().unwrap().push(source);
	}

	fn ensure_index(&self) -> Result<()> {
		let mut guard = self.index.lock().unwrap();
		if guard.is_none() {
			*guard = Some(Bm25Index::new(&self.index_dir)?);
		}
		Ok(())
	}

	/// 刷新：对每个源检查 mtime 变化（第一道过滤），仅变化时全量重索引。
	/// 初始 last_checked 为空 → 全量索引。
	pub async fn refresh(&self) -> Result<()> {
		self.ensure_index()?;
		let sources = self.sources.lock().unwrap().clone();
		for source in &sources {
			let docs = source.documents().await?;
			let source_id = source.id().to_string();
			let mut checked = self.last_checked.lock().unwrap();
			let changed = docs.iter().any(|d| match (&d.mtime, checked.get(&d.path)) {
				(Some(m), Some(last)) => m != last,
				_ => true,
			});
			if changed {
				let index = self.index.lock().unwrap();
				let index = index.as_ref().expect("index ensured in refresh");
				index.add_documents(&source_id, &docs)?;
				for d in &docs {
					if let Some(m) = d.mtime {
						checked.insert(d.path.clone(), m);
					}
				}
			}
		}
		Ok(())
	}

	/// 检索：先懒刷新（每次检索前检查是否需要重建），再交给 bm25。
	pub async fn search(
		&self,
		query: &str,
		top_k: usize,
		source: Option<&str>,
	) -> Result<Vec<Value>> {
		self.refresh().await?;
		let index = self.index.lock().unwrap();
		let index = index.as_ref().expect("index ensured in refresh");
		let hits = index.search(query, top_k, source)?;
		Ok(hits
			.into_iter()
			.map(|h| {
				serde_json::json!({
					"title": h.title,
					"snippet": h.snippet,
					"score": h.score,
					"source": h.source_id,
				})
			})
			.collect())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use async_trait::async_trait;
	use super::super::source::SourceDocument;

	struct StaticSource {
		id: &'static str,
		docs: Vec<SourceDocument>,
	}

	#[async_trait]
	impl KnowledgeSource for StaticSource {
		fn id(&self) -> &str {
			self.id
		}
		fn display_name(&self) -> &str {
			"test"
		}
		async fn documents(&self) -> Result<Vec<SourceDocument>> {
			Ok(self.docs.clone())
		}
	}

	fn temp_index_dir() -> PathBuf {
		static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
		let nanos = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_nanos();
		let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
		std::env::temp_dir().join(format!("router_test_{nanos}_{seq}"))
	}

	#[tokio::test]
	async fn lazy_refresh_then_search_and_serialize() {
		let router = KnowledgeRouter::new(temp_index_dir());
		router.register_source(Arc::new(StaticSource {
			id: "skills",
			docs: vec![SourceDocument {
				title: "Guide".into(),
				content: "how to install the launcher".into(),
				path: "guide.md".into(),
				mtime: None,
			}],
		}));
		// search 触发懒刷新并返回前端结构。
		let hits = router.search("install", 5, None).await.unwrap();
		assert_eq!(hits.len(), 1);
		let obj = hits[0].as_object().unwrap();
		assert_eq!(obj["title"], "Guide");
		assert_eq!(obj["source"], "skills");
		assert!(obj["score"].as_f64().unwrap() > 0.0);
		assert!(obj["snippet"].as_str().unwrap().contains("install"));
	}

	#[tokio::test]
	async fn mtime_unchanged_does_not_reindex() {
		let router = KnowledgeRouter::new(temp_index_dir());
		let source = Arc::new(StaticSource {
			id: "skills",
			docs: vec![SourceDocument {
				title: "Guide".into(),
				content: "unique token alpha".into(),
				path: "guide.md".into(),
				mtime: Some(std::time::SystemTime::now()),
			}],
		});
		router.register_source(source.clone());
		// 第一次刷新全量索引。
		router.refresh().await.unwrap();
		// 第二次刷新：mtime 未变 → 不重索引，仍可检索且结果不变。
		router.refresh().await.unwrap();
		let hits = router.search("alpha", 5, None).await.unwrap();
		assert_eq!(hits.len(), 1);
	}
}
// === AI-WORKSHOP END ===