use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::error::AppError;
use super::config_store::ConfigStore;

const CREDENTIALS_FILE: &str = "credentials.json";
const SETTINGS_FILE: &str = "settings.json";
const TOKEN_KEY: &str = "jwt_token";
const SYNC_INTERVAL_KEY: &str = "sync_interval_minutes";
const DEFAULT_SYNC_INTERVAL: u64 = 30;

/// ConfigStore backed by tauri-plugin-store. Used by the GUI app.
pub struct TauriConfigStore {
    app: AppHandle,
}

impl TauriConfigStore {
    pub fn new(app: &AppHandle) -> Self {
        Self { app: app.clone() }
    }
}

impl ConfigStore for TauriConfigStore {
    fn save_token(&self, token: &str) -> Result<(), AppError> {
        let store = self.app
            .store(CREDENTIALS_FILE)
            .map_err(|e| AppError::Keychain(e.to_string()))?;
        store.set(TOKEN_KEY, serde_json::json!(token));
        store.save().map_err(|e| AppError::Keychain(e.to_string()))
    }

    fn get_token(&self) -> Result<String, AppError> {
        let store = self.app
            .store(CREDENTIALS_FILE)
            .map_err(|e| AppError::Keychain(e.to_string()))?;
        match store.get(TOKEN_KEY) {
            Some(val) => val
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| AppError::Keychain("token is not a string".into())),
            None => Err(AppError::Keychain("no token found".into())),
        }
    }

    fn delete_token(&self) -> Result<(), AppError> {
        let store = self.app
            .store(CREDENTIALS_FILE)
            .map_err(|e| AppError::Keychain(e.to_string()))?;
        store.delete(TOKEN_KEY);
        store.save().map_err(|e| AppError::Keychain(e.to_string()))
    }

    fn read_sync_interval(&self) -> u64 {
        self.app
            .store(SETTINGS_FILE)
            .ok()
            .and_then(|store| store.get(SYNC_INTERVAL_KEY))
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_SYNC_INTERVAL)
    }

    fn write_sync_interval(&self, minutes: u64) {
        if let Ok(store) = self.app.store(SETTINGS_FILE) {
            store.set(SYNC_INTERVAL_KEY, serde_json::json!(minutes));
            let _ = store.save();
        }
    }
}
