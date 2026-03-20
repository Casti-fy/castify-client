use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::models::{CreateFeedResponse, Feed, FeedDetailResponse};
use crate::services::feeds as feeds_service;
use crate::state::AppState;

#[tauri::command]
pub async fn list_feeds(app: AppHandle) -> Result<Vec<Feed>, AppError> {
    let state = app.state::<AppState>();
    feeds_service::fetch_all_feeds(&state).await
}

#[tauri::command]
pub async fn create_feed(
    app: AppHandle,
    name: String,
    source_url: String,
    description: Option<String>,
) -> Result<CreateFeedResponse, AppError> {
    feeds_service::create_feed(&app, name, source_url, description).await
}

#[tauri::command]
pub async fn get_feed_detail(
    app: AppHandle,
    feed_id: String,
) -> Result<FeedDetailResponse, AppError> {
    let state = app.state::<AppState>();
    feeds_service::fetch_feed_detail(&state, &feed_id).await
}

#[tauri::command]
pub async fn delete_feed(app: AppHandle, feed_id: String) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    feeds_service::delete_feed(&state, &feed_id).await
}
