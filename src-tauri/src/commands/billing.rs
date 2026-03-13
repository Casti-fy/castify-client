use tauri::AppHandle;

use crate::error::AppError;
use crate::services::billing as billing_service;

#[tauri::command]
pub async fn create_checkout(
    app: AppHandle,
    plan: String,
    interval: String,
) -> Result<String, AppError> {
    billing_service::create_checkout(&app, plan, interval).await
}

#[tauri::command]
pub async fn create_portal(app: AppHandle) -> Result<String, AppError> {
    billing_service::create_portal(&app).await
}
