use tauri::AppHandle;

use crate::error::AppError;
use crate::models::{AuthResponse, User};
use crate::services::auth as auth_service;

#[tauri::command]
pub async fn login(
    app: AppHandle,
    email: String,
    password: String,
) -> Result<AuthResponse, AppError> {
    auth_service::login(&app, email, password).await
}

#[tauri::command]
pub async fn register(
    app: AppHandle,
    email: String,
    password: String,
) -> Result<AuthResponse, AppError> {
    auth_service::register(&app, email, password).await
}

#[tauri::command]
pub async fn check_auth(app: AppHandle) -> Result<User, AppError> {
    auth_service::fetch_current_user(&app).await
}

#[tauri::command]
pub async fn fetch_plans(
    app: AppHandle,
) -> Result<std::collections::HashMap<String, crate::models::PlanLimits>, AppError> {
    auth_service::fetch_plans(&app).await
}

#[tauri::command]
pub async fn logout(app: AppHandle) -> Result<(), AppError> {
    auth_service::logout(&app).await
}
