use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

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
	pub async fn load(app: &AppHandle) -> Result<Arc<Self>> {
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
			ai_root,
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

	/// 将明文 Key 写入 tauri-plugin-store 安全存储，并同步掩码提示到 config.json。
	pub fn set_api_key(&self, app: &AppHandle, provider: &str, key: &str) -> Result<()> {
		let store = app
			.store("ai-workshop-secrets.json")
			.map_err(|e| other_err(e.to_string()))?;
		store.set(provider, serde_json::Value::String(key.to_string()));
		store.save().map_err(|e| other_err(e.to_string()))?;
		self.set_api_key_hint(provider, key)
	}

	/// 从 tauri-plugin-store 读取明文 Key；store 不存在或未配置时返回 None。
	pub fn get_decrypted_api_key(&self, app: &AppHandle, provider: &str) -> Option<String> {
		let store = app.store("ai-workshop-secrets.json").ok()?;
		store
			.get(provider)
			.and_then(|value| value.as_str().map(|s| s.to_string()))
	}
}
