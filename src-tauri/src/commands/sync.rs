use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::services::{helpers, sync as sync_service};
use crate::state::AppState;

#[tauri::command]
pub async fn sync_feed(app: AppHandle, feed_id: String) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    sync_service::sync_single_feed(&state, &feed_id).await
}

#[tauri::command]
pub async fn backfill_feed(app: AppHandle, feed_id: String, start: u32, end: u32) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    sync_service::backfill_feed(&state, &feed_id, start, end).await
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
    helpers::clear_all_temp_dirs();
    Ok(())
}
