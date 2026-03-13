use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::models::{CreateFeedRequest, CreateFeedResponse, Feed, FeedDetailResponse};
use crate::state::AppState;

use super::sync;

pub async fn fetch_all_feeds(app: &AppHandle) -> Result<Vec<Feed>, AppError> {
    let state = app.state::<AppState>();
    let api = state.api.read().await;
    api.request::<Vec<Feed>>("/api/v1/feeds", "GET", true).await
}

pub async fn create_feed(
    app: &AppHandle,
    name: String,
    source_url: String,
    description: Option<String>,
) -> Result<CreateFeedResponse, AppError> {
    let state = app.state::<AppState>();
    let body = CreateFeedRequest {
        name,
        source_url,
        description,
    };
    let api = state.api.read().await;
    let resp = api
        .request_with_body::<CreateFeedResponse, _>("/api/v1/feeds", "POST", Some(&body), true)
        .await?;
    drop(api);

    // Scan first episodes in background
    let feed = resp.feed.clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        sync::scan_new_feed(&app_clone, &feed).await;
    });

    Ok(resp)
}

pub async fn fetch_feed_detail(
    app: &AppHandle,
    feed_id: &str,
) -> Result<FeedDetailResponse, AppError> {
    let state = app.state::<AppState>();
    let api = state.api.read().await;
    api.request::<FeedDetailResponse>(&format!("/api/v1/feeds/{feed_id}"), "GET", true)
        .await
}

pub async fn delete_feed(app: &AppHandle, feed_id: &str) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let api = state.api.read().await;
    api.request_no_content::<()>(&format!("/api/v1/feeds/{feed_id}"), "DELETE", None, true)
        .await
}
