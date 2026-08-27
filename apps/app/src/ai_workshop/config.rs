use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::api::Result;

fn other_err(msg: impl Into<String>) -> crate::api::TheseusSerializableError {
	crate::api::TheseusSerializableError::Theseus(
		theseus::Error::from(theseus::ErrorKind::OtherError(msg.into())),
	)
}

/// 将 API Key 掩码为 `前4字符****后2字符`（过短时直接返回 `****`），仅持久化提示信息。
fn mask_key(key: &str) -> String {
	let len = key.chars().count();
	if len <= 6 {
		"****".to_string()
	} else {
		let head: String = key.chars().take(4).collect();
		let tail: String = key.chars().skip(len - 2).collect();
		format!("{head}****{tail}")
	}
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ProviderConfig {
	pub api_key_hint: Option<String>,
	pub model: String,
	pub base_url: Option<String>,
	pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct KnowledgeConfig {
	pub allowed_domains: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct SkillsConfig {
	pub auto_load: bool,
	pub max_inject_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct McpConfig {
	pub enabled: bool,
	pub command: String,
	pub args: Vec<String>,
	pub health_check_interval_secs: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ChatHistoryConfig {
	pub max_conversations_per_instance: usize,
	pub retention_days: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct LayoutConfig {
	pub activitybar_position: String, // "left" | "right"
	pub sidebar_width: u32,
	pub bottom_panel_height: u32,
	pub split_ratio: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AiWorkshopConfig {
	pub enabled: bool,
	pub log_lines: usize,
	pub mock_enabled: bool,
	pub max_tool_iterations: usize,
	pub token_warning_threshold: u64,
	pub default_provider: Option<String>,
	pub providers: HashMap<String, ProviderConfig>,
	pub knowledge: KnowledgeConfig,
	pub skills: SkillsConfig,
	pub mcp: McpConfig,
	pub chat_history: ChatHistoryConfig,
	pub layout: LayoutConfig,
}

impl Default for AiWorkshopConfig {
	fn default() -> Self {
		let mut providers = HashMap::new();
		providers.insert(
			"openai".to_string(),
			ProviderConfig {
				api_key_hint: None,
				model: "gpt-4o".to_string(),
				base_url: None,
				enabled: false,
			},
		);
		providers.insert(
			"anthropic".to_string(),
			ProviderConfig {
				api_key_hint: None,
				model: "claude-3-5-sonnet".to_string(),
				base_url: None,
				enabled: false,
			},
		);
		providers.insert(
			"deepseek".to_string(),
			ProviderConfig {
				api_key_hint: None,
				model: "deepseek-chat".to_string(),
				base_url: None,
				enabled: false,
			},
		);
		providers.insert(
			"ollama".to_string(),
			ProviderConfig {
				api_key_hint: None,
				model: "llama3".to_string(),
				base_url: Some("http://localhost:11434".to_string()),
				enabled: false,
			},
		);
		providers.insert(
			"custom".to_string(),
			ProviderConfig {
				api_key_hint: None,
				model: String::new(),
				base_url: None,
				enabled: false,
			},
		);

		Self {
			enabled: true,
			log_lines: 500,
			mock_enabled: false,
			max_tool_iterations: 5,
			token_warning_threshold: 4000,
			default_provider: None,
			providers,
			knowledge: KnowledgeConfig {
				allowed_domains: vec![
					"modrinth.com".to_string(),
					"mcmod.cn".to_string(),
					"minecraft.fandom.com".to_string(),
					"ftbwiki.org".to_string(),
				],
			},
			skills: SkillsConfig {
				auto_load: false,
				max_inject_count: 3,
			},
			mcp: McpConfig {
				enabled: false,
				command: "npx".to_string(),
				args: vec!["-y".to_string(), "@modrinth/mcp".to_string()],
				health_check_interval_secs: 30,
			},
			chat_history: ChatHistoryConfig {
				max_conversations_per_instance: 100,
				retention_days: 90,
			},
			layout: LayoutConfig {
				activitybar_position: "left".to_string(),
				sidebar_width: 280,
				bottom_panel_height: 220,
				split_ratio: 0.6,
			},
		}
	}
}

pub struct ConfigManager {
	inner: RwLock<AiWorkshopConfig>,
	ai_root: PathBuf,
	config_path: PathBuf,
	secrets_path: PathBuf,
}

impl ConfigManager {
	/// 从 theseus 数据目录加载（或初始化）配置；解析失败时记录警告并回退默认值。
	pub async fn load<R: Runtime>(_app: &AppHandle<R>) -> Result<Arc<Self>> {
		let state = theseus::prelude::State::get().await?;
		let ai_root = state.directories.settings_dir.join("ai-workshop");
		std::fs::create_dir_all(&ai_root)?;

		let config_path = ai_root.join("config.json");
		let config = if config_path.exists() {
			match std::fs::read_to_string(&config_path)
				.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
				.and_then(|raw| {
					serde_json::from_str::<AiWorkshopConfig>(&raw)
						.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
				}) {
				Ok(config) => config,
				Err(e) => {
					tracing::warn!("ai_workshop: failed to parse config.json, using defaults: {e}");
					AiWorkshopConfig::default()
				}
			}
		} else {
			let default = AiWorkshopConfig::default();
			if let Err(e) = serde_json::to_string_pretty(&default)
				.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
				.and_then(|raw| std::fs::write(&config_path, raw))
			{
				tracing::warn!("ai_workshop: failed to write default config.json: {e}");
			}
			default
		};

		Ok(Arc::new(Self {
			inner: RwLock::new(config),
			ai_root: ai_root.clone(),
			config_path,
			secrets_path: ai_root.join("secrets.json"),
		}))
	}

	fn persist(&self, config: &AiWorkshopConfig) -> Result<()> {
		let raw = serde_json::to_string_pretty(config).map_err(|e| other_err(e.to_string()))?;
		std::fs::write(&self.config_path, raw)?;
		Ok(())
	}

	pub fn config(&self) -> AiWorkshopConfig {
		self.inner.read().unwrap().clone()
	}

	#[cfg(test)]
	pub(crate) fn for_tests(config: AiWorkshopConfig, ai_root: PathBuf) -> Arc<Self> {
		Arc::new(Self {
			inner: RwLock::new(config),
			ai_root: ai_root.clone(),
			config_path: ai_root.join("config.json"),
			secrets_path: ai_root.join("secrets.json"),
		})
	}

	pub async fn save_config(&self, config: AiWorkshopConfig) -> Result<()> {
		self.persist(&config)?;
		*self.inner.write().unwrap() = config;
		Ok(())
	}

	pub fn ai_root_dir(&self) -> PathBuf {
		self.ai_root.clone()
	}

	pub fn skills_dir(&self) -> PathBuf {
		self.ai_root.join("skills")
	}

	pub fn logs_dir(&self) -> PathBuf {
		self.ai_root.join("logs")
	}

	pub fn bm25_index_dir(&self) -> PathBuf {
		self.ai_root.join("bm25_index")
	}

	pub fn chat_db_path(&self) -> PathBuf {
		self.ai_root.join("chat_history").join("chat.db")
	}

	pub fn get_api_key_hint(&self, provider: &str) -> Option<String> {
		self.inner
			.read()
			.unwrap()
			.providers
			.get(provider)
			.and_then(|p| p.api_key_hint.clone())
	}

	/// 保存掩码后的 Key 提示并持久化配置（同步版本，供非 async 调用方使用）。
	pub fn set_api_key_hint(&self, provider: &str, key: &str) -> Result<()> {
		let mut config = self.config();
		let hint = mask_key(key);
		if let Some(provider_config) = config.providers.get_mut(provider) {
			provider_config.api_key_hint = Some(hint);
		} else {
			config.providers.insert(
				provider.to_string(),
				ProviderConfig {
					api_key_hint: Some(hint),
					model: String::new(),
					base_url: None,
					enabled: false,
				},
			);
		}
		self.persist(&config)?;
		*self.inner.write().unwrap() = config;
		Ok(())
	}

	/// 读取 secrets.json 中的明文 Key 映射（provider → key）。
	fn load_secrets(&self) -> HashMap<String, String> {
		std::fs::read_to_string(&self.secrets_path)
			.ok()
			.and_then(|raw| serde_json::from_str(&raw).ok())
			.unwrap_or_default()
	}

	/// 将明文 Key 映射写入 secrets.json（权限 0600），仅存于 ai-workshop 目录，
	/// 不进入 config.json（那里只保留掩码提示）。返回真实 key 的保存结果。
	fn write_secrets(&self, secrets: &HashMap<String, String>) -> Result<()> {
		let raw =
			serde_json::to_string_pretty(secrets).map_err(|e| other_err(e.to_string()))?;
		std::fs::write(&self.secrets_path, raw)?;
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			let _ = std::fs::set_permissions(&self.secrets_path, std::fs::Permissions::from_mode(0o600));
		}
		Ok(())
	}

	/// 将明文 Key 写入 secrets.json，并同步掩码提示到 config.json。
	pub fn set_api_key(&self, provider: &str, key: &str) -> Result<()> {
		let mut secrets = self.load_secrets();
		secrets.insert(provider.to_string(), key.to_string());
		self.write_secrets(&secrets)?;
		self.set_api_key_hint(provider, key)
	}

	/// 从 secrets.json 读取明文 Key；未配置时返回 None。
	pub fn get_decrypted_api_key(&self, provider: &str) -> Option<String> {
		self.load_secrets().get(provider).cloned()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn temp_ai_root() -> PathBuf {
		let nanos = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_nanos();
		let dir = std::env::temp_dir().join(format!("ai_config_test_{nanos}"));
		std::fs::create_dir_all(&dir).unwrap();
		dir
	}

	#[test]
	fn api_key_round_trip_uses_temp_secrets_file() {
		let dir = temp_ai_root();
		let manager = ConfigManager::for_tests(AiWorkshopConfig::default(), dir.clone());

		manager.set_api_key("openai", "sk-real-key-123456").unwrap();

		// 解密返回真实 key
		assert_eq!(
			manager.get_decrypted_api_key("openai").unwrap(),
			"sk-real-key-123456"
		);
		// 未配置的 provider 返回 None
		assert!(manager.get_decrypted_api_key("anthropic").is_none());

		// config.json 中只存掩码提示，绝不含真实 key
		let hint = manager.get_api_key_hint("openai").unwrap();
		assert_ne!(hint, "sk-real-key-123456");
		assert!(hint.contains("****"), "hint should be masked, got {hint}");

		// secrets 落盘到 ai_root/secrets.json，而非 config.json
		let config_raw = std::fs::read_to_string(manager.config_path.clone()).unwrap();
		assert!(
			!config_raw.contains("sk-real-key-123456"),
			"config.json must not contain plaintext key"
		);
		assert!(manager.secrets_path.exists());

		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[test]
	fn set_api_key_overwrites_and_persists() {
		let dir = temp_ai_root();
		let manager = ConfigManager::for_tests(AiWorkshopConfig::default(), dir.clone());

		manager.set_api_key("deepseek", "old-key").unwrap();
		manager.set_api_key("deepseek", "new-key").unwrap();

		// 新实例重新加载 secrets 后仍能读到最新 key（已持久化）
		let reloaded = ConfigManager::for_tests(AiWorkshopConfig::default(), dir.clone());
		assert_eq!(reloaded.get_decrypted_api_key("deepseek").unwrap(), "new-key");

		std::fs::remove_dir_all(&dir).unwrap();
	}
}
