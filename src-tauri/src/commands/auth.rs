use tauri::State;

use crate::error::AppError;
use crate::models::*;
use crate::state::AppState;

#[tauri::command]
pub async fn login(
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
    Ok(resp)
}

#[tauri::command]
pub async fn register(
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
pub async fn logout(state: State<'_, AppState>) -> Result<(), AppError> {
    state.api.write().await.set_token(None);
    // Stop periodic sync
    let mut handle = state.sync_handle.lock().await;
    if let Some(h) = handle.take() {
        h.abort();
    }
    Ok(())
}
