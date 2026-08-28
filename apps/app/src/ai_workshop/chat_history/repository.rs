use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use rusqlite::{Connection, params};
use uuid::Uuid;

use super::db::{other_err, sqlite_err};
use super::models;
use crate::api::Result;

pub struct ChatHistoryRepository {
    inner: Mutex<Connection>,
}

impl ChatHistoryRepository {
    /// 打开数据库并执行每日备份检查。
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = super::db::open(db_path)?;
        super::db::backup_if_due(&conn, db_path)?;
        Ok(Self {
            inner: Mutex::new(conn),
        })
    }

    /// 列出会话，按 `updated_at` 降序分页；`instance_id` 为 None 时不过滤。
    pub async fn list_conversations(
        &self,
        instance_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<models::Conversation>> {
        // TODO: rusqlite 为同步 API，若在低端设备上出现 UI 卡顿，可迁移至 spawn_blocking
        let conn = self.inner.lock().unwrap();
        let mut stmt = conn
			.prepare(
				"SELECT id, title, instance_id, created_at, updated_at FROM conversations
				 WHERE (?1 IS NULL OR instance_id = ?1)
				 ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3",
			)
			.map_err(sqlite_err)?;
        let rows = stmt
            .query_map(params![instance_id, limit, offset], |row| {
                Ok(models::Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    instance_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(sqlite_err)?;
        rows.collect::<rusqlite::Result<Vec<_>, _>>()
            .map_err(sqlite_err)
    }

    /// 获取单个会话及其消息（按 `created_at` 升序分页）；不存在时返回 None。
    pub async fn get_conversation(
        &self,
        id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Option<(models::Conversation, Vec<models::Message>)>> {
        let conn = self.inner.lock().unwrap();
        let conversation = match conn.query_row(
			"SELECT id, title, instance_id, created_at, updated_at FROM conversations WHERE id = ?1",
			params![id],
			|row| {
				Ok(models::Conversation {
					id: row.get(0)?,
					title: row.get(1)?,
					instance_id: row.get(2)?,
					created_at: row.get(3)?,
					updated_at: row.get(4)?,
				})
			},
		) {
			Ok(conversation) => conversation,
			Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
			Err(e) => return Err(sqlite_err(e)),
		};

        let mut stmt = conn
			.prepare(
				"SELECT id, conversation_id, role, content, tool_calls, tool_call_id, created_at
				 FROM messages
				 WHERE conversation_id = ?1
				 ORDER BY created_at ASC LIMIT ?2 OFFSET ?3",
			)
			.map_err(sqlite_err)?;
        let rows = stmt
            .query_map(params![id, limit, offset], |row| {
                Ok(models::Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    tool_calls: row.get(4)?,
                    tool_call_id: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(sqlite_err)?;
        let messages = rows
            .collect::<rusqlite::Result<Vec<_>, _>>()
            .map_err(sqlite_err)?;

        Ok(Some((conversation, messages)))
    }

    /// 新建会话，id 为 UUID v4。
    pub async fn create_conversation(
        &self,
        title: &str,
        instance_id: Option<&str>,
    ) -> Result<models::Conversation> {
        let now = Utc::now().timestamp();
        let conversation = models::Conversation {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            instance_id: instance_id.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        };
        let conn = self.inner.lock().unwrap();
        conn.execute(
			"INSERT INTO conversations (id, title, instance_id, created_at, updated_at)
			 VALUES (?1, ?2, ?3, ?4, ?5)",
			params![
				&conversation.id,
				&conversation.title,
				&conversation.instance_id,
				&conversation.created_at,
				&conversation.updated_at
			],
		)
		.map_err(sqlite_err)?;
        Ok(conversation)
    }

    /// 重命名会话。
    pub async fn rename_conversation(
        &self,
        id: &str,
        title: &str,
    ) -> Result<()> {
        let conn = self.inner.lock().unwrap();
        conn.execute(
			"UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
			params![title, Utc::now().timestamp(), id],
		)
		.map_err(sqlite_err)?;
        Ok(())
    }

    /// 删除会话及其全部消息（外键级联）。
    pub async fn delete_conversation(&self, id: &str) -> Result<()> {
        let conn = self.inner.lock().unwrap();
        conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])
            .map_err(sqlite_err)?;
        Ok(())
    }

    /// 追加消息并刷新所属会话的 `updated_at`。
    pub async fn add_message(
        &self,
        msg: models::NewMessage,
    ) -> Result<models::Message> {
        let now = Utc::now().timestamp();
        let message = models::Message {
            id: Uuid::new_v4().to_string(),
            conversation_id: msg.conversation_id.clone(),
            role: msg.role.clone(),
            content: msg.content.clone(),
            tool_calls: msg.tool_calls.clone(),
            tool_call_id: msg.tool_call_id.clone(),
            created_at: now,
        };
        let conn = self.inner.lock().unwrap();
        conn.execute(
			"INSERT INTO messages (id, conversation_id, role, content, tool_calls, tool_call_id, created_at)
			 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
			params![
				&message.id,
				&message.conversation_id,
				&message.role,
				&message.content,
				&message.tool_calls,
				&message.tool_call_id,
				&message.created_at
			],
		)
		.map_err(sqlite_err)?;
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now, msg.conversation_id],
        )
        .map_err(sqlite_err)?;
        Ok(message)
    }

    /// 导出会话：`json` 输出美化 JSON；`markdown` 输出逐条消息（含 tool_calls 摘要）。
    pub async fn export_conversation(
        &self,
        id: &str,
        format: &str,
    ) -> Result<String> {
        let Some((conversation, messages)) =
            self.get_conversation(id, u32::MAX, 0).await?
        else {
            return Err(other_err(format!("Conversation not found: {id}")));
        };
        match format {
            "json" => {
                let payload = serde_json::json!({ "conversation": conversation, "messages": messages });
                serde_json::to_string_pretty(&payload)
                    .map_err(|e| other_err(e.to_string()))
            }
            "markdown" => {
                let mut out = format!("# {}\n\n", conversation.title);
                for message in &messages {
                    out.push_str(&format!(
                        "**{}**: {}\n",
                        message.role, message.content
                    ));
                    if let Some(tool_calls) = &message.tool_calls {
                        out.push_str(&format!(
                            "  - tool_calls: {tool_calls}\n"
                        ));
                    }
                }
                Ok(out)
            }
            other => {
                Err(other_err(format!("Unsupported export format: {other}")))
            }
        }
    }

    /// 清空全部会话（需 `confirm`，级联删除消息），返回删除的会话数。
    pub async fn clear_all(&self, confirm: bool) -> Result<usize> {
        if !confirm {
            return Err(other_err("clear_all requires confirm=true"));
        }
        let conn = self.inner.lock().unwrap();
        let deleted = conn
            .execute("DELETE FROM conversations", [])
            .map_err(sqlite_err)?;
        Ok(deleted)
    }

    /// 删除超过保留期的旧会话，返回删除的会话数。
    pub async fn cleanup_old(&self, retention_days: i64) -> Result<usize> {
        let cutoff = Utc::now().timestamp() - retention_days * 86_400;
        let conn = self.inner.lock().unwrap();
        let deleted = conn
            .execute(
                "DELETE FROM conversations WHERE updated_at < ?1",
                params![cutoff],
            )
            .map_err(sqlite_err)?;
        Ok(deleted)
    }

    /// 会话总数（供 `get_ai_status` 使用）。
    pub async fn count_conversations(&self) -> Result<i64> {
        let conn = self.inner.lock().unwrap();
        let count = conn
            .query_row("SELECT COUNT(*) FROM conversations", [], |row| {
                row.get(0)
            })
            .map_err(sqlite_err)?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDb {
        repo: ChatHistoryRepository,
        _dir: std::path::PathBuf,
    }

    impl TestDb {
        fn new() -> Self {
            let dir = std::env::temp_dir()
                .join(format!("ai_workshop_test_{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let repo = ChatHistoryRepository::open(&dir.join("chat.db"))
                .expect("open temp db");
            Self { repo, _dir: dir }
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self._dir);
        }
    }

    #[tokio::test]
    async fn create_add_get_conversation() {
        let db = TestDb::new();
        let conversation =
            db.repo.create_conversation("会话", None).await.unwrap();
        db.repo
            .add_message(models::NewMessage {
                conversation_id: conversation.id.clone(),
                role: "user".to_string(),
                content: "你好".to_string(),
                tool_calls: None,
                tool_call_id: None,
            })
            .await
            .unwrap();
        db.repo
            .add_message(models::NewMessage {
                conversation_id: conversation.id.clone(),
                role: "assistant".to_string(),
                content: "你好！".to_string(),
                tool_calls: None,
                tool_call_id: None,
            })
            .await
            .unwrap();

        let (got, messages) = db
            .repo
            .get_conversation(&conversation.id, 50, 0)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.title, "会话");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
    }

    #[tokio::test]
    async fn rename_conversation() {
        let db = TestDb::new();
        let conversation =
            db.repo.create_conversation("旧标题", None).await.unwrap();
        db.repo
            .rename_conversation(&conversation.id, "新标题")
            .await
            .unwrap();
        let (got, _) = db
            .repo
            .get_conversation(&conversation.id, 50, 0)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.title, "新标题");
    }

    #[tokio::test]
    async fn delete_conversation_cascades_messages() {
        let db = TestDb::new();
        let conversation =
            db.repo.create_conversation("待删除", None).await.unwrap();
        db.repo
            .add_message(models::NewMessage {
                conversation_id: conversation.id.clone(),
                role: "user".to_string(),
                content: "hi".to_string(),
                tool_calls: None,
                tool_call_id: None,
            })
            .await
            .unwrap();

        db.repo.delete_conversation(&conversation.id).await.unwrap();
        assert!(
            db.repo
                .get_conversation(&conversation.id, 50, 0)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn export_conversation_json_and_markdown() {
        let db = TestDb::new();
        let conversation =
            db.repo.create_conversation("导出", None).await.unwrap();
        db.repo
            .add_message(models::NewMessage {
                conversation_id: conversation.id.clone(),
                role: "user".to_string(),
                content: "hello".to_string(),
                tool_calls: None,
                tool_call_id: None,
            })
            .await
            .unwrap();

        let json = db
            .repo
            .export_conversation(&conversation.id, "json")
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("conversation").is_some());
        assert!(parsed.get("messages").is_some());

        let md = db
            .repo
            .export_conversation(&conversation.id, "markdown")
            .await
            .unwrap();
        assert!(md.contains("# 导出"));
        assert!(md.contains("**user**: hello"));

        let err = db.repo.export_conversation(&conversation.id, "csv").await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn clear_all_requires_confirm() {
        let db = TestDb::new();
        db.repo.create_conversation("a", None).await.unwrap();
        db.repo.create_conversation("b", None).await.unwrap();

        assert!(db.repo.clear_all(false).await.is_err());

        let deleted = db.repo.clear_all(true).await.unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(db.repo.count_conversations().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn count_conversations() {
        let db = TestDb::new();
        assert_eq!(db.repo.count_conversations().await.unwrap(), 0);
        db.repo.create_conversation("a", None).await.unwrap();
        db.repo.create_conversation("b", None).await.unwrap();
        assert_eq!(db.repo.count_conversations().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn pagination_across_60_messages() {
        let db = TestDb::new();
        let conversation =
            db.repo.create_conversation("分页", None).await.unwrap();
        for i in 0..60 {
            db.repo
                .add_message(models::NewMessage {
                    conversation_id: conversation.id.clone(),
                    role: "user".to_string(),
                    content: format!("消息 {i}"),
                    tool_calls: None,
                    tool_call_id: None,
                })
                .await
                .unwrap();
        }

        let (_, page1) = db
            .repo
            .get_conversation(&conversation.id, 50, 0)
            .await
            .unwrap()
            .unwrap();
        let (_, page2) = db
            .repo
            .get_conversation(&conversation.id, 50, 50)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(page1.len(), 50);
        assert_eq!(page2.len(), 10);
        assert_eq!(page1[0].content, "消息 0");
        assert_eq!(page2[0].content, "消息 50");
    }
}
