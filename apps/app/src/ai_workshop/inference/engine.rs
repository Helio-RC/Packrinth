use std::sync::Arc;

use serde::Serialize;

use crate::ai_workshop::chat_history::models::NewMessage;
use crate::ai_workshop::providers::factory::create_provider;
use crate::ai_workshop::providers::trait::{
	AiMessage, AiMessageRole, AiProvider, AiUsage, StreamEvent, ToolCall, ToolDefinition,
};
use crate::ai_workshop::AiWorkshopState;
use crate::api::Result;

fn other_err(msg: impl Into<String>) -> crate::api::TheseusSerializableError {
	crate::api::TheseusSerializableError::Theseus(
		theseus::Error::from(theseus::ErrorKind::OtherError(msg.into())),
	)
}

/// 单轮对话结果。
#[derive(Clone, Debug, Serialize)]
pub struct ChatResult {
	pub reply: String,
	pub usage: serde_json::Value,
}

/// 推理引擎：负责消息构建、提供商调用与工具执行循环。
pub struct InferenceEngine {
	state: Arc<AiWorkshopState>,
}

impl InferenceEngine {
	pub fn new(state: Arc<AiWorkshopState>) -> Self {
		Self { state }
	}

	/// 单轮：构建消息 → provider.chat → 若有 tool_calls 则逐条执行（写入 tool 消息）→ 返回最终回复。
	pub async fn run_single_turn(&self, conversation_id: &str, content: &str) -> Result<ChatResult> {
		let context =
			super::context::InferenceContext::new(self.state.clone(), conversation_id.to_string());
		let mut messages = context.build_messages(content).await;
		messages = context.trim(messages).await;

		let provider = create_provider(&self.state.config_manager.config()).map_err(other_err)?;
		let tools = self.tool_definitions();

		let response = provider
			.chat(&messages, &tools)
			.await
			.map_err(|e| other_err(e.to_string()))?;

		let mut reply = response.content.clone().unwrap_or_default();
		let mut usage = serde_json::to_value(&response.usage).unwrap_or(serde_json::Value::Null);

		if !response.tool_calls.is_empty() {
			messages.push(AiMessage {
				role: AiMessageRole::Assistant,
				content: reply.clone(),
				tool_calls: Some(response.tool_calls.clone()),
				tool_call_id: None,
				name: None,
			});
			for call in &response.tool_calls {
				let result = self.execute_tool(call).await;
				messages.push(AiMessage::tool_result(call.id.clone(), result));
			}
			let response = provider
				.chat(&messages, &tools)
				.await
				.map_err(|e| other_err(e.to_string()))?;
			reply = response.content.clone().unwrap_or_default();
			usage = serde_json::to_value(&response.usage).unwrap_or(serde_json::Value::Null);
		}

		Ok(ChatResult { reply, usage })
	}

	/// 多轮：provider.stream 循环（最大 max_tool_iterations 轮），每轮流式事件经 on_event 转发。
	pub async fn run_multi_turn(
		&self,
		conversation_id: &str,
		content: &str,
		mut on_event: Box<dyn FnMut(StreamEvent) + Send>,
	) -> Result<()> {
		let context =
			super::context::InferenceContext::new(self.state.clone(), conversation_id.to_string());
		let mut messages = context.build_messages(content).await;
		messages = context.trim(messages).await;

		let provider = create_provider(&self.state.config_manager.config()).map_err(other_err)?;
		let tools = self.tool_definitions();
		let max_iterations = self.state.config_manager.config().max_tool_iterations.max(1);

		for _ in 0..max_iterations {
			let (reply, tool_calls, usage) =
				self.run_stream_round(&provider, &messages, &tools, &mut on_event).await?;

			if tool_calls.is_empty() {
				self.persist_assistant(conversation_id, &reply, &[]).await?;
				on_event(StreamEvent {
					delta: None,
					tool_calls: None,
					usage,
					done: true,
					error: None,
				});
				return Ok(());
			}

			on_event(StreamEvent {
				delta: Some(reply.clone()),
				tool_calls: Some(tool_calls.clone()),
				usage: None,
				done: false,
				error: None,
			});

			let mut tool_messages = Vec::new();
			for call in &tool_calls {
				let result = if self.tool_requires_confirmation(&call.name) {
					let approved = self.wait_for_confirmation(conversation_id, &call.id).await;
					if approved {
						self.execute_tool(call).await
					} else {
						let message = format!("错误：工具 {} 未获用户确认，已跳过", call.name);
						on_event(StreamEvent {
							delta: None,
							tool_calls: None,
							usage: None,
							done: false,
							error: Some(message.clone()),
						});
						message
					}
				} else {
					self.execute_tool(call).await
				};
				tool_messages.push(AiMessage::tool_result(call.id.clone(), result));
			}

			self.persist_assistant(conversation_id, &reply, &tool_calls).await?;
			for tool_message in &tool_messages {
				self.persist_tool_message(conversation_id, tool_message).await?;
			}

			messages.push(AiMessage {
				role: AiMessageRole::Assistant,
				content: reply,
				tool_calls: Some(tool_calls),
				tool_call_id: None,
				name: None,
			});
			messages.extend(tool_messages);
			messages = context.trim(messages).await;
		}

		on_event(StreamEvent {
			delta: None,
			tool_calls: None,
			usage: None,
			done: true,
			error: None,
		});
		Ok(())
	}

	/// 执行一轮流式推理，转发事件并收集回复 / 工具调用 / 用量。
	async fn run_stream_round(
		&self,
		provider: &Arc<dyn AiProvider>,
		messages: &[AiMessage],
		tools: &[ToolDefinition],
		on_event: &mut Box<dyn FnMut(StreamEvent) + Send>,
	) -> Result<(String, Vec<ToolCall>, Option<AiUsage>)> {
		let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamEvent>(64);
		let provider = provider.clone();
		let messages = messages.to_vec();
		let tools = tools.to_vec();
		let stream_task = tokio::spawn(async move { provider.stream(&messages, &tools, tx).await });

		let mut reply = String::new();
		let mut tool_calls = Vec::new();
		let mut usage = None;
		let mut error = None;
		while let Some(event) = rx.recv().await {
			if let Some(delta) = &event.delta {
				reply.push_str(delta);
			}
			if let Some(calls) = &event.tool_calls {
				tool_calls.extend(calls.clone());
			}
			if let Some(u) = &event.usage {
				usage = Some(u.clone());
			}
			if let Some(e) = &event.error {
				error = Some(e.clone());
			}
			on_event(event);
		}

		let stream_result = stream_task
			.await
			.map_err(|e| other_err(format!("流式任务失败: {e}")))?;
		if let Err(e) = stream_result {
			return Err(other_err(e.to_string()));
		}
		if let Some(e) = error {
			return Err(other_err(e));
		}

		Ok((reply, tool_calls, usage))
	}

	/// 执行单个工具，返回工具结果 JSON 字符串。
	async fn execute_tool(&self, call: &ToolCall) -> String {
		let Some(tool) = self.state.tool_registry.get(&call.name) else {
			return format!("错误：未知工具 {}", call.name);
		};
		let task_id = uuid::Uuid::new_v4().to_string();
		let context = crate::ai_workshop::tools::context::ExecutionContext {
			instance_id: None,
			cancellation_token: self
				.state
				.task_registry
				.new_token(&task_id)
				.unwrap_or_default(),
			..Default::default()
		};
		match tool.execute(call.arguments.clone(), &context).await {
			Ok(value) => serde_json::to_string(&value).unwrap_or_else(|_| value.to_string()),
			Err(e) => format!("错误：{e}"),
		}
	}

	/// 工具是否需要用户确认。
	fn tool_requires_confirmation(&self, name: &str) -> bool {
		self.state
			.tool_registry
			.get(name)
			.map(|tool| tool.requires_confirmation())
			.unwrap_or(false)
	}

	/// 轮询 chat_history 等待用户确认，超时视为拒绝。
	async fn wait_for_confirmation(&self, conversation_id: &str, tool_call_id: &str) -> bool {
		let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
		while std::time::Instant::now() < deadline {
			if let Ok(Some((_, messages))) = self
				.state
				.chat_history
				.get_conversation(conversation_id, 100, 0)
				.await
			{
				for message in messages.iter().rev() {
					if message.role == "tool" && message.tool_call_id.as_deref() == Some(tool_call_id)
					{
						if let Some(approved) = parse_confirmation(&message.content) {
							return approved;
						}
					}
				}
			}
			tokio::time::sleep(std::time::Duration::from_millis(500)).await;
		}
		false
	}

	/// 收集全部已注册工具为 ToolDefinition 列表。
	fn tool_definitions(&self) -> Vec<ToolDefinition> {
		self.state
			.tool_registry
			.list()
			.iter()
			.map(|info| ToolDefinition {
				name: info.name.clone(),
				description: info.description.clone(),
				parameters: openai_parameters(&info.params_schema),
			})
			.collect()
	}

	async fn persist_assistant(
		&self,
		conversation_id: &str,
		reply: &str,
		tool_calls: &[ToolCall],
	) -> Result<()> {
		let tool_calls_json = if tool_calls.is_empty() {
			None
		} else {
			Some(serde_json::to_string(tool_calls).map_err(|e| other_err(e.to_string()))?)
		};
		self.state
			.chat_history
			.add_message(NewMessage {
				conversation_id: conversation_id.to_string(),
				role: "assistant".to_string(),
				content: reply.to_string(),
				tool_calls: tool_calls_json,
				tool_call_id: None,
			})
			.await?;
		Ok(())
	}

	async fn persist_tool_message(&self, conversation_id: &str, message: &AiMessage) -> Result<()> {
		self.state
			.chat_history
			.add_message(NewMessage {
				conversation_id: conversation_id.to_string(),
				role: "tool".to_string(),
				content: message.content.clone(),
				tool_calls: None,
				tool_call_id: message.tool_call_id.clone(),
			})
			.await?;
		Ok(())
	}
}

/// 解析确认消息内容（"approved"/"rejected" 或 {"approved": bool}）。
fn parse_confirmation(content: &str) -> Option<bool> {
	if let Ok(value) = serde_json::from_str::<serde_json::Value>(content) {
		if let Some(approved) = value.get("approved").and_then(|a| a.as_bool()) {
			return Some(approved);
		}
	}
	let lower = content.to_lowercase();
	if lower.contains("approved") || lower.contains("true") {
		Some(true)
	} else if lower.contains("rejected") || lower.contains("false") {
		Some(false)
	} else {
		None
	}
}

/// 从 ToolInfo.params_schema 提取 OpenAI 兼容的 parameters（RootSchema 取 .schema 字段）。
fn openai_parameters(params_schema: &serde_json::Value) -> serde_json::Value {
	params_schema
		.get("schema")
		.cloned()
		.unwrap_or_else(|| params_schema.clone())
}
