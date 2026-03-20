use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::services::billing as billing_service;
use crate::state::AppState;

#[tauri::command]
pub async fn create_checkout(
    app: AppHandle,
    plan: String,
    interval: String,
) -> Result<String, AppError> {
    let state = app.state::<AppState>();
    billing_service::create_checkout(&state, plan, interval).await
}

#[tauri::command]
pub async fn create_portal(app: AppHandle) -> Result<String, AppError> {
    let state = app.state::<AppState>();
    billing_service::create_portal(&state).await
}
