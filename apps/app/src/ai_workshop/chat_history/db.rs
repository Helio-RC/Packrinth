use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::api::Result;

pub(crate) fn sqlite_err(
    e: rusqlite::Error,
) -> crate::api::TheseusSerializableError {
    crate::api::TheseusSerializableError::Theseus(theseus::Error::from(
        theseus::ErrorKind::OtherError(e.to_string()),
    ))
}

pub(crate) fn other_err(
    msg: impl Into<String>,
) -> crate::api::TheseusSerializableError {
    crate::api::TheseusSerializableError::Theseus(theseus::Error::from(
        theseus::ErrorKind::OtherError(msg.into()),
    ))
}

/// 打开（或创建）对话历史数据库，启用 WAL 与外键，校验完整性并建表。
pub fn open(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path).map_err(sqlite_err)?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(sqlite_err)?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(sqlite_err)?;

    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(sqlite_err)?;
    if integrity != "ok" {
        return Err(other_err(format!(
            "SQLite integrity check failed: {integrity}"
        )));
    }

    conn.execute_batch(
		"CREATE TABLE IF NOT EXISTS conversations (
			id TEXT PRIMARY KEY,
			title TEXT NOT NULL,
			instance_id TEXT,
			created_at INTEGER NOT NULL,
			updated_at INTEGER NOT NULL
		);
		CREATE TABLE IF NOT EXISTS messages (
			id TEXT PRIMARY KEY,
			conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
			role TEXT NOT NULL,
			content TEXT NOT NULL,
			tool_calls TEXT,
			tool_call_id TEXT,
			created_at INTEGER NOT NULL
		);
		CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id, created_at);
		CREATE INDEX IF NOT EXISTS idx_conversations_updated ON conversations(updated_at DESC);",
	)
	.map_err(sqlite_err)?;

    Ok(conn)
}

/// 每日自动备份：数据库文件最后修改日期非今天时，用 `VACUUM INTO` 生成一致性快照
/// `chat_backup_YYYYMMDD.db`（WAL 模式下直接复制文件不安全），并仅保留最近 7 份备份。
pub fn backup_if_due(conn: &Connection, db_path: &Path) -> Result<()> {
    let meta = match std::fs::metadata(db_path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let modified: DateTime<Utc> = meta.modified()?.into();
    let modified_date = modified.date_naive();
    if modified_date == Utc::now().date_naive() {
        return Ok(());
    }

    let Some(parent) = db_path.parent() else {
        return Ok(());
    };
    let backup_path = parent
        .join(format!("chat_backup_{}.db", modified_date.format("%Y%m%d")));
    if !backup_path.exists() {
        let escaped = backup_path.to_string_lossy().replace('\'', "''");
        conn.execute_batch(&format!("VACUUM INTO '{escaped}'"))
            .map_err(sqlite_err)?;
    }

    let mut backups: Vec<PathBuf> = std::fs::read_dir(parent)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().and_then(|name| name.to_str()).is_some_and(
                |name| {
                    name.starts_with("chat_backup_") && name.ends_with(".db")
                },
            )
        })
        .collect();
    backups.sort();
    while backups.len() > 7 {
        if let Some(oldest) = backups.first() {
            let _ = std::fs::remove_file(oldest);
        }
        backups.remove(0);
    }

    Ok(())
}
