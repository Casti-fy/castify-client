use tauri::AppHandle;

use crate::error::AppError;
use crate::models::{CreateFeedResponse, Feed, FeedDetailResponse};
use crate::services::feeds as feeds_service;

#[tauri::command]
pub async fn list_feeds(app: AppHandle) -> Result<Vec<Feed>, AppError> {
    feeds_service::fetch_all_feeds(&app).await
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
    feeds_service::fetch_feed_detail(&app, &feed_id).await
}

#[tauri::command]
pub async fn delete_feed(app: AppHandle, feed_id: String) -> Result<(), AppError> {
    feeds_service::delete_feed(&app, &feed_id).await
}
