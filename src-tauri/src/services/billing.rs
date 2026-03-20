use serde::Deserialize;

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

pub async fn create_checkout(
    state: &AppState,
    plan: String,
    interval: String,
) -> Result<String, AppError> {
    let body = serde_json::json!({ "plan": plan, "interval": interval });
    let resp: CheckoutResponse = state
        .api
        .read()
        .await
        .request_with_body("/api/v1/billing/checkout", "POST", Some(&body), true)
        .await?;
    Ok(resp.checkout_url)
}

pub async fn create_portal(state: &AppState) -> Result<String, AppError> {
    let resp: PortalResponse = state
        .api
        .read()
        .await
        .request("/api/v1/billing/portal", "POST", true)
        .await?;
    Ok(resp.portal_url)
}
