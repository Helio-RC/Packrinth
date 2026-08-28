// === AI-WORKSHOP START ===
// API Key 安全存储抽象：系统密钥环实现 + 测试内存实现。
// 生产环境走 keyring（macOS Keychain / Windows 凭据管理器 / Linux Secret Service），
// 无密钥环时上抛错误给调用方，绝不回退明文落盘。
use std::collections::HashMap;
use std::sync::Mutex;

/// Key 存储服务名（keyring 的 service 维度，key 维度为 provider 名）。
const KEYRING_SERVICE: &str = "packrinth-ai-workshop";

/// 密钥存储抽象，便于单测与无密钥环环境（CI）替换实现。
pub trait KeyStore: Send + Sync {
    /// 读取明文 Key；不存在返回 Ok(None)，密钥环出错返回 Err。
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    /// 写入明文 Key。
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    /// 删除明文 Key；不存在视为 Ok。
    fn delete(&self, key: &str) -> Result<(), String>;
}

/// 生产实现：系统密钥环。服务名固定，key = provider 名。
pub struct KeyringKeyStore;

impl KeyStore for KeyringKeyStore {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, key)
            .map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("读取密钥环 {key} 失败: {e}")),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, key)
            .map_err(|e| e.to_string())?;
        entry
            .set_password(value)
            .map_err(|e| format!("写入密钥环 {key} 失败: {e}"))
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, key)
            .map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("删除密钥环 {key} 失败: {e}")),
        }
    }
}

/// 测试实现：进程内存 HashMap。CI 无密钥环时替代生产实现。
#[derive(Default)]
pub struct InMemoryKeyStore {
    inner: Mutex<HashMap<String, String>>,
}

impl InMemoryKeyStore {
    fn key(service: &str, key: &str) -> String {
        format!("{service}/{key}")
    }
}

impl KeyStore for InMemoryKeyStore {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .get(&Self::key(KEYRING_SERVICE, key))
            .cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.inner
            .lock()
            .unwrap()
            .insert(Self::key(KEYRING_SERVICE, key), value.to_string());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        self.inner
            .lock()
            .unwrap()
            .remove(&Self::key(KEYRING_SERVICE, key));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_round_trip() {
        let store = InMemoryKeyStore::default();
        assert_eq!(store.get("openai").unwrap(), None);
        store.set("openai", "sk-1").unwrap();
        assert_eq!(store.get("openai").unwrap().as_deref(), Some("sk-1"));
        store.delete("openai").unwrap();
        assert_eq!(store.get("openai").unwrap(), None);
    }

    #[test]
    fn provider_names_are_isolated() {
        let store = InMemoryKeyStore::default();
        store.set("openai", "a").unwrap();
        store.set("anthropic", "b").unwrap();
        assert_eq!(store.get("openai").unwrap().as_deref(), Some("a"));
        assert_eq!(store.get("anthropic").unwrap().as_deref(), Some("b"));
    }
}
// === AI-WORKSHOP END ===
