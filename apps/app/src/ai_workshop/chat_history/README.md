# chat_history

SQLite 对话持久化（`rusqlite`，bundled）。库文件：`<data_dir>/ai-workshop/chat_history/chat.db`。

## 组成

- `db.rs`：建库建表（conversations / messages + 两个索引，见 goal.md §3.4）。
- `repository.rs`：会话与消息 CRUD；`list_conversations` / `get_conversation` 支持 `offset` / `limit` 分页（默认 50 条，按 `created_at` 升序）。
- `models.rs`：行数据映射（NewMessage / Message / Conversation）。

## 测试

repository 覆盖建表、CRUD、分页、删除级联与事务回滚。
