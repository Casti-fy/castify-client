use crate::error::AppError;
use crate::models::{CreateFeedRequest, CreateFeedResponse, Feed, FeedDetailResponse, UpdateFeedRequest};
use crate::state::AppState;

use super::sync;

pub async fn fetch_all_feeds(state: &AppState) -> Result<Vec<Feed>, AppError> {
    let api = state.api.read().await;
    api.request::<Vec<Feed>>("/api/v1/feeds", "GET", true).await
}

pub async fn create_feed(
    state: &AppState,
    name: String,
    source_url: String,
    description: Option<String>,
) -> Result<CreateFeedResponse, AppError> {
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
    let state = state.clone();
    tokio::spawn(async move {
        sync::scan_new_feed(&state, &feed).await;
    });

    Ok(resp)
}

pub async fn fetch_feed_detail(
    state: &AppState,
    feed_id: &str,
) -> Result<FeedDetailResponse, AppError> {
    let api = state.api.read().await;
    api.request::<FeedDetailResponse>(&format!("/api/v1/feeds/{feed_id}"), "GET", true)
        .await
}

pub async fn update_feed_artwork(
    state: &AppState,
    feed_id: &str,
    artwork_url: &str,
) -> Result<(), AppError> {
    let body = UpdateFeedRequest {
        artwork_url: Some(artwork_url.to_string()),
    };
    let api = state.api.read().await;
    api.request_no_content(
        &format!("/api/v1/feeds/{feed_id}"),
        "PATCH",
        Some(&body),
        true,
    )
    .await
}

pub async fn delete_feed(state: &AppState, feed_id: &str) -> Result<(), AppError> {
    let api = state.api.read().await;
    api.request_no_content::<()>(&format!("/api/v1/feeds/{feed_id}"), "DELETE", None, true)
        .await?;

    let mut cancelled = state.cancelled_feeds.write().await;
    cancelled.insert(feed_id.to_string());

    Ok(())
}
