use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::error::AppError;

const STORE_FILE: &str = "credentials.json";
const TOKEN_KEY: &str = "jwt_token";

pub fn save_token(app: &AppHandle, token: &str) -> Result<(), AppError> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| AppError::Keychain(e.to_string()))?;
    store.set(TOKEN_KEY, serde_json::json!(token));
    store
        .save()
        .map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn get_token(app: &AppHandle) -> Result<String, AppError> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| AppError::Keychain(e.to_string()))?;
    match store.get(TOKEN_KEY) {
        Some(val) => val
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Keychain("token is not a string".into())),
        None => Err(AppError::Keychain("no token found".into())),
    }
}

pub fn delete_token(app: &AppHandle) -> Result<(), AppError> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| AppError::Keychain(e.to_string()))?;
    store.delete(TOKEN_KEY);
    store
        .save()
        .map_err(|e| AppError::Keychain(e.to_string()))
}
