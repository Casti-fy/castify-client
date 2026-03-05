use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::AppError;
use crate::models::*;
use crate::services::{extractor, uploader};
use crate::state::AppState;

fn emit_progress(app: &AppHandle, feed_id: &str, feed_name: &str, step: &str, message: &str) {
    let _ = app.emit(
        "sync-progress",
        SyncProgressEvent {
            feed_id: feed_id.to_string(),
            feed_name: feed_name.to_string(),
            step: step.to_string(),
            message: message.to_string(),
        },
    );
}

pub async fn sync_feed_internal(app: &AppHandle, feed: &Feed) -> Result<(), AppError> {
    // Pass 1: fast scan & create episodes
    scan_feed(app, feed).await?;

    // Pass 2 & 3: download and upload run concurrently via a channel.
    // As each episode finishes downloading, it's sent to the upload task.
    let (tx, rx) = tokio::sync::mpsc::channel::<Episode>(8);

    let app_dl = app.clone();
    let feed_dl = feed.clone();
    let download_task = tokio::spawn(async move {
        if let Err(e) = download_feed(&app_dl, &feed_dl, tx).await {
            log::error!("Download feed {} failed: {e}", feed_dl.name);
        }
    });

    let app_ul = app.clone();
    let feed_ul = feed.clone();
    let upload_task = tokio::spawn(async move {
        if let Err(e) = upload_feed(&app_ul, &feed_ul, rx).await {
            log::error!("Upload feed {} failed: {e}", feed_ul.name);
        }
    });

    // Don't block the caller — let both run in background
    tokio::spawn(async move {
        let _ = download_task.await;
        let _ = upload_task.await;
    });

    Ok(())
}

/// Pass 1: Fast flat-playlist scan. Creates episodes in DB with basic info.
async fn scan_feed(app: &AppHandle, feed: &Feed) -> Result<(), AppError> {
    let state = app.state::<AppState>();

    emit_progress(app, &feed.id, &feed.name, "fetch", "Fetching playlist...");

    let detail: FeedDetailResponse = state
        .api
        .read()
        .await
        .request(&format!("/api/v1/feeds/{}", feed.id), "GET", true)
        .await?;

    let existing_ids: HashSet<String> = detail.episodes.iter().map(|e| e.video_id.clone()).collect();
    let entries = extractor::fetch_playlist_fast(app, &feed.source_url, 15).await?;
    let new_entries: Vec<&PlaylistEntry> = entries
        .iter()
        .filter(|e| e.id.as_ref().map_or(true, |id| !existing_ids.contains(id)))
        .collect();

    if new_entries.is_empty() {
        emit_progress(app, &feed.id, &feed.name, "done", "Already up to date");
        return Ok(());
    }

    for entry in &new_entries {
        let video_id = match &entry.id {
            Some(id) => id.clone(),
            None => continue,
        };
        let title = entry.title.clone().unwrap_or_else(|| video_id.clone());

        emit_progress(app, &feed.id, &feed.name, "create", &format!("Creating: {title}"));

        let create_body = CreateEpisodeRequest {
            video_id: video_id.clone(),
            title: title.clone(),
            description: None,
            pub_date: None,
            duration_sec: entry.duration.map(|d| d as i64),
        };

        if let Err(e) = state
            .api
            .read()
            .await
            .request_with_body::<CreateEpisodeResponse, _>(
                &format!("/api/v1/feeds/{}/episodes", feed.id),
                "POST",
                Some(&create_body),
                true,
            )
            .await
        {
            log::warn!("Failed to create episode {title}: {e}");
        }
    }

    emit_progress(app, &feed.id, &feed.name, "done", &format!("Found {} new episodes", new_entries.len()));
    Ok(())
}

/// Pass 2: For each pending/failed episode, fetch metadata and download audio.
/// Sends completed episodes to the upload task via the channel.
async fn download_feed(
    app: &AppHandle,
    feed: &Feed,
    tx: tokio::sync::mpsc::Sender<Episode>,
) -> Result<(), AppError> {
    let state = app.state::<AppState>();

    let detail: FeedDetailResponse = state
        .api
        .read()
        .await
        .request(&format!("/api/v1/feeds/{}", feed.id), "GET", true)
        .await?;

    let pending_episodes: Vec<Episode> = detail
        .episodes
        .into_iter()
        .filter(|e| e.status == "pending" || e.status == "failed")
        .collect();

    if pending_episodes.is_empty() {
        return Ok(());
    }

    let temp_dir = temp_dir_for_feed(&feed.id);
    tokio::fs::create_dir_all(&temp_dir).await?;

    for ep in pending_episodes {
        // Fetch full metadata
        emit_progress(app, &feed.id, &feed.name, "metadata", &format!("Fetching metadata: {}", ep.title));
        match extractor::fetch_video_metadata(app, &ep.video_id).await {
            Ok(meta) => {
                let pub_date = format_upload_date(meta.upload_date.as_deref());
                let update_body = UpdateEpisodeMetadataRequest {
                    description: meta.description.clone(),
                    pub_date,
                    duration_sec: meta.duration.map(|d| d as i64),
                };
                if let Err(e) = state
                    .api
                    .read()
                    .await
                    .request_no_content(
                        &format!("/api/v1/episodes/{}", ep.id),
                        "PATCH",
                        Some(&update_body),
                        true,
                    )
                    .await
                {
                    log::warn!("Failed to update metadata for {}: {e}", ep.title);
                }
            }
            Err(e) => log::warn!("Failed to fetch metadata for {}: {e}", ep.title),
        }

        // Download audio
        emit_progress(app, &feed.id, &feed.name, "download", &format!("Downloading: {}", ep.title));
        match extractor::extract_audio(app, &ep.video_id, &temp_dir).await {
            Ok(_) => {
                let _ = update_episode_status(app, &ep.id, "downloaded", None).await;
                let _ = tx.send(ep).await;
            }
            Err(e) => {
                log::warn!("Download failed for {}: {e}", ep.title);
                let _ = update_episode_status(app, &ep.id, "failed", None).await;
            }
        }
    }

    emit_progress(app, &feed.id, &feed.name, "done", "Downloads complete");
    Ok(())
}

/// Pass 3: Receives downloaded episodes and uploads them to B2.
async fn upload_feed(
    app: &AppHandle,
    feed: &Feed,
    mut rx: tokio::sync::mpsc::Receiver<Episode>,
) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let temp_dir = temp_dir_for_feed(&feed.id);

    while let Some(ep) = rx.recv().await {
        let audio_path = temp_dir.join(format!("{}.m4a", ep.video_id));
        if !audio_path.exists() {
            log::warn!("Audio file missing for {}, marking failed", ep.title);
            let _ = update_episode_status(app, &ep.id, "failed", None).await;
            continue;
        }

        // Get upload URL
        let url_resp: UploadURLResponse = match state
            .api
            .read()
            .await
            .request_with_body(
                &format!("/api/v1/episodes/{}/upload-url", ep.id),
                "POST",
                Some(&serde_json::json!({})),
                true,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log::warn!("Failed to get upload URL for {}: {e}", ep.title);
                let _ = update_episode_status(app, &ep.id, "failed", None).await;
                continue;
            }
        };

        // Upload
        emit_progress(app, &feed.id, &feed.name, "upload", &format!("Uploading: {}", ep.title));
        match uploader::upload_to_b2(&audio_path, &url_resp.upload_url, &url_resp.authorization_token, &url_resp.file_name).await {
            Ok(()) => {
                let file_size = tokio::fs::metadata(&audio_path).await.map(|m| m.len()).unwrap_or(0);
                let _ = update_episode_status(app, &ep.id, "ready", Some(file_size)).await;
                let _ = tokio::fs::remove_file(&audio_path).await;
                emit_progress(app, &feed.id, &feed.name, "complete", &format!("Done: {}", ep.title));
            }
            Err(e) => {
                log::warn!("Upload failed for {}: {e}", ep.title);
                let _ = update_episode_status(app, &ep.id, "failed", None).await;
            }
        }
    }

    // Clean up temp dir if empty
    let _ = tokio::fs::remove_dir(&temp_dir).await;
    emit_progress(app, &feed.id, &feed.name, "done", "Upload complete");
    Ok(())
}

fn temp_dir_for_feed(feed_id: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("castify-{feed_id}"))
}

fn format_upload_date(upload_date: Option<&str>) -> Option<String> {
    upload_date.and_then(|d| {
        if d.len() == 8 {
            Some(format!(
                "{}-{}-{}T00:00:00Z",
                &d[..4],
                &d[4..6],
                &d[6..8]
            ))
        } else {
            None
        }
    })
}

async fn update_episode_status(
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
    let result = state
        .api
        .read()
        .await
        .request_no_content(
            &format!("/api/v1/episodes/{episode_id}"),
            "PATCH",
            Some(&body),
            true,
        )
        .await;
    result
}

// -- Tauri commands --

#[tauri::command]
pub async fn sync_feed(app: AppHandle, feed_id: String) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let detail: FeedDetailResponse = state
        .api
        .read()
        .await
        .request(&format!("/api/v1/feeds/{feed_id}"), "GET", true)
        .await?;

    sync_feed_internal(&app, &detail.feed).await
}

#[tauri::command]
pub async fn start_periodic_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    interval_minutes: u64,
) -> Result<(), AppError> {
    let mut handle = state.sync_handle.lock().await;
    if let Some(h) = handle.take() {
        h.abort();
    }

    let api = Arc::clone(&state.api);
    let interval = Duration::from_secs(interval_minutes * 60);

    let task = tokio::spawn(async move {
        loop {
            // Fetch feeds
            let feeds: Vec<Feed> = match api
                .read()
                .await
                .request("/api/v1/feeds", "GET", true)
                .await
            {
                Ok(f) => f,
                Err(e) => {
                    log::warn!("Periodic sync: failed to fetch feeds: {e}");
                    tokio::time::sleep(interval).await;
                    continue;
                }
            };

            for feed in &feeds {
                let _ = sync_feed_internal(&app, feed).await;
            }

            tokio::time::sleep(interval).await;
        }
    });

    *handle = Some(task);
    Ok(())
}

#[tauri::command]
pub async fn stop_periodic_sync(state: State<'_, AppState>) -> Result<(), AppError> {
    let mut handle = state.sync_handle.lock().await;
    if let Some(h) = handle.take() {
        h.abort();
    }
    Ok(())
}
