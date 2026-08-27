// === AI-WORKSHOP START ===
// 知识检索模块：BM25 检索（tantivy）+ 内容爬取与分块。
// 流 D.3/D.4 实现：mtime 增量索引、域名白名单爬虫、智能分块。
// 模块入口保持既有 pub 接口（KnowledgeRouter::new/search/refresh），逻辑分发到子模块。
pub mod bm25;
pub mod router;
pub mod source;

pub use router::KnowledgeRouter;
// === AI-WORKSHOP END ===
