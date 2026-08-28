// === AI-WORKSHOP START ===
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 日志环形缓冲区：容量可配置，周期性落盘，供排障与 AI 分析使用。
/// 流 C.5 实现：进程输出重定向捕获游戏日志、周期性落盘。
pub struct LogBuffer {
	inner: Mutex<VecDeque<String>>,
	capacity: usize,
	/// 落盘目标目录（附加后启用行数阈值落盘）。
	dest: Mutex<Option<PathBuf>>,
	/// 距上次落盘多少行触发一次落盘（log_lines / 10）；0 表示仅按时间间隔落盘。
	flush_after: std::sync::atomic::AtomicUsize,
}

impl LogBuffer {
	pub fn new(capacity: usize) -> Self {
		Self {
			inner: Mutex::new(VecDeque::with_capacity(capacity)),
			capacity,
			dest: Mutex::new(None),
			flush_after: std::sync::atomic::AtomicUsize::new(0),
		}
	}

	/// 绑定落盘目录与行数阈值（阈值 = log_lines / 10，默认 50 行）。
	pub fn attach_dest(&self, dir: PathBuf, flush_after: usize) {
		*self.dest.lock().unwrap() = Some(dir);
		self.flush_after.store(flush_after, std::sync::atomic::Ordering::Relaxed);
	}

	pub fn push(&self, line: String) {
		let threshold_reached = {
			let mut inner = self.inner.lock().unwrap();
			while inner.len() >= self.capacity {
				inner.pop_front();
			}
			inner.push_back(line);
			let flush_after = self.flush_after.load(std::sync::atomic::Ordering::Relaxed);
			flush_after > 0 && inner.len() >= flush_after
		};
		// 行数阈值落盘（锁外执行，避免与 flush_to_disk 的 inner 锁重入/死锁）。
		if threshold_reached {
			if let Some(dir) = self.dest.lock().unwrap().clone() {
				let _ = self.flush_to_disk(&dir);
			}
		}
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

/// 测试辅助：建临时目录（测试间命名唯一）。
fn temp_dir(tag: &str) -> std::path::PathBuf {
	let nanos = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap()
		.as_nanos();
	let dir = std::env::temp_dir().join(format!("log_buffer_{tag}_{nanos}"));
	std::fs::create_dir_all(&dir).unwrap();
	dir
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
	fn threshold_flush_flushes_after_n_lines() {
		let dir = temp_dir("threshold");
		let buffer = LogBuffer::new(100);
		buffer.attach_dest(dir.clone(), 5);

		for i in 0..4 {
			buffer.push(format!("line{i}"));
		}
		// 未达阈值：不落盘
		assert!(!dir.join("app.log").exists());

		buffer.push("line4".to_string());
		// 达阈值：落盘且清空
		let raw = std::fs::read_to_string(dir.join("app.log")).unwrap();
		assert!(raw.contains("line4"));
		assert!(raw.contains("line0"), "整批写入，包含较早行");
		assert!(buffer.content().is_empty(), "落盘后缓冲区清空");

		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[test]
	fn threshold_flush_resets_after_flush() {
		let dir = temp_dir("threshold2");
		let buffer = LogBuffer::new(100);
		buffer.attach_dest(dir.clone(), 3);

		for i in 0..3 {
			buffer.push(format!("a{i}"));
		}
		for i in 0..3 {
			buffer.push(format!("b{i}"));
		}
		let raw = std::fs::read_to_string(dir.join("app.log")).unwrap();
		assert!(raw.contains("a2"));
		assert!(raw.contains("b2"));
		// 同一行不会重复追加：第二批发第 3 行后再次落盘，file 只有 6 行内容
		assert_eq!(raw.lines().count(), 6);

		std::fs::remove_dir_all(&dir).unwrap();
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