use serde::Deserialize;
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct CheckoutResponse {
    checkout_url: String,
}

#[derive(Debug, Deserialize)]
struct PortalResponse {
    portal_url: String,
}

#[tauri::command]
pub async fn create_checkout(
    state: State<'_, AppState>,
    plan: String,
) -> Result<String, AppError> {
    let body = serde_json::json!({ "plan": plan });
    let resp: CheckoutResponse = state
        .api
        .read()
        .await
        .request_with_body("/api/v1/billing/checkout", "POST", Some(&body), true)
        .await?;
    Ok(resp.checkout_url)
}

#[tauri::command]
pub async fn create_portal(state: State<'_, AppState>) -> Result<String, AppError> {
    let resp: PortalResponse = state
        .api
        .read()
        .await
        .request("/api/v1/billing/portal", "POST", true)
        .await?;
    Ok(resp.portal_url)
}
