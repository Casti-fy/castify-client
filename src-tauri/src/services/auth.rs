use std::collections::HashMap;

use crate::error::AppError;
use crate::models::{AuthResponse, LoginRequest, PlanLimits, RegisterRequest, User};
use crate::state::AppState;

use super::sync;

pub async fn login(
    state: &AppState,
    email: String,
    password: String,
) -> Result<AuthResponse, AppError> {
    let api = state.api.read().await;
    let body = LoginRequest { email, password };
    let resp = api
        .request_with_body::<AuthResponse, _>("/api/v1/auth/login", "POST", Some(&body), false)
        .await?;
    drop(api);

    apply_auth(state, &resp).await;
    Ok(resp)
}

pub async fn register(
    state: &AppState,
    email: String,
    password: String,
) -> Result<AuthResponse, AppError> {
    let api = state.api.read().await;
    let body = RegisterRequest { email, password };
    let resp = api
        .request_with_body::<AuthResponse, _>("/api/v1/auth/register", "POST", Some(&body), false)
        .await?;
    drop(api);

    apply_auth(state, &resp).await;
    Ok(resp)
}

/// Save token, update cached limits, and auto-start sync after successful auth.
async fn apply_auth(state: &AppState, resp: &AuthResponse) {
    state
        .api
        .write()
        .await
        .set_token(Some(resp.token.clone()));
    let _ = state.store.save_token(&resp.token);
    *state.cached_limits.write().await = Some(resp.user.limits.clone());

    let state = state.clone();
    tokio::spawn(async move {
        sync::auto_start_sync(&state).await;
    });
}

pub async fn fetch_current_user(state: &AppState) -> Result<User, AppError> {
    let user = {
        let api = state.api.read().await;
        api.request::<User>("/api/v1/auth/me", "GET", true).await?
    };
    *state.cached_limits.write().await = Some(user.limits.clone());
    Ok(user)
}

pub async fn fetch_plans(state: &AppState) -> Result<HashMap<String, PlanLimits>, AppError> {
    let api = state.api.read().await;
    api.request::<HashMap<String, PlanLimits>>("/api/v1/plans", "GET", false)
        .await
}

pub async fn logout(state: &AppState) -> Result<(), AppError> {
    state.api.write().await.set_token(None);
    *state.cached_limits.write().await = None;
    let _ = state.store.delete_token();
    sync::stop_periodic_sync(state).await?;
    Ok(())
}
