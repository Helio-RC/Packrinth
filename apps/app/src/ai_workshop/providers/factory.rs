use std::sync::Arc;

use crate::ai_workshop::config::ConfigManager;
use crate::ai_workshop::providers::anthropic::AnthropicProvider;
use crate::ai_workshop::providers::mock::MockProvider;
use crate::ai_workshop::providers::ollama::OllamaProvider;
use crate::ai_workshop::providers::openai::OpenAIProvider;
use crate::ai_workshop::providers::provider_trait::AiProvider;

/// 根据配置创建 AI 提供商实例。
/// `provider_name` 缺省时使用 `config.default_provider`。真实 Key 从
/// `config_manager.get_decrypted_api_key` 获取（而非掩码提示）。
pub fn create_provider(
    config_manager: &ConfigManager,
    provider_name: Option<&str>,
) -> Result<Arc<dyn AiProvider>, String> {
    let config = config_manager.config();
    if config.mock_enabled {
        return Ok(Arc::new(MockProvider));
    }
    let name = provider_name
        .or(config.default_provider.as_deref())
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
            let api_key = require_api_key(config_manager, name)?;
            Ok(Arc::new(OpenAIProvider::new(
                api_key,
                provider_config.model.clone(),
                Some("https://api.openai.com/v1".to_string()),
            )))
        }
        "deepseek" => {
            let api_key = require_api_key(config_manager, name)?;
            Ok(Arc::new(OpenAIProvider::new(
                api_key,
                provider_config.model.clone(),
                Some("https://api.deepseek.com/v1".to_string()),
            )))
        }
        "custom" => {
            let api_key = require_api_key(config_manager, name)?;
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
            let api_key = require_api_key(config_manager, name)?;
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

/// 从 config_manager 解密真实 Key；未配置时返回明确错误（绝不使用掩码提示）。
fn require_api_key(
    config_manager: &ConfigManager,
    provider: &str,
) -> Result<String, String> {
    config_manager
        .get_decrypted_api_key(provider)
        .ok_or_else(|| format!("API Key 未配置，请在设置中配置 {provider}"))
}
