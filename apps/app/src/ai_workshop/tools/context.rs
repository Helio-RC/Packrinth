// === AI-WORKSHOP START ===
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 任务取消令牌：工具在循环/IO 间隙调用 `is_cancelled()` 检测取消。
#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
	pub fn cancel(&self) {
		self.0.store(true, Ordering::Relaxed);
	}

	pub fn is_cancelled(&self) -> bool {
		self.0.load(Ordering::Relaxed)
	}
}

/// 工具执行上下文：承载实例标识、取消令牌与进度上报等运行时信息。
#[derive(Default)]
pub struct ExecutionContext {
	pub instance_id: Option<String>,
	pub cancellation_token: CancellationToken,
}

/// 按实例串行化写操作的锁管理器（写工具进入前获取）。
#[derive(Default)]
pub struct InstanceLockManager;

/// 进行中任务注册表：`task_id` → 取消令牌，供前端取消操作使用。
#[derive(Default)]
pub struct TaskRegistry {
	tasks: Mutex<HashMap<String, CancellationToken>>,
}

impl TaskRegistry {
	pub fn new_token(&self, task_id: &str) -> Option<CancellationToken> {
		let token = CancellationToken::default();
		self.tasks
			.lock()
			.unwrap()
			.insert(task_id.to_string(), token.clone());
		Some(token)
	}

	pub fn cancel(&self, task_id: &str) -> bool {
		if let Some(token) = self.tasks.lock().unwrap().remove(task_id) {
			token.cancel();
			true
		} else {
			false
		}
	}

	pub fn cancel_all(&self) {
		for token in self.tasks.lock().unwrap().drain() {
			token.1.cancel();
		}
	}
}
// === AI-WORKSHOP END ===