use std::sync::Arc;

use crate::ai_workshop::config::AiWorkshopConfig;
use crate::ai_workshop::providers::anthropic::AnthropicProvider;
use crate::ai_workshop::providers::mock::MockProvider;
use crate::ai_workshop::providers::ollama::OllamaProvider;
use crate::ai_workshop::providers::openai::OpenAIProvider;
use crate::ai_workshop::providers::trait::AiProvider;

/// 根据配置创建 AI 提供商实例。
pub fn create_provider(config: &AiWorkshopConfig) -> Result<Arc<dyn AiProvider>, String> {
	if config.mock_enabled {
		return Ok(Arc::new(MockProvider));
	}
	let name = config
		.default_provider
		.as_deref()
		.ok_or_else(|| "未配置 AI 提供商".to_string())?;
	let provider_config = config
		.providers
		.get(name)
		.ok_or_else(|| format!("未知提供商: {name}"))?;
	if !provider_config.enabled {
		return Err(format!("提供商 {name} 未启用，请在设置中启用"));
	}
	match name {
		"openai" => {
			let api_key = require_api_key(provider_config.api_key_hint.as_deref())?;
			Ok(Arc::new(OpenAIProvider::new(
				api_key,
				provider_config.model.clone(),
				Some("https://api.openai.com/v1".to_string()),
			)))
		}
		"deepseek" => {
			let api_key = require_api_key(provider_config.api_key_hint.as_deref())?;
			Ok(Arc::new(OpenAIProvider::new(
				api_key,
				provider_config.model.clone(),
				Some("https://api.deepseek.com/v1".to_string()),
			)))
		}
		"custom" => {
			let api_key = require_api_key(provider_config.api_key_hint.as_deref())?;
			let base_url = provider_config
				.base_url
				.clone()
				.ok_or_else(|| "自定义提供商需要配置 base_url".to_string())?;
			Ok(Arc::new(OpenAIProvider::new(
				api_key,
				provider_config.model.clone(),
				Some(base_url),
			)))
		}
		"anthropic" => {
			let api_key = require_api_key(provider_config.api_key_hint.as_deref())?;
			Ok(Arc::new(AnthropicProvider::new(
				api_key,
				provider_config.model.clone(),
			)))
		}
		"ollama" => Ok(Arc::new(OllamaProvider::new(
			provider_config.model.clone(),
			provider_config.base_url.clone(),
		))),
		other => Err(format!("未知提供商: {other}")),
	}
}

fn require_api_key(api_key: Option<&str>) -> Result<String, String> {
	api_key
		.map(|key| key.to_string())
		.ok_or_else(|| "请先在设置中配置 API Key".to_string())
}
