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
