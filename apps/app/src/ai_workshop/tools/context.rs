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

/// 工具执行上下文：承载实例标识、取消令牌与进度上报等运行时信息。
#[derive(Default)]
pub struct ExecutionContext {
	pub instance_id: Option<String>,
	pub cancellation_token: CancellationToken,
	/// 进度回调（由前端 `tool-progress` 事件接线；AI 引擎执行时为空）。
	pub emit_progress: Option<Box<dyn Fn(&ProgressPayload) + Send + Sync>>,
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
}
// === AI-WORKSHOP END ===