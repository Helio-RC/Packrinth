# knowledge

BM25 知识检索（tantivy；RAG 向量暂缓，见 goal.md）。

## 组成

- `source.rs`：`KnowledgeSource` trait；内置 `SkillsSource`（以技能 guide.md 为文档源）。
- `router.rs`：按 mtime 增量检查（第一道过滤）→ 懒建索引 → 检索；`refresh_knowledge` 手动刷新。
- `bm25.rs`：tantivy 索引封装（全量替换式写入 + 查询）。
- `chunker.rs`：文档分块（≤512 tokens 量级）。
- `crawler.rs`：域名白名单 + 大小限制 + scraper 提取正文 → 入库。（2026-08-28：html2md 0.2.17 与 workspace release `panic=abort` 混链冲突，已回退 scraper 提取。）

## 测试

router 覆盖 mtime 未变不重索引、内容变化重索引；chunker/crawler 覆盖分块、白名单拒绝、非法 URL。
