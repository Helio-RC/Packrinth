use crate::ai_workshop::providers::provider_trait::{
	AiMessage, AiMessageRole, AiProvider, AiResponse, AiUsage, ProviderError, StreamEvent,
	ToolCall, ToolDefinition,
};

/// 无字段的模拟提供商，用于离线演示与测试。
pub struct MockProvider;

const GREETING_JSON: &str = r#"{"content":"你好！我是你的 Minecraft 模组包制作助手。我可以帮你搜索、安装、管理模组，生成 KubeJS / CraftTweaker 脚本，分析崩溃日志，管理实例等。请问有什么可以帮你？","tool_calls":[],"usage":{"prompt_tokens":12,"completion_tokens":40,"total_tokens":52}}"#;

const INSTALL_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_1","name":"search_mods","arguments":{"query":"JEI","limit":10}}],"usage":{"prompt_tokens":20,"completion_tokens":15,"total_tokens":35}}"#;

const SEARCH_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_2","name":"search_mods","arguments":{"query":"sodium","limit":5}}],"usage":{"prompt_tokens":18,"completion_tokens":12,"total_tokens":30}}"#;

const LIST_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_3","name":"list_installed_mods","arguments":{}}],"usage":{"prompt_tokens":15,"completion_tokens":10,"total_tokens":25}}"#;

const REMOVE_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_4","name":"remove_mod","arguments":{"mod_id":"jei"}}],"usage":{"prompt_tokens":16,"completion_tokens":11,"total_tokens":27}}"#;

const UPDATE_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_5","name":"update_mod","arguments":{"mod_id":"sodium"}}],"usage":{"prompt_tokens":16,"completion_tokens":11,"total_tokens":27}}"#;

const CONFIG_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_6","name":"read_config","arguments":{"path":"config/jei.toml"}}],"usage":{"prompt_tokens":17,"completion_tokens":12,"total_tokens":29}}"#;

const WRITE_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_7","name":"write_config","arguments":{"path":"config/jei.toml","content":"[general]\nenabled = true"}}],"usage":{"prompt_tokens":19,"completion_tokens":13,"total_tokens":32}}"#;

const KUBEJS_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_8","name":"generate_kubejs_script","arguments":{"script_type":"recipe","content":"ServerEvents.recipes(event => {\n  event.shaped('minecraft:diamond', ['AAA', 'ABA', 'AAA'], { A: 'minecraft:iron_ingot', B: 'minecraft:gold_ingot' });\n});"}}],"usage":{"prompt_tokens":22,"completion_tokens":18,"total_tokens":40}}"#;

const RECIPE_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_9","name":"generate_crafttweaker_script","arguments":{"recipe":"shaped","output":"minecraft:diamond"}}],"usage":{"prompt_tokens":21,"completion_tokens":16,"total_tokens":37}}"#;

const CRASH_JSON: &str = r#"{"content":"我分析了崩溃日志，看起来是模组 A 与模组 B 之间的冲突导致的。建议你尝试移除其中一个模组，或者更新到兼容版本。需要我帮你进一步分析吗？","tool_calls":[],"usage":{"prompt_tokens":30,"completion_tokens":50,"total_tokens":80}}"#;

const DEPS_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_10","name":"resolve_dependencies","arguments":{"mod_id":"jei"}}],"usage":{"prompt_tokens":20,"completion_tokens":14,"total_tokens":34}}"#;

const GIT_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_11","name":"git_commit","arguments":{"message":"update modpack config"}}],"usage":{"prompt_tokens":18,"completion_tokens":12,"total_tokens":30}}"#;

const CREATE_INSTANCE_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_12","name":"create_instance","arguments":{"name":"My Modpack","game_version":"1.20.1","loader":"fabric"}}],"usage":{"prompt_tokens":24,"completion_tokens":16,"total_tokens":40}}"#;

const INSTANCES_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_13","name":"list_instances","arguments":{}}],"usage":{"prompt_tokens":15,"completion_tokens":10,"total_tokens":25}}"#;

const KNOWLEDGE_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_14","name":"search_knowledge","arguments":{"query":"如何安装 Fabric","top_k":3}}],"usage":{"prompt_tokens":20,"completion_tokens":13,"total_tokens":33}}"#;

const SKILLS_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_15","name":"list_skills","arguments":{}}],"usage":{"prompt_tokens":15,"completion_tokens":10,"total_tokens":25}}"#;

const LAUNCH_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_16","name":"launch_instance","arguments":{"instance_id":"default"}}],"usage":{"prompt_tokens":18,"completion_tokens":12,"total_tokens":30}}"#;

const ROLLBACK_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_17","name":"rollback_config","arguments":{"path":"config/jei.toml","backup_id":"latest"}}],"usage":{"prompt_tokens":20,"completion_tokens":14,"total_tokens":34}}"#;

const ERROR_JSON: &str = r#"{"content":null,"tool_calls":[],"usage":{"prompt_tokens":5,"completion_tokens":0,"total_tokens":5},"error":"模拟的提供商错误：请求失败（mock error case）"}"#;

const STORY_JSON: &str = r#"{"content":"从前有一座 Minecraft 服务器，里面住着一位喜欢研究模组包的玩家。他每天都会尝试新的模组组合，记录下每一个有趣的发现。有一天，他发现了一个神奇的模组，可以让所有方块都变成彩虹色，从此他的世界变得五彩斑斓。","tool_calls":[],"usage":{"prompt_tokens":25,"completion_tokens":80,"total_tokens":105},"stream_chunks":3}"#;

const MULTI_JSON: &str = r#"{"content":null,"tool_calls":[{"id":"call_18","name":"search_mods","arguments":{"query":"JEI","limit":5}},{"id":"call_19","name":"search_mods","arguments":{"query":"sodium","limit":5}}],"usage":{"prompt_tokens":22,"completion_tokens":20,"total_tokens":42}}"#;

const REFUSE_JSON: &str = r#"{"content":"好的，不需要执行任何操作。如果之后需要帮助，随时告诉我！","tool_calls":[],"usage":{"prompt_tokens":10,"completion_tokens":20,"total_tokens":30}}"#;

const FALLBACK_JSON: &str = r#"{"content":"我理解你的意思，但我不太确定具体要做什么。你可以试试让我：搜索/安装模组、生成脚本、分析崩溃日志、管理实例等。","tool_calls":[],"usage":{"prompt_tokens":8,"completion_tokens":30,"total_tokens":38}}"#;

/// 案例集：`(匹配规则, 响应 JSON)`，按数组顺序优先级匹配（更具体的规则在前）。
const MOCK_CASES: &[(&str, &str)] = &[
	("安装多个", MULTI_JSON),
	("multi", MULTI_JSON),
	("已安装", LIST_JSON),
	("list", LIST_JSON),
	("安装", INSTALL_JSON),
	("install", INSTALL_JSON),
	("搜索", SEARCH_JSON),
	("search", SEARCH_JSON),
	("删除", REMOVE_JSON),
	("remove", REMOVE_JSON),
	("更新", UPDATE_JSON),
	("update", UPDATE_JSON),
	("修改配置", WRITE_JSON),
	("write", WRITE_JSON),
	("配置", CONFIG_JSON),
	("config", CONFIG_JSON),
	("kubejs", KUBEJS_JSON),
	("crafttweaker", KUBEJS_JSON),
	("recipe", RECIPE_JSON),
	("配方", RECIPE_JSON),
	("崩溃", CRASH_JSON),
	("crash", CRASH_JSON),
	("依赖", DEPS_JSON),
	("dependency", DEPS_JSON),
	("git", GIT_JSON),
	("提交", GIT_JSON),
	("创建实例", CREATE_INSTANCE_JSON),
	("实例", INSTANCES_JSON),
	("instance", INSTANCES_JSON),
	("知识", KNOWLEDGE_JSON),
	("文档", KNOWLEDGE_JSON),
	("技能", SKILLS_JSON),
	("skill", SKILLS_JSON),
	("启动", LAUNCH_JSON),
	("launch", LAUNCH_JSON),
	("回滚", ROLLBACK_JSON),
	("rollback", ROLLBACK_JSON),
	("报错", ERROR_JSON),
	("故事", STORY_JSON),
	("story", STORY_JSON),
	("不需要", REFUSE_JSON),
	("不用", REFUSE_JSON),
	("你好", GREETING_JSON),
	("hello", GREETING_JSON),
	("", FALLBACK_JSON),
];

/// 取最后一条 user 消息，全小写 contains 匹配，按数组顺序优先级。
fn match_case(messages: &[AiMessage]) -> &'static str {
	let last_user = messages
		.iter()
		.rev()
		.find(|message| matches!(message.role, AiMessageRole::User))
		.map(|message| message.content.to_lowercase())
		.unwrap_or_default();
	for (rule, json) in MOCK_CASES {
		if last_user.contains(rule) {
			return json;
		}
	}
	FALLBACK_JSON
}

fn parse_tool_calls(value: &serde_json::Value) -> Vec<ToolCall> {
	value
		.get("tool_calls")
		.and_then(|calls| calls.as_array())
		.map(|calls| {
			calls
				.iter()
				.map(|call| ToolCall {
					id: call
						.get("id")
						.and_then(|id| id.as_str())
						.unwrap_or_default()
						.to_string(),
					name: call
						.get("name")
						.and_then(|name| name.as_str())
						.unwrap_or_default()
						.to_string(),
					arguments: call
						.get("arguments")
						.cloned()
						.unwrap_or_else(|| serde_json::Value::Object(Default::default())),
				})
				.collect()
		})
		.unwrap_or_default()
}

fn parse_usage(value: &serde_json::Value) -> Option<AiUsage> {
	value.get("usage").map(|usage| {
		let prompt_tokens = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
		let completion_tokens = usage
			.get("completion_tokens")
			.and_then(|v| v.as_u64())
			.unwrap_or(0);
		let total_tokens = usage
			.get("total_tokens")
			.and_then(|v| v.as_u64())
			.unwrap_or(prompt_tokens + completion_tokens);
		AiUsage {
			prompt_tokens,
			completion_tokens,
			total_tokens,
		}
	})
}

#[async_trait::async_trait]
impl AiProvider for MockProvider {
	fn name(&self) -> &'static str {
		"mock"
	}

	async fn chat(
		&self,
		messages: &[AiMessage],
		_tools: &[ToolDefinition],
	) -> Result<AiResponse, ProviderError> {
		let value: serde_json::Value = serde_json::from_str(match_case(messages))
			.map_err(|e| ProviderError(format!("mock 案例解析失败: {e}")))?;
		if let Some(error) = value.get("error").and_then(|e| e.as_str()) {
			return Err(ProviderError(error.to_string()));
		}
		let content = value
			.get("content")
			.and_then(|c| c.as_str())
			.map(|s| s.to_string());
		let tool_calls = parse_tool_calls(&value);
		let usage = parse_usage(&value);
		Ok(AiResponse { content, tool_calls, usage })
	}

	async fn stream(
		&self,
		messages: &[AiMessage],
		_tools: &[ToolDefinition],
		tx: tokio::sync::mpsc::Sender<StreamEvent>,
	) -> Result<(), ProviderError> {
		let value: serde_json::Value = serde_json::from_str(match_case(messages))
			.map_err(|e| ProviderError(format!("mock 案例解析失败: {e}")))?;
		if let Some(error) = value.get("error").and_then(|e| e.as_str()) {
			let _ = tx
				.send(StreamEvent {
					delta: None,
					tool_calls: None,
					usage: None,
					done: true,
					error: Some(error.to_string()),
				})
				.await;
			return Ok(());
		}
		let content = value
			.get("content")
			.and_then(|c| c.as_str())
			.unwrap_or_default()
			.to_string();
		let tool_calls = parse_tool_calls(&value);
		let usage = parse_usage(&value);

		let chars: Vec<char> = content.chars().collect();
		let chunk_count = value
			.get("stream_chunks")
			.and_then(|c| c.as_u64())
			.unwrap_or(0) as usize;
		if chunk_count > 0 && !chars.is_empty() {
			let chunk_size = (chars.len() + chunk_count - 1) / chunk_count;
			for chunk in chars.chunks(chunk_size) {
				let text: String = chunk.iter().collect();
				let _ = tx
					.send(StreamEvent {
						delta: Some(text),
						tool_calls: None,
						usage: None,
						done: false,
						error: None,
					})
					.await;
			}
		} else {
			for chunk in chars.chunks(8) {
				let text: String = chunk.iter().collect();
				let _ = tx
					.send(StreamEvent {
						delta: Some(text),
						tool_calls: None,
						usage: None,
						done: false,
						error: None,
					})
					.await;
			}
		}

		if !tool_calls.is_empty() {
			let _ = tx
				.send(StreamEvent {
					delta: None,
					tool_calls: Some(tool_calls),
					usage: None,
					done: false,
					error: None,
				})
				.await;
		}
		let _ = tx
			.send(StreamEvent {
				delta: None,
				tool_calls: None,
				usage,
				done: true,
				error: None,
			})
			.await;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ai_workshop::providers::provider_trait::{AiMessage, AiMessageRole};

	fn user_message(content: &str) -> Vec<AiMessage> {
		vec![AiMessage {
			role: AiMessageRole::User,
			content: content.to_string(),
			tool_calls: None,
			tool_call_id: None,
			name: None,
		}]
	}

	#[tokio::test]
	async fn greeting_returns_content_without_tools() {
		let provider = MockProvider;
		let response = provider.chat(&user_message("你好"), &[]).await.unwrap();
		let content = response.content.expect("greeting should have content");
		assert!(!content.is_empty());
		assert!(response.tool_calls.is_empty());
		let usage = response.usage.expect("usage should be present");
		assert!(usage.total_tokens > 0);
		assert_eq!(usage.total_tokens, usage.prompt_tokens + usage.completion_tokens);
	}

	#[tokio::test]
	async fn install_triggers_search_mods_tool_call() {
		let provider = MockProvider;
		let response = provider
			.chat(&user_message("帮我安装 JEI"), &[])
			.await
			.unwrap();
		assert_eq!(response.tool_calls.len(), 1);
		assert_eq!(response.tool_calls[0].name, "search_mods");
		let query = response.tool_calls[0]
			.arguments
			.get("query")
			.and_then(|v| v.as_str())
			.unwrap_or_default();
		assert_eq!(query, "JEI");
	}

	#[tokio::test]
	async fn multi_install_triggers_two_tool_calls() {
		let provider = MockProvider;
		let response = provider.chat(&user_message("安装多个"), &[]).await.unwrap();
		assert_eq!(response.tool_calls.len(), 2);
		assert!(response.tool_calls.iter().all(|c| c.name == "search_mods"));
	}

	#[tokio::test]
	async fn error_case_returns_provider_error() {
		let provider = MockProvider;
		let err = provider.chat(&user_message("请报错给我看看"), &[]).await.unwrap_err();
		assert!(err.to_string().contains("模拟的提供商错误"));
	}

	#[tokio::test]
	async fn unmatched_input_falls_back() {
		let provider = MockProvider;
		let response = provider
			.chat(&user_message("qwerty12345不存在的输入"), &[])
			.await
			.unwrap();
		let content = response.content.unwrap_or_default();
		assert!(content.contains("我不太确定具体要做什么"));
	}

	#[tokio::test]
	async fn stream_sends_events_and_terminates() {
		let provider = MockProvider;
		let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamEvent>(16);
		provider.stream(&user_message("你好"), &[], tx).await.unwrap();
		let mut events = Vec::new();
		while let Some(event) = rx.recv().await {
			events.push(event);
		}
		assert!(!events.is_empty(), "stream should emit at least one event");
		assert!(events.iter().any(|e| e.delta.is_some()), "stream should emit deltas");
		assert!(events.last().unwrap().done, "stream should end with done event");
	}

	#[tokio::test]
	async fn match_case_uses_last_user_message() {
		let messages = vec![
			AiMessage {
				role: AiMessageRole::User,
				content: "你好".to_string(),
				tool_calls: None,
				tool_call_id: None,
				name: None,
			},
			AiMessage {
				role: AiMessageRole::Assistant,
				content: "something".to_string(),
				tool_calls: None,
				tool_call_id: None,
				name: None,
			},
			AiMessage {
				role: AiMessageRole::User,
				content: "搜索 sodium".to_string(),
				tool_calls: None,
				tool_call_id: None,
				name: None,
			},
		];
		let value: serde_json::Value =
			serde_json::from_str(match_case(&messages)).expect("valid json");
		let calls = value.get("tool_calls").and_then(|c| c.as_array()).unwrap();
		assert_eq!(calls.len(), 1);
		assert_eq!(calls[0]["name"], "search_mods");
	}
}
