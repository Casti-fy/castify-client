use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::models::{CreateEpisodeRequest, CreateEpisodeResponse, UpdateEpisodeRequest, UploadURLResponse};
use crate::state::AppState;

pub async fn create_episode(
    app: &AppHandle,
    feed_id: &str,
    body: &CreateEpisodeRequest,
) -> Result<CreateEpisodeResponse, AppError> {
    let state = app.state::<AppState>();
    let api = state.api.read().await;
    api.request_with_body::<CreateEpisodeResponse, _>(
        &format!("/api/v1/feeds/{}/episodes", feed_id),
        "POST",
        Some(body),
        true,
    )
    .await
}

pub async fn update_status(
    app: &AppHandle,
    episode_id: &str,
    status: &str,
    file_size: Option<u64>,
) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let body = UpdateEpisodeRequest {
        status: status.to_string(),
        file_size,
    };
    let api = state.api.read().await;
    api.request_no_content(
        &format!("/api/v1/episodes/{episode_id}"),
        "PATCH",
        Some(&body),
        true,
    )
    .await
}

pub async fn get_upload_url(
    app: &AppHandle,
    episode_id: &str,
) -> Result<UploadURLResponse, AppError> {
    let state = app.state::<AppState>();
    let api = state.api.read().await;
    api.request_with_body(
        &format!("/api/v1/episodes/{}/upload-url", episode_id),
        "POST",
        Some(&serde_json::json!({})),
        true,
    )
    .await
}

