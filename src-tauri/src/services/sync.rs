use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::models::Feed;
use crate::services::{extractor, feeds as feeds_service};
use crate::state::{AppState, Job, Priority};

use super::{sync_download, sync_scan, sync_upload};


/// Push a feed's not-ready episodes to channels with the given priority.
pub async fn push_feed_episodes(app: &AppHandle, feed_id: &str, priority: Priority) {
    let state = app.state::<AppState>();
    let detail = match feeds_service::fetch_feed_detail(app, feed_id).await {
        Ok(d) => d,
        Err(e) => {
            log::warn!("Failed to fetch feed detail for push: {e}");
            return;
        }
    };
    for ep in &detail.episodes {
        match ep.status.as_str() {
            "pending" | "failed" => {
                let episode_url =
                    extractor::episode_url(&detail.feed.source_url, &ep.video_id);
                state
                    .sync_channels
                    .send_download(Job {
                        feed_id: feed_id.to_string(),
                        feed_name: detail.feed.name.clone(),
                        episode_id: ep.id.clone(),
                        episode_title: ep.title.clone(),
                        video_id: ep.video_id.clone(),
                        episode_url,
                        priority,
                    })
                    .await;
            }
            "uploading" => {
                let episode_url =
                    extractor::episode_url(&detail.feed.source_url, &ep.video_id);
                state
                    .sync_channels
                    .send_upload(Job {
                        feed_id: feed_id.to_string(),
                        feed_name: detail.feed.name.clone(),
                        episode_id: ep.id.clone(),
                        episode_title: ep.title.clone(),
                        video_id: ep.video_id.clone(),
                        episode_url,
                        priority,
                    })
                    .await;
            }
            _ => {}
        }
    }
}

/// Scan the first N episodes of a feed and push not-ready ones as Urgent.
/// Also fetches the channel artwork and updates the feed if found.
pub async fn scan_new_feed(app: &AppHandle, feed: &Feed) {
    // Fetch channel artwork in parallel with episode scan
    let app_artwork = app.clone();
    let feed_id = feed.id.clone();
    let source_url = feed.source_url.clone();
    let artwork_handle = tokio::spawn(async move {
        match extractor::fetch_channel_artwork_url(&app_artwork, &source_url).await {
            Ok(Some(url)) => {
                if let Err(e) = feeds_service::update_feed_artwork(&app_artwork, &feed_id, &url).await {
                    log::warn!("[scan_new_feed] failed to update artwork: {e}");
                }
            }
            Ok(None) => log::info!("[scan_new_feed] no artwork found for feed {feed_id}"),
            Err(e) => log::warn!("[scan_new_feed] failed to fetch artwork: {e}"),
        }
    });

    let feeds = [feed.clone()];
    let _ = run_sync_for_feeds(app, &feeds, 5, Priority::Urgent).await;
    let _ = artwork_handle.await;
}

/// Sync a single feed: scan for new episodes, then push any not-ready ones as Urgent.
pub async fn sync_single_feed(app: &AppHandle, feed_id: &str) -> Result<(), AppError> {
    let detail = feeds_service::fetch_feed_detail(app, feed_id).await?;
    let feed = detail.feed.clone();
    push_feed_episodes(app, feed_id, Priority::Urgent).await;
    run_sync_for_feeds(app, &[feed], 10, Priority::Urgent).await?;
    Ok(())
}

pub async fn run_sync_for_feeds(
    app: &AppHandle,
    feeds: &[Feed],
    max_scan_items: u32,
    priority: Priority,
) -> Result<(), AppError> {
    sync_scan::run_scan(app, feeds, max_scan_items, priority).await;
    Ok(())
}

// ----- Settings helpers -----

const SETTINGS_STORE: &str = "settings.json";
const SYNC_INTERVAL_KEY: &str = "sync_interval_minutes";
const DEFAULT_SYNC_INTERVAL: u64 = 30;

pub fn read_sync_interval(app: &AppHandle) -> u64 {
    use tauri_plugin_store::StoreExt;

    app.store(SETTINGS_STORE)
        .ok()
        .and_then(|store| store.get(SYNC_INTERVAL_KEY))
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_SYNC_INTERVAL)
}

pub fn write_sync_interval(app: &AppHandle, minutes: u64) {
    use tauri_plugin_store::StoreExt;

    if let Ok(store) = app.store(SETTINGS_STORE) {
        store.set(SYNC_INTERVAL_KEY, serde_json::json!(minutes));
        let _ = store.save();
    }
}

// ----- Periodic sync orchestration -----

pub async fn start_periodic_sync(app: &AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let mut handles = state.sync_handles.lock().await;

    log::info!("Starting periodic sync");

    let app_scan = app.clone();
    handles.scan = Some(tokio::spawn(async move {
        let mut last_scan: Option<tokio::time::Instant> = None;
        loop {
            let interval_minutes = read_sync_interval(&app_scan);
            let interval = Duration::from_secs(interval_minutes * 60);
            if let Some(last) = last_scan {
                if last.elapsed() < interval {
                    continue;
                }
            }

            let feeds: Vec<Feed> = match feeds_service::fetch_all_feeds(&app_scan).await {
                Ok(f) => f,
                Err(e) => {
                    log::warn!("Failed to fetch feeds: {e}");
                    continue;
                }
            };
            let _ = run_sync_for_feeds(&app_scan, &feeds, 5, Priority::High).await;
            last_scan = Some(tokio::time::Instant::now());
        }
    }));

    // Ensure we have fresh channel receivers in case sync was previously started/stopped.
    if state.sync_channels.download_rx.lock().await.is_none()
        || state.sync_channels.upload_rx.lock().await.is_none()
    {
        state.sync_channels.reset().await;
    }

    let dl_rx = state.sync_channels.download_rx.lock().await.take();
    let ul_rx = state.sync_channels.upload_rx.lock().await.take();

    if let Some(rx) = dl_rx {
        let app_dl = app.clone();
        handles.download = Some(tokio::spawn(async move {
            sync_download::start_download_worker(app_dl, rx).await;
        }));
    } else {
        log::warn!("Download receivers unavailable, workers not started");
    }

    if let Some(rx) = ul_rx {
        let app_ul = app.clone();
        handles.upload = Some(tokio::spawn(async move {
            sync_upload::start_upload_worker(app_ul, rx).await;
        }));
    } else {
        log::warn!("Upload receivers unavailable, workers not started");
    }

    Ok(())
}

pub async fn auto_start_sync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let has_token = state.api.read().await.has_token();
    if !has_token {
        log::info!("No auth token, skipping auto-start sync");
        return;
    }

    startup_recovery(app).await;

    if let Err(e) = start_periodic_sync(app).await {
        log::warn!("Auto-start sync failed: {e}");
        return;
    }
}

/// Fetch all not-ready episodes and push them to channels as Normal priority.
async fn startup_recovery(app: &AppHandle) {
    let feeds = match feeds_service::fetch_all_feeds(app).await {
        Ok(f) => f,
        Err(e) => {
            log::warn!("Failed to fetch feeds: {e}");
            return;
        }
    };

    for feed in &feeds {
        push_feed_episodes(app, &feed.id, Priority::Normal).await;
    }
}

pub async fn stop_periodic_sync(app: AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let mut handles = state.sync_handles.lock().await;
    if let Some(h) = handles.scan.take() {
        h.abort();
    }
    if let Some(h) = handles.download.take() {
        h.abort();
    }
    if let Some(h) = handles.upload.take() {
        h.abort();
    }
    // Reset channels so a future start can obtain fresh receivers.
    state.sync_channels.reset().await;
    Ok(())
}

