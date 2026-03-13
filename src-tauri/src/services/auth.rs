use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::models::{AuthResponse, LoginRequest, PlanLimits, RegisterRequest, User};
use crate::state::AppState;

use super::{keychain, sync};

pub async fn login(
    app: &AppHandle,
    email: String,
    password: String,
) -> Result<AuthResponse, AppError> {
    let state = app.state::<AppState>();
    let api = state.api.read().await;
    let body = LoginRequest { email, password };
    let resp = api
        .request_with_body::<AuthResponse, _>("/api/v1/auth/login", "POST", Some(&body), false)
        .await?;
    drop(api);

    apply_auth(app, &resp).await;
    Ok(resp)
}

pub async fn register(
    app: &AppHandle,
    email: String,
    password: String,
) -> Result<AuthResponse, AppError> {
    let state = app.state::<AppState>();
    let api = state.api.read().await;
    let body = RegisterRequest { email, password };
    let resp = api
        .request_with_body::<AuthResponse, _>("/api/v1/auth/register", "POST", Some(&body), false)
        .await?;
    drop(api);

    apply_auth(app, &resp).await;
    Ok(resp)
}

/// Save token, update cached limits, and auto-start sync after successful auth.
async fn apply_auth(app: &AppHandle, resp: &AuthResponse) {
    let state = app.state::<AppState>();
    state
        .api
        .write()
        .await
        .set_token(Some(resp.token.clone()));
    let _ = keychain::save_token(app, &resp.token);
    *state.cached_limits.write().await = Some(resp.user.limits.clone());

    let handle = app.clone();
    tokio::spawn(async move {
        sync::auto_start_sync(&handle).await;
    });
}

pub async fn fetch_current_user(app: &AppHandle) -> Result<User, AppError> {
    let state = app.state::<AppState>();
    let user = {
        let api = state.api.read().await;
        api.request::<User>("/api/v1/auth/me", "GET", true).await?
    };
    *state.cached_limits.write().await = Some(user.limits.clone());
    Ok(user)
}

pub async fn fetch_plans(app: &AppHandle) -> Result<HashMap<String, PlanLimits>, AppError> {
    let state = app.state::<AppState>();
    let api = state.api.read().await;
    api.request::<HashMap<String, PlanLimits>>("/api/v1/plans", "GET", false)
        .await
}

pub async fn logout(app: &AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    state.api.write().await.set_token(None);
    *state.cached_limits.write().await = None;
    let _ = keychain::delete_token(app);
    sync::stop_periodic_sync(app.clone()).await?;
    Ok(())
}
