use std::sync::Arc;

use crate::ai_workshop::chat_history::models::Message;
use crate::ai_workshop::providers::provider_trait::{AiMessage, AiMessageRole, ToolCall};
use crate::ai_workshop::AiWorkshopState;

/// 推理上下文：负责构建系统提示、注入技能与知识、读取历史消息。
pub struct InferenceContext {
	state: Arc<AiWorkshopState>,
	conversation_id: String,
}

impl InferenceContext {
	pub fn new(state: Arc<AiWorkshopState>, conversation_id: String) -> Self {
		Self { state, conversation_id }
	}

	/// 固定系统提示：你是 Minecraft 模组包制作助手（含工具使用说明）。
	pub fn system_prompt(&self) -> String {
		"你是 Minecraft 模组包制作助手，运行在 Modrinth App 中。你可以帮助用户搜索、安装、管理模组，生成 KubeJS / CraftTweaker 脚本，分析崩溃日志，管理实例，解析依赖等。\n\n你可以调用工具来完成任务。当用户请求涉及工具能力时，请调用相应工具；工具返回结果后，根据结果组织自然语言回复。\n\n规则：\n- 只使用提供的工具，不要编造工具名。\n- 工具参数必须符合 JSON Schema。\n- 如果用户请求不明确，先询问澄清。\n- 涉及写操作（安装/删除/修改配置等）时，等待用户确认后再执行。"
			.to_string()
	}

	/// 构建完整消息列表：系统提示（含技能与知识注入）→ 历史消息 → 当前用户消息。
	pub async fn build_messages(&self, user_message: &str) -> Vec<AiMessage> {
		let mut messages = Vec::new();

		let mut system = self.system_prompt();
		let max_inject = self.state.config_manager.config().skills.max_inject_count;
		for skill in self.state.skill_loader.enabled_skills().iter().take(max_inject) {
			let guide: String = skill.guide_md.chars().take(2000).collect();
			system.push_str(&format!("\n\n## 技能: {}\n{}", skill.name, guide));
		}

		// 知识检索启用时，将结果注入为 system 附加段
		let config = self.state.config_manager.config();
		if !config.knowledge.allowed_domains.is_empty() {
			if let Ok(results) = self
				.state
				.knowledge_router
				.search(user_message, 3, None)
				.await
			{
				if !results.is_empty() {
					let mut knowledge = String::from("## 知识参考\n");
					for (index, result) in results.iter().enumerate() {
						knowledge.push_str(&format!("{}. {}\n", index + 1, result));
					}
					system.push_str(&format!("\n\n{knowledge}"));
				}
			}
		}
		messages.push(AiMessage::system(system));

		if let Ok(Some((_, history))) = self
			.state
			.chat_history
			.get_conversation(&self.conversation_id, 20, 0)
			.await
		{
			for message in history {
				if let Some(ai_message) = history_message_to_ai(&message) {
					messages.push(ai_message);
				}
			}
		}

		messages.push(AiMessage::user(user_message.to_string()));

		messages
	}

	/// 简单按总字符数 120_000 截断并保留首尾。
	pub async fn trim(&self, messages: Vec<AiMessage>) -> Vec<AiMessage> {
		let total: usize = messages.iter().map(|message| message.content.len()).sum();
		if total <= 120_000 {
			return messages;
		}
		let mut trimmed = Vec::new();
		if let Some(first) = messages.first() {
			trimmed.push(first.clone());
		}
		let mut budget = 120_000usize
			.saturating_sub(trimmed.first().map(|message| message.content.len()).unwrap_or(0));
		for message in messages.iter().rev().skip(1) {
			if budget >= message.content.len() {
				trimmed.push(message.clone());
				budget -= message.content.len();
			} else {
				break;
			}
		}
		trimmed.reverse();
		trimmed
	}
}

/// 将历史消息转换为 AiMessage（tool_calls 字段按 JSON 解析）。
fn history_message_to_ai(message: &Message) -> Option<AiMessage> {
	let role = match message.role.as_str() {
		"system" => AiMessageRole::System,
		"user" => AiMessageRole::User,
		"assistant" => AiMessageRole::Assistant,
		"tool" => AiMessageRole::Tool,
		_ => return None,
	};
	let tool_calls = message
		.tool_calls
		.as_deref()
		.and_then(|raw| serde_json::from_str::<Vec<ToolCall>>(raw).ok());
	Some(AiMessage {
		role,
		content: message.content.clone(),
		tool_calls,
		tool_call_id: message.tool_call_id.clone(),
		name: None,
	})
}
