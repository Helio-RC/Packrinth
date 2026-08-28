use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::ai_workshop::keystore::{KeyStore, KeyringKeyStore};
use crate::api::Result;

#[cfg(test)]
use crate::ai_workshop::keystore::InMemoryKeyStore;

fn other_err(msg: impl Into<String>) -> crate::api::TheseusSerializableError {
    crate::api::TheseusSerializableError::Theseus(theseus::Error::from(
        theseus::ErrorKind::OtherError(msg.into()),
    ))
}

fn default_auto_troubleshoot() -> bool {
    AUTO_TROUBLESHOOT_DEFAULT
}

fn default_log_flush_interval_secs() -> u64 {
    120
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

pub const AUTO_TROUBLESHOOT_DEFAULT: bool = true;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct AiWorkshopConfig {
    pub enabled: bool,
    pub log_lines: usize,
    #[serde(default = "default_log_flush_interval_secs")]
    pub log_flush_interval_secs: u64,
    pub mock_enabled: bool,
    #[serde(default = "default_auto_troubleshoot")]
    pub auto_troubleshoot: bool,
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
            log_flush_interval_secs: 120,
            mock_enabled: false,
            auto_troubleshoot: AUTO_TROUBLESHOOT_DEFAULT,
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
    key_store: Arc<dyn KeyStore>,
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
                    serde_json::from_str::<AiWorkshopConfig>(&raw).map_err(
                        |e| std::io::Error::new(std::io::ErrorKind::Other, e),
                    )
                }) {
                Ok(config) => config,
                Err(e) => {
                    tracing::warn!(
                        "ai_workshop: failed to parse config.json, using defaults: {e}"
                    );
                    AiWorkshopConfig::default()
                }
            }
        } else {
            let default = AiWorkshopConfig::default();
            if let Err(e) = serde_json::to_string_pretty(&default)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                .and_then(|raw| std::fs::write(&config_path, raw))
            {
                tracing::warn!(
                    "ai_workshop: failed to write default config.json: {e}"
                );
            }
            default
        };

        Ok(Arc::new(Self {
            inner: RwLock::new(config),
            ai_root: ai_root.clone(),
            config_path,
            key_store: Arc::new(KeyringKeyStore),
        }))
    }

    fn persist(&self, config: &AiWorkshopConfig) -> Result<()> {
        let raw = serde_json::to_string_pretty(config)
            .map_err(|e| other_err(e.to_string()))?;
        std::fs::write(&self.config_path, raw)?;
        Ok(())
    }

    pub fn config(&self) -> AiWorkshopConfig {
        self.inner.read().unwrap().clone()
    }

    #[cfg(test)]
    pub(crate) fn for_tests(
        config: AiWorkshopConfig,
        ai_root: PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(config),
            ai_root: ai_root.clone(),
            config_path: ai_root.join("config.json"),
            key_store: Arc::new(InMemoryKeyStore::default()),
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

    /// 将明文 Key 写入系统密钥环（keyring），并同步掩码提示到 config.json。
    /// 密钥环不可用时上抛错误；调用方应提示用户配置系统密钥环，绝不回退明文落盘。
    pub fn set_api_key(&self, provider: &str, key: &str) -> Result<()> {
        self.key_store
            .set(provider, key)
            .map_err(|e| other_err(format!("API Key 存储失败：{e}")))?;
        self.set_api_key_hint(provider, key)
    }

    /// 从系统密钥环读取明文 Key；未配置时返回 None；密钥环出错记录警告并返回 None。
    pub fn get_decrypted_api_key(&self, provider: &str) -> Option<String> {
        match self.key_store.get(provider) {
            Ok(value) => value,
            Err(e) => {
                tracing::warn!(
                    "ai_workshop: failed to read api key from keyring: {e}"
                );
                None
            }
        }
    }
}

/// IPC 边界 DTO（camelCase，与前端 `AiWorkshopConfig` 契约一致）。
/// config.json 文件格式保持 snake_case（见 goal.md §8.1），仅在 Tauri 命令出入口转换。
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AiWorkshopConfigDto {
    pub enabled: bool,
    pub log_lines: usize,
    pub log_flush_interval_secs: u64,
    pub mock_enabled: bool,
    pub auto_troubleshoot: bool,
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

impl From<&AiWorkshopConfig> for AiWorkshopConfigDto {
    fn from(config: &AiWorkshopConfig) -> Self {
        Self {
            enabled: config.enabled,
            log_lines: config.log_lines,
            log_flush_interval_secs: config.log_flush_interval_secs,
            mock_enabled: config.mock_enabled,
            auto_troubleshoot: config.auto_troubleshoot,
            max_tool_iterations: config.max_tool_iterations,
            token_warning_threshold: config.token_warning_threshold,
            default_provider: config.default_provider.clone(),
            providers: config.providers.clone(),
            knowledge: config.knowledge.clone(),
            skills: config.skills.clone(),
            mcp: config.mcp.clone(),
            chat_history: config.chat_history.clone(),
            layout: config.layout.clone(),
        }
    }
}

impl From<AiWorkshopConfigDto> for AiWorkshopConfig {
    fn from(dto: AiWorkshopConfigDto) -> Self {
        Self {
            enabled: dto.enabled,
            log_lines: dto.log_lines,
            log_flush_interval_secs: dto.log_flush_interval_secs,
            mock_enabled: dto.mock_enabled,
            auto_troubleshoot: dto.auto_troubleshoot,
            max_tool_iterations: dto.max_tool_iterations,
            token_warning_threshold: dto.token_warning_threshold,
            default_provider: dto.default_provider,
            providers: dto.providers,
            knowledge: dto.knowledge,
            skills: dto.skills,
            mcp: dto.mcp,
            chat_history: dto.chat_history,
            layout: dto.layout,
        }
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
    fn api_key_round_trip_via_in_memory_key_store() {
        let dir = temp_ai_root();
        let manager =
            ConfigManager::for_tests(AiWorkshopConfig::default(), dir.clone());

        manager.set_api_key("openai", "sk-real-key-123456").unwrap();

        // 解密返回真实 key
        assert_eq!(
            manager.get_decrypted_api_key("openai").unwrap(),
            "sk-real-key-123456"
        );
        // 未配置的 provider 返回 None
        assert!(manager.get_decrypted_api_key("anthropic").is_none());

        // config.json 中只存掩码提示，绝不含真实 key（密钥环不写盘）
        let hint = manager.get_api_key_hint("openai").unwrap();
        assert_ne!(hint, "sk-real-key-123456");
        assert!(hint.contains("****"), "hint should be masked, got {hint}");

        let config_raw =
            std::fs::read_to_string(manager.config_path.clone()).unwrap();
        assert!(
            !config_raw.contains("sk-real-key-123456"),
            "config.json must not contain plaintext key"
        );

        // 目录内不存在任何密钥落盘文件
        let leaked: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("secret")
            })
            .collect();
        assert!(leaked.is_empty(), "no secrets file may exist on disk");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn set_api_key_overwrites_in_key_store() {
        let dir = temp_ai_root();
        let manager =
            ConfigManager::for_tests(AiWorkshopConfig::default(), dir.clone());

        manager.set_api_key("deepseek", "old-key").unwrap();
        manager.set_api_key("deepseek", "new-key").unwrap();
        assert_eq!(
            manager.get_decrypted_api_key("deepseek").unwrap(),
            "new-key"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn dto_round_trip_preserves_config() {
        let dir = temp_ai_root();
        let manager =
            ConfigManager::for_tests(AiWorkshopConfig::default(), dir.clone());

        let original = manager.config();
        let dto = AiWorkshopConfigDto::from(&original);
        let wire = serde_json::to_value(&dto).unwrap();

        // IPC 契约：camelCase 键
        assert!(
            wire.get("logLines").is_some(),
            "dto must serialize camelCase keys"
        );
        assert!(wire.get("autoTroubleshoot").is_some());
        assert!(wire.get("log_lines").is_none());

        // 文件格式：snake_case
        let file_value = serde_json::to_value(&original).unwrap();
        assert!(
            file_value.get("log_lines").is_some(),
            "file format stays snake_case"
        );

        let parsed: AiWorkshopConfigDto = serde_json::from_value(wire).unwrap();
        let back: AiWorkshopConfig = parsed.into();
        assert_eq!(back.log_lines, original.log_lines);
        assert_eq!(back.auto_troubleshoot, original.auto_troubleshoot);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn auto_troubleshoot_defaults_true() {
        let config = AiWorkshopConfig::default();
        assert!(config.auto_troubleshoot);

        // 旧 config.json 无该字段时，反序列化应默认 true
        let mut value = serde_json::to_value(config).unwrap();
        value.as_object_mut().unwrap().remove("auto_troubleshoot");
        let legacy: AiWorkshopConfig = serde_json::from_value(value).unwrap();
        assert!(legacy.auto_troubleshoot);
    }
}
