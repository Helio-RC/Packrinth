// === AI-WORKSHOP START ===
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;

/// 日志环形缓冲区：容量可配置，周期性落盘，供排障与 AI 分析使用。
/// 流 C.5 实现：进程输出重定向捕获游戏日志、周期性落盘。
pub struct LogBuffer {
	inner: Mutex<VecDeque<String>>,
	capacity: usize,
}

impl LogBuffer {
	pub fn new(capacity: usize) -> Self {
		Self {
			inner: Mutex::new(VecDeque::with_capacity(capacity)),
			capacity,
		}
	}

	pub fn push(&self, line: String) {
		let mut inner = self.inner.lock().unwrap();
		while inner.len() >= self.capacity {
			inner.pop_front();
		}
		inner.push_back(line);
	}

	/// 将缓冲区内容追加写入 `<dir>/app.log`（不覆盖历史）。
	/// 写入成功后清空缓冲区（drain），避免周期性落盘重复累积已写过的日志。
	pub fn flush_to_disk(&self, dir: &Path) -> Result<(), String> {
		let content = self.content().join("\n");
		if content.is_empty() {
			return Ok(());
		}
		std::fs::create_dir_all(dir).map_err(|e| format!("failed to create logs dir: {e}"))?;
		let path = dir.join("app.log");
		let mut file = std::fs::OpenOptions::new()
			.create(true)
			.append(true)
			.open(&path)
			.map_err(|e| format!("failed to open {}: {e}", path.display()))?;
		use std::io::Write;
		file.write_all(content.as_bytes())
			.map_err(|e| format!("failed to write {}: {e}", path.display()))?;
		file.write_all(b"\n")
			.map_err(|e| format!("failed to write {}: {e}", path.display()))?;
		// 落盘成功后清空缓冲区。
		self.inner.lock().unwrap().clear();
		Ok(())
	}

	pub fn content(&self) -> Vec<String> {
		self.inner.lock().unwrap().iter().cloned().collect()
	}

	pub fn tail(&self, limit: usize) -> Vec<String> {
		let inner = self.inner.lock().unwrap();
		let start = inner.len().saturating_sub(limit);
		inner.iter().skip(start).cloned().collect()
	}

	pub fn capacity(&self) -> usize {
		self.capacity
	}

	/// 仅测试/调试用：注入虚假日志触发排障流程。
	pub fn inject(&self, log: String) {
		self.push(log);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::fs;

	fn temp_dir(tag: &str) -> std::path::PathBuf {
		let dir = std::env::temp_dir().join(format!(
			"packrinth-ai-logg-{tag}-{}",
			std::process::id()
		));
		let _ = fs::remove_dir_all(&dir);
		dir
	}

	#[test]
	fn flush_writes_buffered_lines_to_app_log() {
		let dir = temp_dir("write");
		let buffer = LogBuffer::new(100);
		buffer.push("line 1".to_string());
		buffer.push("line 2".to_string());

		buffer.flush_to_disk(&dir).expect("flush should succeed");

		let raw = fs::read_to_string(dir.join("app.log")).expect("app.log should exist");
		assert!(raw.contains("line 1"));
		assert!(raw.contains("line 2"));
		fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn second_flush_appends_new_lines_only() {
		let dir = temp_dir("append");
		let buffer = LogBuffer::new(100);
		buffer.push("first".to_string());
		buffer.flush_to_disk(&dir).expect("first flush should succeed");

		// flush 成功后缓冲区已清空：再次 flush 不应重复追加旧的 "first"。
		buffer.push("second".to_string());
		buffer.flush_to_disk(&dir).expect("second flush should succeed");

		let raw = fs::read_to_string(dir.join("app.log")).expect("app.log should exist");
		assert!(raw.contains("second"), "second flush content must be appended");
		// 缓冲区在首次 flush 后被 drain，故 "first" 只出现一次（追加，不覆盖，也不重复累积）。
		assert_eq!(
			raw.matches("first").count(),
			1,
			"flushed buffer must be drained so old lines are not re-appended"
		);
		fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn flush_empty_buffer_creates_no_file() {
		let dir = temp_dir("empty");
		let buffer = LogBuffer::new(100);
		buffer.flush_to_disk(&dir).expect("flush should succeed");
		assert!(!dir.join("app.log").exists(), "empty buffer must not create file");
		fs::remove_dir_all(&dir).ok();
	}
}
// === AI-WORKSHOP END ===