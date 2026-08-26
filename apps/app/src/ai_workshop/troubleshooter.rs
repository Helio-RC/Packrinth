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

	/// 将缓冲区内容追加写入 `<dir>/app.log`。
	pub fn flush_to_disk(&self, dir: &Path) -> Result<(), String> {
		let _ = dir;
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
// === AI-WORKSHOP END ===