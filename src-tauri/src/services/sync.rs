use std::time::Duration;

use crate::error::AppError;
use crate::models::Feed;
use crate::services::{episode as episode_service, extractor, extractor::PlaylistOrder, feeds as feeds_service};
use crate::state::{AppState, Job, Priority};

use super::{sync_download, sync_scan, sync_upload};

fn parse_playlist_order(s: &str) -> PlaylistOrder {
    match s {
        "oldest" => PlaylistOrder::Oldest,
        "popular" => PlaylistOrder::Popular,
        _ => PlaylistOrder::Newest,
    }
}


/// Push a feed's not-ready episodes to channels with the given priority.
pub async fn push_feed_episodes(state: &AppState, feed_id: &str, priority: Priority) {
    let detail = match feeds_service::fetch_feed_detail(state, feed_id).await {
        Ok(d) => d,
        Err(e) => {
            log::warn!("Failed to fetch feed detail for push: {e}");
            return;
        }
    };
    log::info!("Pushing feed episodes for feed {feed_id}: {:?}", detail.episodes);
    for ep in &detail.episodes {
        match ep.status.as_str() {
            "pending" | "failed" => {
                let episode_url =
                    extractor::episode_url(&detail.feed.source_url, &ep.video_id);
                log::info!("Pushing episode {:?} to download channel", ep);
                state
                    .sync_channels
                    .send_download(Job {
                        feed_id: feed_id.to_string(),
                        feed_name: detail.feed.name.clone(),
                        episode_id: ep.id.clone(),
                        episode_title: ep.title.clone(),
                        video_id: ep.video_id.clone(),
                        episode_url,
                        pub_date: ep.pub_date.clone(),
                        priority,
                    })
                    .await;
            }
            _ => {}
        }
    }
}

/// Scan the first N episodes of a feed and push not-ready ones as Urgent.
/// Also fetches the channel artwork in parallel.
pub async fn scan_new_feed(state: &AppState, feed: &Feed) {
    let feed_id = feed.id.clone();
    let source_url = feed.source_url.clone();
    let state_artwork = state.clone();
    let artwork_handle = tokio::spawn(async move {
        match extractor::fetch_channel_artwork_url(&state_artwork, &source_url).await {
            Ok(Some(url)) => {
                if let Err(e) = feeds_service::update_feed_artwork(&state_artwork, &feed_id, &url).await {
                    log::warn!("[scan_new_feed] failed to update artwork: {e}");
                }
            }
            Ok(None) => log::info!("[scan_new_feed] no artwork found for feed {feed_id}"),
            Err(e) => log::warn!("[scan_new_feed] failed to fetch artwork: {e}"),
        }
    });

    let feeds = [feed.clone()];
    let order = parse_playlist_order(&feed.fetch_order);
    run_sync_for_feeds(state, &feeds, 0, 5, order, Priority::Urgent).await;
    let _ = artwork_handle.await;
}

/// Sync a single feed: scan for new episodes, then push any not-ready ones as Urgent.
pub async fn sync_single_feed(state: &AppState, feed_id: &str) -> Result<(), AppError> {
    let detail = feeds_service::fetch_feed_detail(state, feed_id).await?;
    let feed = detail.feed.clone();
    push_feed_episodes(state, feed_id, Priority::Urgent).await;
    let order = parse_playlist_order(&feed.fetch_order);
    run_sync_for_feeds(state, &[feed], 0, 20, order, Priority::Urgent).await;
    Ok(())
}

/// Backfill older episodes from a feed's playlist using a custom index range.
pub async fn backfill_feed(state: &AppState, feed_id: &str, start: u32, end: u32) -> Result<(), AppError> {
    let detail = feeds_service::fetch_feed_detail(state, feed_id).await?;
    let feed = detail.feed.clone();
    let order = parse_playlist_order(&feed.fetch_order);
    run_sync_for_feeds(state, &[feed], start, end, order, Priority::Normal).await;
    Ok(())
}

pub async fn run_sync_for_feeds(
    state: &AppState,
    feeds: &[Feed],
    start: u32,
    end: u32,
    order: PlaylistOrder,
    priority: Priority,
) {
    sync_scan::run_scan(state, feeds, start, end, order, priority).await;
}

// ----- Settings helpers -----

pub fn read_sync_interval(state: &AppState) -> u64 {
    state.store.read_sync_interval()
}

pub fn write_sync_interval(state: &AppState, minutes: u64) {
    state.store.write_sync_interval(minutes);
}

// ----- Periodic sync orchestration -----

pub async fn start_periodic_sync(state: &AppState) -> Result<(), AppError> {
    let state_clone = state.clone();
    let mut handles = state.sync_handles.lock().await;

    if handles.download.is_some() {
        log::info!("Periodic sync already running");
        return Ok(());
    }

    log::info!("Starting periodic sync");

    // Start download/upload workers BEFORE anything enqueues jobs. Receivers are
    // taken once here; if missing (e.g. after stop), recreate channels first.
    if state.sync_channels.download_rx.lock().await.is_none()
        || state.sync_channels.upload_rx.lock().await.is_none()
    {
        log::info!("Recreating sync channels");
        state.sync_channels.reset().await;
    }

    let dl_rx = state.sync_channels.download_rx.lock().await.take();
    let ul_rx = state.sync_channels.upload_rx.lock().await.take();

    if let Some(rx) = dl_rx {
        let state_dl = state_clone.clone();
        handles.download = Some(tokio::spawn(async move {
            sync_download::start_download_worker(state_dl, rx).await;
        }));
    } else {
        log::warn!("Download receivers unavailable, workers not started");
    }

    if let Some(rx) = ul_rx {
        let state_ul = state_clone.clone();
        handles.upload = Some(tokio::spawn(async move {
            sync_upload::start_upload_worker(state_ul, rx).await;
        }));
    } else {
        log::warn!("Upload receivers unavailable, workers not started");
    }

    let state_scan = state_clone.clone();
    handles.scan = Some(tokio::spawn(async move {
        let mut last_scan: Option<tokio::time::Instant> = None;
        loop {
            let interval_minutes = read_sync_interval(&state_scan);
            let interval = Duration::from_secs(interval_minutes * 60);
            if let Some(last) = last_scan {
                if last.elapsed() < interval {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    continue;
                }
            }

            let feeds: Vec<Feed> = match feeds_service::fetch_all_feeds(&state_scan).await {
                Ok(f) => f,
                Err(e) => {
                    log::warn!("Failed to fetch feeds: {e}");
                    continue;
                }
            };
            for feed in &feeds {
                let order = parse_playlist_order(&feed.fetch_order);
                run_sync_for_feeds(&state_scan, &[feed.clone()], 0, 5, order, Priority::High).await;
            }
            requeue_incomplete(&state_scan, Priority::High).await;
            last_scan = Some(tokio::time::Instant::now());
        }
    }));

    Ok(())
}

pub async fn auto_start_sync(state: &AppState) {
    let has_token = state.api.read().await.has_token();
    if !has_token {
        log::info!("No auth token, skipping auto-start sync");
        return;
    }

    if let Err(e) = start_periodic_sync(state).await {
        log::warn!("Auto-start sync failed: {e}");
        return;
    }

    let state_requeue = state.clone();
    tokio::spawn(async move {
        requeue_incomplete(&state_requeue, Priority::Normal).await;
    });
}

/// Fetch all incomplete episodes in one request and push them to the download channel.
async fn requeue_incomplete(state: &AppState, priority: Priority) {
    let episodes = match episode_service::fetch_incomplete(state).await {
        Ok(eps) => eps,
        Err(e) => {
            log::warn!("Failed to fetch incomplete episodes: {e}");
            return;
        }
    };

    log::info!("Requeue incomplete: {} episodes at {:?} priority", episodes.len(), priority);
    for ep in &episodes {
        let episode_url = extractor::episode_url(&ep.feed_source_url, &ep.video_id);
        state
            .sync_channels
            .send_download(Job {
                feed_id: ep.feed_id.clone(),
                feed_name: ep.feed_name.clone(),
                episode_id: ep.id.clone(),
                episode_title: ep.title.clone(),
                video_id: ep.video_id.clone(),
                episode_url,
                pub_date: ep.pub_date.clone(),
                priority,
            })
            .await;
    }
}

pub async fn stop_periodic_sync(state: &AppState) -> Result<(), AppError> {
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
    state.sync_channels.reset().await;
    Ok(())
}
