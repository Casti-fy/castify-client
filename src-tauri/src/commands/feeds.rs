use tauri::State;

use crate::error::AppError;
use crate::models::*;
use crate::state::AppState;

#[tauri::command]
pub async fn list_feeds(state: State<'_, AppState>) -> Result<Vec<Feed>, AppError> {
    state
        .api
        .read()
        .await
        .request::<Vec<Feed>>("/api/v1/feeds", "GET", true)
        .await
}

#[tauri::command]
pub async fn create_feed(
    state: State<'_, AppState>,
    name: String,
    source_url: String,
    description: Option<String>,
) -> Result<CreateFeedResponse, AppError> {
    let body = CreateFeedRequest {
        name,
        source_url,
        description,
    };
    state
        .api
        .read()
        .await
        .request_with_body("/api/v1/feeds", "POST", Some(&body), true)
        .await
}

#[tauri::command]
pub async fn get_feed_detail(
    state: State<'_, AppState>,
    feed_id: String,
) -> Result<FeedDetailResponse, AppError> {
    state
        .api
        .read()
        .await
        .request::<FeedDetailResponse>(&format!("/api/v1/feeds/{feed_id}"), "GET", true)
        .await
}

#[tauri::command]
pub async fn delete_feed(state: State<'_, AppState>, feed_id: String) -> Result<(), AppError> {
    state
        .api
        .read()
        .await
        .request_no_content::<()>(&format!("/api/v1/feeds/{feed_id}"), "DELETE", None, true)
        .await
}
