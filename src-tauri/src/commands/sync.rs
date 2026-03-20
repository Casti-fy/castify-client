use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::services::sync as sync_service;
use crate::state::AppState;

#[tauri::command]
pub async fn sync_feed(app: AppHandle, feed_id: String) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    sync_service::sync_single_feed(&state, &feed_id).await
}

#[tauri::command]
pub async fn get_sync_interval(app: AppHandle) -> Result<u64, AppError> {
    let state = app.state::<AppState>();
    Ok(sync_service::read_sync_interval(&state))
}

#[tauri::command]
pub async fn set_sync_interval(app: AppHandle, minutes: u64) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    sync_service::write_sync_interval(&state, minutes);
    Ok(())
}

#[tauri::command]
pub async fn clear_sync_cache() -> Result<(), AppError> {
    let tmp_dir = std::env::temp_dir();

    if let Ok(entries) = fs::read_dir(&tmp_dir) {
        for entry in entries.flatten() {
            let path: PathBuf = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("castify-") {
                    let _ = fs::remove_dir_all(&path);
                }
            }
        }
    }

    Ok(())
}
