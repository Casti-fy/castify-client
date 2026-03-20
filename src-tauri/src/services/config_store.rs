use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::error::AppError;

const DEFAULT_SYNC_INTERVAL: u64 = 30;

/// Platform-agnostic key-value storage for credentials and settings.
/// GUI implements this with tauri-plugin-store; CLI uses the built-in
/// file-based implementation.
pub trait ConfigStore: Send + Sync {
    fn save_token(&self, token: &str) -> Result<(), AppError>;
    fn get_token(&self) -> Result<String, AppError>;
    fn delete_token(&self) -> Result<(), AppError>;
    fn read_sync_interval(&self) -> u64;
    fn write_sync_interval(&self, minutes: u64);
}

/// File-based ConfigStore backed by JSON files on disk.
/// Works without Tauri — suitable for CLI and testing.
pub struct FileConfigStore {
    credentials_path: PathBuf,
    settings_path: PathBuf,
    cache: RwLock<HashMap<String, serde_json::Value>>,
}

impl FileConfigStore {
    pub fn new(data_dir: &Path) -> Self {
        let _ = std::fs::create_dir_all(data_dir);
        let store = Self {
            credentials_path: data_dir.join("credentials.json"),
            settings_path: data_dir.join("settings.json"),
            cache: RwLock::new(HashMap::new()),
        };
        // Pre-load both files into cache
        store.load_file(&store.credentials_path);
        store.load_file(&store.settings_path);
        store
    }

    fn load_file(&self, path: &Path) {
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&data) {
                if let Ok(mut cache) = self.cache.write() {
                    cache.extend(map);
                }
            }
        }
    }

    fn save_file(&self, path: &Path, keys: &[&str]) {
        let cache = self.cache.read().unwrap();
        let mut map = HashMap::new();
        for &key in keys {
            if let Some(val) = cache.get(key) {
                map.insert(key.to_string(), val.clone());
            }
        }
        if let Ok(json) = serde_json::to_string_pretty(&map) {
            let _ = std::fs::write(path, json);
        }
    }

    fn get(&self, key: &str) -> Option<serde_json::Value> {
        self.cache.read().ok()?.get(key).cloned()
    }

    fn set(&self, key: &str, value: serde_json::Value) {
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(key.to_string(), value);
        }
    }

    fn delete(&self, key: &str) {
        if let Ok(mut cache) = self.cache.write() {
            cache.remove(key);
        }
    }
}

const TOKEN_KEY: &str = "jwt_token";
const SYNC_INTERVAL_KEY: &str = "sync_interval_minutes";

impl ConfigStore for FileConfigStore {
    fn save_token(&self, token: &str) -> Result<(), AppError> {
        self.set(TOKEN_KEY, serde_json::json!(token));
        self.save_file(&self.credentials_path, &[TOKEN_KEY]);
        Ok(())
    }

    fn get_token(&self) -> Result<String, AppError> {
        self.get(TOKEN_KEY)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| AppError::Keychain("no token found".into()))
    }

    fn delete_token(&self) -> Result<(), AppError> {
        self.delete(TOKEN_KEY);
        self.save_file(&self.credentials_path, &[TOKEN_KEY]);
        Ok(())
    }

    fn read_sync_interval(&self) -> u64 {
        self.get(SYNC_INTERVAL_KEY)
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_SYNC_INTERVAL)
    }

    fn write_sync_interval(&self, minutes: u64) {
        self.set(SYNC_INTERVAL_KEY, serde_json::json!(minutes));
        self.save_file(&self.settings_path, &[SYNC_INTERVAL_KEY]);
    }
}
