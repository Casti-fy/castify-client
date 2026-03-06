use tauri::{AppHandle, State};

use crate::error::AppError;
use crate::models::*;
use crate::services::keychain;
use crate::state::AppState;

#[tauri::command]
pub async fn login(
    app: AppHandle,
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<AuthResponse, AppError> {
    let body = LoginRequest { email, password };
    let resp: AuthResponse = state
        .api
        .read()
        .await
        .request_with_body("/api/v1/auth/login", "POST", Some(&body), false)
        .await?;

    state.api.write().await.set_token(Some(resp.token.clone()));
    let _ = keychain::save_token(&app, &resp.token);
    Ok(resp)
}

#[tauri::command]
pub async fn register(
    app: AppHandle,
    state: State<'_, AppState>,
    email: String,
    password: String,
) -> Result<AuthResponse, AppError> {
    let body = RegisterRequest { email, password };
    let resp: AuthResponse = state
        .api
        .read()
        .await
        .request_with_body("/api/v1/auth/register", "POST", Some(&body), false)
        .await?;

    state.api.write().await.set_token(Some(resp.token.clone()));
    let _ = keychain::save_token(&app, &resp.token);
    Ok(resp)
}

#[tauri::command]
pub async fn check_auth(state: State<'_, AppState>) -> Result<User, AppError> {
    state
        .api
        .read()
        .await
        .request::<User>("/api/v1/auth/me", "GET", true)
        .await
}

#[tauri::command]
pub async fn logout(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    state.api.write().await.set_token(None);
    let _ = keychain::delete_token(&app);
    // Stop periodic sync
    let mut handles = state.sync_handles.lock().await;
    if let Some(h) = handles.scan.take() {
        h.abort();
    }
    if let Some(h) = handles.download.take() {
        h.abort();
    }
    if let Some(h) = handles.upload.take() {
        h.abort();
    }
    Ok(())
}
