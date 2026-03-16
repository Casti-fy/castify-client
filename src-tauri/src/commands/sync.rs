use std::fs;
use std::path::PathBuf;

use tauri::AppHandle;

use crate::error::AppError;
use crate::services::sync as sync_service;

#[tauri::command]
pub async fn sync_feed(app: AppHandle, feed_id: String) -> Result<(), AppError> {
    sync_service::sync_single_feed(&app, &feed_id).await
}

#[tauri::command]
pub async fn get_sync_interval(app: AppHandle) -> Result<u64, AppError> {
    Ok(sync_service::read_sync_interval(&app))
}

#[tauri::command]
pub async fn set_sync_interval(app: AppHandle, minutes: u64) -> Result<(), AppError> {
    sync_service::write_sync_interval(&app, minutes);
    Ok(())
}

#[tauri::command]
pub async fn clear_sync_cache() -> Result<(), AppError> {
    let tmp_dir = std::env::temp_dir();

    // Remove all temp dirs of the form castify-<feed_id>
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
