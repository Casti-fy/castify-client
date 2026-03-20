use crate::error::AppError;
use crate::models::{CreateEpisodeRequest, CreateEpisodeResponse, UpdateEpisodeMetadataRequest, UpdateEpisodeRequest, UploadURLResponse};
use crate::state::AppState;

pub async fn create_episode(
    state: &AppState,
    feed_id: &str,
    body: &CreateEpisodeRequest,
) -> Result<CreateEpisodeResponse, AppError> {
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
    state: &AppState,
    episode_id: &str,
    status: &str,
    file_size: Option<u64>,
) -> Result<(), AppError> {
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

#[allow(dead_code)]
pub async fn update_metadata(
    state: &AppState,
    episode_id: &str,
    body: &UpdateEpisodeMetadataRequest,
) -> Result<(), AppError> {
    let api = state.api.read().await;
    api.request_no_content(
        &format!("/api/v1/episodes/{episode_id}"),
        "PATCH",
        Some(body),
        true,
    )
    .await
}

pub async fn get_upload_url(
    state: &AppState,
    episode_id: &str,
) -> Result<UploadURLResponse, AppError> {
    let api = state.api.read().await;
    api.request_with_body(
        &format!("/api/v1/episodes/{}/upload-url", episode_id),
        "POST",
        Some(&serde_json::json!({})),
        true,
    )
    .await
}
