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

/// 工具进度上报载荷：步骤名、可选百分比、可选消息。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
	pub step: String,
	pub percent: Option<f32>,
	pub message: Option<String>,
}

/// 工具执行上下文：承载实例标识、取消令牌、进度上报与实例写锁管理器等运行时信息。
#[derive(Default)]
pub struct ExecutionContext {
	pub instance_id: Option<String>,
	pub cancellation_token: CancellationToken,
	/// 进度回调（由前端 `tool-progress` 事件接线；AI 引擎执行时为空）。
	pub emit_progress: Option<Box<dyn Fn(&ProgressPayload) + Send + Sync>>,
	/// 实例写锁管理器：写工具进入前获取锁，串行化对同一实例的写操作。
	/// 由引擎 / 手动工具面板共享同一实例，保证跨入口互斥。
	pub instance_lock_manager: Arc<InstanceLockManager>,
}

impl ExecutionContext {
	/// 上报执行进度：若配置了 emit_progress 回调则调用（best-effort，忽略回调错误）。
	pub fn report_progress(
		&self,
		step: impl Into<String>,
		percent: Option<f32>,
		message: Option<String>,
	) {
		if let Some(emit) = &self.emit_progress {
			emit(&ProgressPayload {
				step: step.into(),
				percent,
				message,
			});
		}
	}
}

/// 按实例串行化写操作的锁管理器（写工具进入前获取）。
pub struct InstanceLockManager {
	inner: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl Default for InstanceLockManager {
	fn default() -> Self {
		Self {
			inner: Mutex::new(HashMap::new()),
		}
	}
}

impl InstanceLockManager {
	/// 获取指定实例的写锁。同一实例上的并发写操作会等待；`timeout` 内未获锁
	/// 返回明确错误。使用 `OwnedMutexGuard`：guard 持有底层 Arc，锁的所有权
	/// 独立于本管理器存活，调用方持 guard 期间保持互斥。
	/// 注意：禁止嵌套获取——同一工具内不得再次调用本方法（工具间无嵌套调用，
	/// 天然满足）。实例写锁为同一实例跨工具/跨入口的串行化互斥。
	pub async fn acquire_write_lock(
		&self,
		instance_id: &str,
		timeout: std::time::Duration,
	) -> Result<tokio::sync::OwnedMutexGuard<()>, String> {
		let lock = self
			.inner
			.lock()
			.unwrap()
			.entry(instance_id.to_string())
			.or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
			.clone();
		match tokio::time::timeout(timeout, lock.lock_owned()).await {
			Ok(guard) => Ok(guard),
			Err(_) => Err("另一个操作正在修改此实例，请稍后重试".to_string()),
		}
	}
}

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

	/// 移除已完成任务的取消令牌，返回被移除的令牌（若存在）。
	/// 工具正常完成（Ok/Err）后调用，避免长期会话中 HashMap 无限增长。
	/// 与 cancel 不同：只移除令牌，不触发取消。
	pub fn remove(&self, task_id: &str) -> Option<CancellationToken> {
		self.tasks.lock().unwrap().remove(task_id)
	}

	pub fn cancel_all(&self) {
		for token in self.tasks.lock().unwrap().drain() {
			token.1.cancel();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::Arc;

	#[test]
	fn report_progress_invokes_callback_with_payload() {
		let recorded = Arc::new(Mutex::new(Vec::new()));
		let recorded_clone = recorded.clone();
		let ctx = ExecutionContext {
			instance_id: None,
			cancellation_token: CancellationToken::default(),
			instance_lock_manager: Arc::new(InstanceLockManager::default()),
			emit_progress: Some(Box::new(move |p: &ProgressPayload| {
				recorded_clone
					.lock()
					.unwrap()
					.push((p.step.clone(), p.percent, p.message.clone()));
			})),
		};

		ctx.report_progress("download", Some(42.5), Some("下载中".to_string()));
		ctx.report_progress("install", None, None);

		let recorded = recorded.lock().unwrap();
		assert_eq!(recorded.len(), 2);
		assert_eq!(recorded[0].0, "download");
		assert_eq!(recorded[0].1, Some(42.5));
		assert_eq!(recorded[0].2.as_deref(), Some("下载中"));
		assert_eq!(recorded[1].0, "install");
		assert_eq!(recorded[1].1, None);
	}

	#[test]
	fn report_progress_is_noop_without_callback() {
		let ctx = ExecutionContext::default();
		ctx.report_progress("step", Some(1.0), None);
	}

	#[tokio::test]
	async fn write_lock_times_out_when_held() {
		let manager = InstanceLockManager::default();
		let _guard = manager
			.acquire_write_lock("inst-1", std::time::Duration::from_secs(5))
			.await
			.unwrap();

		let err = manager
			.acquire_write_lock("inst-1", std::time::Duration::from_millis(50))
			.await
			.unwrap_err();
		assert!(err.contains("另一个操作正在修改此实例"), "got: {err}");
	}

	#[tokio::test]
	async fn write_lock_released_after_guard_dropped() {
		let manager = InstanceLockManager::default();
		{
			let _guard = manager
				.acquire_write_lock("inst-1", std::time::Duration::from_secs(5))
				.await
				.unwrap();
		}
		// guard 已释放，应立即重新获得锁。
		let _guard = manager
			.acquire_write_lock("inst-1", std::time::Duration::from_millis(50))
			.await
			.expect("lock should be re-acquirable after release");
	}

	#[tokio::test]
	async fn write_lock_serializes_same_instance_only() {
		let manager = InstanceLockManager::default();
		// 不同实例互不阻塞。
		let _a = manager
			.acquire_write_lock("inst-a", std::time::Duration::from_secs(5))
			.await
			.unwrap();
		let _b = manager
			.acquire_write_lock("inst-b", std::time::Duration::from_millis(50))
			.await
			.expect("different instances must not block each other");
	}

	#[tokio::test]
	async fn write_lock_blocks_concurrent_task_until_released() {
		let manager = Arc::new(InstanceLockManager::default());
		let guard = manager
			.acquire_write_lock("inst-1", std::time::Duration::from_secs(5))
			.await
			.unwrap();

		let manager2 = manager.clone();
		let acquired = Arc::new(tokio::sync::Mutex::new(false));
		let acquired2 = acquired.clone();
		let worker = tokio::spawn(async move {
			// 在持有者释放前，短超时应失败；改为先等待释放信号。
			let err = manager2
				.acquire_write_lock("inst-1", std::time::Duration::from_millis(50))
				.await;
			assert!(err.is_err(), "concurrent acquire should time out while held");
			*acquired2.lock().await = true;
		});

		// 稍作等待让 worker 尝试获取并超时，再释放锁。
		tokio::time::sleep(std::time::Duration::from_millis(100)).await;
		drop(guard);
		worker.await.unwrap();
		assert!(*acquired.lock().await, "worker should have finished");
	}

	#[test]
	fn task_registry_remove_cleans_token_without_cancelling() {
		let registry = TaskRegistry::default();
		let token = registry.new_token("t1").unwrap();
		assert!(!token.is_cancelled());

		let removed = registry.remove("t1");
		assert!(removed.is_some(), "remove should return the existing token");
		assert!(!removed.unwrap().is_cancelled(), "remove must not cancel the token");

		// 再次 remove 返回 None（已清理）。
		assert!(registry.remove("t1").is_none());
		// cancel 对已清理任务返回 false。
		assert!(!registry.cancel("t1"));
	}
}
// === AI-WORKSHOP END ===