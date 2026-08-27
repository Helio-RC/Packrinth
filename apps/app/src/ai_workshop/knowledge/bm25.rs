// === AI-WORKSHOP START ===
// BM25 检索实现（tantivy 0.26）：schema、索引写入、检索与命中结果。
use std::path::Path;

use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, OwnedValue, Schema, TantivyDocument, STORED, TEXT};
use tantivy::{doc, Index, ReloadPolicy};

use crate::ai_workshop::other_err;
use crate::api::Result;
use super::source::SourceDocument;

/// 检索命中项。
pub struct SearchHit {
	pub title: String,
	pub snippet: String,
	pub score: f64,
	pub source_id: String,
}

fn err(e: impl std::fmt::Display) -> crate::api::TheseusSerializableError {
	other_err(e.to_string())
}

fn build_schema() -> Schema {
	let mut builder = Schema::builder();
	builder.add_text_field("title", TEXT | STORED);
	builder.add_text_field("content", TEXT | STORED);
	builder.add_text_field("source_id", TEXT | STORED);
	builder.add_text_field("path", STORED);
	builder.build()
}

/// tantivy 索引的线程安全封装（Index 为 Send + Sync，可被 Arc 共享）。
pub struct Bm25Index {
	index: Index,
}

impl Bm25Index {
	pub fn new(index_dir: &Path) -> Result<Self> {
		let schema = build_schema();
		let index = match Index::open_in_dir(index_dir) {
			Ok(index) => index,
			Err(_) => {
				std::fs::create_dir_all(index_dir).map_err(err)?;
				Index::create_in_dir(index_dir, schema).map_err(err)?
			}
		};
		Ok(Self { index })
	}

	/// 全量替换式写入：重建 writer、add_document 后 commit（writer 每批重建，符合 brief）。
	pub fn add_documents(&self, source_id: &str, docs: &[SourceDocument]) -> Result<()> {
		let schema = self.index.schema();
		let title = schema.get_field("title").expect("title field");
		let content = schema.get_field("content").expect("content field");
		let source = schema.get_field("source_id").expect("source_id field");
		let path = schema.get_field("path").expect("path field");

		let mut writer = self.index.writer(50_000_000).map_err(err)?;
		for d in docs {
			writer
				.add_document(doc!(
					title => d.title.as_str(),
					content => d.content.as_str(),
					source => source_id,
					path => d.path.as_str(),
				))
				.map_err(err)?;
		}
		writer.commit().map_err(err)?;
		Ok(())
	}

	pub fn search(&self, query: &str, top_k: usize, source_filter: Option<&str>) -> Result<Vec<SearchHit>> {
		if top_k == 0 {
			return Ok(Vec::new());
		}
		let schema = self.index.schema();
		let title = schema.get_field("title").expect("title field");
		let content = schema.get_field("content").expect("content field");
		let source = schema.get_field("source_id").expect("source_id field");

		let reader = self
			.index
			.reader_builder()
			.reload_policy(ReloadPolicy::OnCommitWithDelay)
			.try_into()
			.map_err(err)?;
		let searcher = reader.searcher();

		let query_parser = QueryParser::for_index(&self.index, vec![title, content]);
		let parsed: Box<dyn Query> = match query_parser.parse_query(query) {
			Ok(q) => q,
			// 非法语法（如 "("）降级为 TermQuery 全词匹配，不返回错误。
			Err(_) => fallback_term_query(query, title, content),
		};

		let final_query: Box<dyn Query> = if let Some(source_id) = source_filter {
			let mut clauses: Vec<(Occur, Box<dyn Query>)> = vec![(
				Occur::Must,
				Box::new(TermQuery::new(
					tantivy::Term::from_field_text(source, source_id),
					IndexRecordOption::Basic,
				)),
			)];
			clauses.push((Occur::Must, parsed));
			Box::new(BooleanQuery::new(clauses))
		} else {
			parsed
		};

		let top_docs = searcher
			.search(&final_query, &TopDocs::with_limit(top_k).order_by_score())
			.map_err(err)?;

		let mut hits = Vec::with_capacity(top_docs.len());
		for (score, doc_address) in top_docs {
			let doc = searcher.doc::<TantivyDocument>(doc_address).map_err(err)?;
			let title_str = get_str(&doc, title).unwrap_or_default();
			let content_str = get_str(&doc, content).unwrap_or_default();
			let source_id = get_str(&doc, source).unwrap_or_default();
			let snippet: String = content_str.chars().take(200).collect();
			hits.push(SearchHit {
				title: title_str,
				snippet,
				score: score as f64,
				source_id,
			});
		}
		Ok(hits)
	}
}

/// 非法查询降级：将每个空白分隔的词拆成 title/content 上的 TermQuery，OR 组合。
fn fallback_term_query(query: &str, title: Field, content: Field) -> Box<dyn Query> {
	let words: Vec<&str> = query.split_whitespace().collect();
	if words.is_empty() {
		return Box::new(BooleanQuery::new(Vec::new()));
	}
	let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::with_capacity(words.len() * 2);
	for word in words {
		clauses.push((
			Occur::Should,
			Box::new(TermQuery::new(
				tantivy::Term::from_field_text(title, word),
				IndexRecordOption::Basic,
			)),
		));
		clauses.push((
			Occur::Should,
			Box::new(TermQuery::new(
				tantivy::Term::from_field_text(content, word),
				IndexRecordOption::Basic,
			)),
		));
	}
	Box::new(BooleanQuery::new(clauses))
}

fn get_str(doc: &TantivyDocument, field: Field) -> Option<String> {
	let owned: OwnedValue = doc.get_first(field)?.into();
	match owned {
		OwnedValue::Str(s) => Some(s),
		OwnedValue::PreTokStr(t) => Some(t.text),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	fn temp_index_dir() -> PathBuf {
		let nanos = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_nanos();
		std::env::temp_dir().join(format!("bm25_test_{nanos}"))
	}

	fn doc(title: &str, content: &str) -> SourceDocument {
		SourceDocument {
			title: title.to_string(),
			content: content.to_string(),
			path: format!("{title}.md"),
			mtime: None,
		}
	}

	#[test]
	fn search_returns_relevant_first() {
		let dir = temp_index_dir();
		let index = Bm25Index::new(&dir).unwrap();
		index
			.add_documents(
				"skills",
				&[
					doc("Install Guide", "How to install the launcher and configure the game."),
					doc("Troubleshooting", "Common errors during installation and their fixes."),
				],
			)
			.unwrap();
		let hits = index.search("install errors", 3, None).unwrap();
		assert!(!hits.is_empty());
		assert!(hits.iter().all(|h| h.source_id == "skills"));
		assert_eq!(hits[0].title, "Install Guide");
		assert!(!hits[0].snippet.is_empty());
		assert!(hits[0].snippet.len() <= 200);
	}

	#[test]
	fn search_filters_by_source() {
		let dir = temp_index_dir();
		let index = Bm25Index::new(&dir).unwrap();
		index
			.add_documents("skills", &[doc("Guide", "install the launcher here")])
			.unwrap();
		index
			.add_documents("docs", &[doc("Manual", "install the game here")])
			.unwrap();
		let hits = index.search("install", 10, Some("docs")).unwrap();
		assert!(!hits.is_empty());
		assert!(hits.iter().all(|h| h.source_id == "docs"));
		let no_hits = index.search("install", 10, Some("nonexistent")).unwrap();
		assert!(no_hits.is_empty());
	}

	#[test]
	fn invalid_query_degrades_without_error() {
		let dir = temp_index_dir();
		let index = Bm25Index::new(&dir).unwrap();
		index
			.add_documents("skills", &[doc("Guide", "install the launcher")])
			.unwrap();
		// 非法语法（"("）与空串均不应 panic / 返回错误。
		let hits = index.search("(", 10, None).unwrap();
		let empty = index.search("", 10, None).unwrap();
		assert!(hits.is_empty());
		assert!(empty.is_empty());
	}
}
// === AI-WORKSHOP END ===