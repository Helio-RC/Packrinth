// === AI-WORKSHOP START ===
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// 全局日志缓冲实例：tracing 桥接层在缓冲注册前产生的日志行会丢弃，
/// `install_global` 之后写入该实例（供日志面板/排障读取）。
static GLOBAL_BUFFER: OnceLock<Arc<LogBuffer>> = OnceLock::new();

/// 注册全局 LogBuffer（在 `initialize_after_state` 中调用）。
pub fn install_global(buffer: Arc<LogBuffer>) {
    let _ = GLOBAL_BUFFER.set(buffer);
}

/// 日志环形缓冲区：容量可配置，周期性落盘，供排障与 AI 分析使用。
/// 流 C.5 实现：进程输出重定向捕获游戏日志、周期性落盘。
pub struct LogBuffer {
    inner: Mutex<VecDeque<String>>,
    capacity: usize,
    /// 落盘目标目录（附加后启用行数阈值落盘）。
    dest: Mutex<Option<PathBuf>>,
    /// 距上次落盘多少行触发一次落盘（log_lines / 10）；0 表示仅按时间间隔落盘。
    flush_after: AtomicUsize,
    /// 已写入磁盘的行数（全局行号，包含因容量淘汰的行）。
    persisted: AtomicUsize,
    /// 因容量满而被淘汰的行数。
    dropped: AtomicUsize,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
            dest: Mutex::new(None),
            flush_after: AtomicUsize::new(0),
            persisted: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
        }
    }

    /// 绑定落盘目录与行数阈值（阈值 = log_lines / 10，默认 50 行）。
    pub fn attach_dest(&self, dir: PathBuf, flush_after: usize) {
        *self.dest.lock().unwrap() = Some(dir);
        self.flush_after.store(flush_after, Ordering::Relaxed);
    }

    pub fn push(&self, line: String) {
        let threshold_reached = {
            let mut inner = self.inner.lock().unwrap();
            while inner.len() >= self.capacity {
                inner.pop_front();
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            inner.push_back(line);
            let flush_after = self.flush_after.load(Ordering::Relaxed);
            flush_after > 0 && inner.len() >= flush_after
        };
        // 行数阈值落盘（锁外执行，避免与 flush_to_disk 的 inner 锁重入/死锁）。
        if threshold_reached
            && let Some(dir) = self.dest.lock().unwrap().clone()
        {
            let _ = self.flush_to_disk(&dir);
        }
    }

    /// 将缓冲区中新增的行追加写入 `<dir>/app.log`（增量落盘，不覆盖历史）。
    /// 不清空内存缓冲区（日志面板实时读取），仅推进持久化偏移，避免重复累积。
    pub fn flush_to_disk(&self, dir: &Path) -> Result<(), String> {
        let (content, total) = {
            let inner = self.inner.lock().unwrap();
            let dropped = self.dropped.load(Ordering::Relaxed);
            let persisted = self.persisted.load(Ordering::Relaxed);
            // 缓冲区中仍保留的行全局序号为 [dropped, dropped + len)；
            // 从 persisted 之后的本地偏移开始写，淘汰前的行无从找回则从 0 开始。
            let from_local = persisted.saturating_sub(dropped).min(inner.len());
            let new_lines: Vec<String> =
                inner.iter().skip(from_local).cloned().collect();
            (new_lines.join("\n"), dropped + inner.len())
        };
        if content.is_empty() {
            return Ok(());
        }
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("failed to create logs dir: {e}"))?;
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
        self.persisted.store(total, Ordering::Relaxed);
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

/// tracing 桥接写入器：将应用日志按行写入全局 LogBuffer（日志面板与排障读取）。
/// 缓冲实例在 `install_global` 前未注册，期间产生的日志行会被忽略（启动早期，影响有限）。
pub struct LogBufferWriter;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBufferWriter {
    type Writer = LineBufferWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LineBufferWriter::default()
    }
}

/// 单事件写入器：按换行拆分，完整行推入全局 LogBuffer。
#[derive(Default)]
pub struct LineBufferWriter {
    buf: Vec<u8>,
}

impl std::io::Write for LineBufferWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(data);
        while let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
            let mut raw: Vec<u8> = self.buf.drain(..=pos).collect();
            raw.pop();
            if let Some(buffer) = GLOBAL_BUFFER.get() {
                buffer.push(String::from_utf8_lossy(&raw).to_string());
            }
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for LineBufferWriter {
    fn drop(&mut self) {
        if !self.buf.is_empty()
            && let Some(buffer) = GLOBAL_BUFFER.get()
        {
            buffer.push(String::from_utf8_lossy(&self.buf).to_string());
        }
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
        let dir = std::env::temp_dir()
            .join(format!("packrinth-ai-logg-{tag}-{}", std::process::id()));
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

        let raw = fs::read_to_string(dir.join("app.log"))
            .expect("app.log should exist");
        assert!(raw.contains("line 1"));
        assert!(raw.contains("line 2"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn second_flush_appends_new_lines_only() {
        let dir = temp_dir("append");
        let buffer = LogBuffer::new(100);
        buffer.push("first".to_string());
        buffer
            .flush_to_disk(&dir)
            .expect("first flush should succeed");

        // 首次 flush 后内存缓冲区保留（日志面板实时读取），但偏移已推进：
        // 再次 flush 不应重复追加旧的 "first"。
        buffer.push("second".to_string());
        buffer
            .flush_to_disk(&dir)
            .expect("second flush should succeed");

        let raw = fs::read_to_string(dir.join("app.log"))
            .expect("app.log should exist");
        assert!(
            raw.contains("second"),
            "second flush content must be appended"
        );
        assert_eq!(
            raw.matches("first").count(),
            1,
            "persisted offset must prevent re-appending old lines"
        );
        // 增量落盘不清空内存：面板仍可读取已落盘的行。
        assert_eq!(
            buffer.content(),
            vec!["first".to_string(), "second".to_string()],
            "flush must not clear in-memory buffer"
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
        // 达阈值：增量落盘，内存保留
        let raw = std::fs::read_to_string(dir.join("app.log")).unwrap();
        assert!(raw.contains("line4"));
        assert!(raw.contains("line0"), "整批写入，包含较早行");
        assert_eq!(buffer.content().len(), 5, "落盘后内存缓冲区保留");

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
        assert!(
            !dir.join("app.log").exists(),
            "empty buffer must not create file"
        );
        fs::remove_dir_all(&dir).ok();
    }
}
// === AI-WORKSHOP END ===
