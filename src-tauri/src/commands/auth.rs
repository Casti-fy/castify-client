use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::models::{AuthResponse, User};
use crate::services::auth as auth_service;
use crate::state::AppState;

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
    let state = app.state::<AppState>();
    auth_service::fetch_current_user(&state).await
}

#[tauri::command]
pub async fn fetch_plans(
    app: AppHandle,
) -> Result<std::collections::HashMap<String, crate::models::PlanLimits>, AppError> {
    let state = app.state::<AppState>();
    auth_service::fetch_plans(&state).await
}

#[tauri::command]
pub async fn logout(app: AppHandle) -> Result<(), AppError> {
    auth_service::logout(&app).await
}
