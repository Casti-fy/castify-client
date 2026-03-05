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
    let state = app.state::<AppState>();

    emit_progress(app, &feed.id, &feed.name, "fetch", "Fetching playlist...");

    let detail: FeedDetailResponse = state
        .api
        .read()
        .await
        .request(&format!("/api/v1/feeds/{}", feed.id), "GET", true)
        .await?;

    let existing_ids: HashSet<String> = detail.episodes.iter().map(|e| e.video_id.clone()).collect();
    let entries = extractor::fetch_playlist(app, &feed.source_url, 15).await?;
    let new_entries: Vec<&PlaylistEntry> = entries
        .iter()
        .filter(|e| e.id.as_ref().map_or(true, |id| !existing_ids.contains(id)))
        .collect();

    let failed_episodes: Vec<&Episode> = detail
        .episodes
        .iter()
        .filter(|e| e.status == "failed")
        .collect();

    if new_entries.is_empty() && failed_episodes.is_empty() {
        emit_progress(app, &feed.id, &feed.name, "done", "Already up to date");
        return Ok(());
    }

    let temp_dir = std::env::temp_dir().join(format!("castify-{}", uuid_simple()));
    tokio::fs::create_dir_all(&temp_dir).await?;

    // Retry failed episodes
    for ep in &failed_episodes {
        emit_progress(
            app,
            &feed.id,
            &feed.name,
            "retry",
            &format!("Retrying: {}", ep.title),
        );

        let result = retry_episode(app, &state, ep).await;
        if let Err(e) = result {
            log::warn!("Retry failed for {}: {e}", ep.title);
            let _ = update_episode_status(&state, &ep.id, "failed", None).await;
        }
    }

    // Process new entries
    for entry in &new_entries {
        let video_id = match &entry.id {
            Some(id) => id.clone(),
            None => continue,
        };
        let title = entry.title.clone().unwrap_or_else(|| video_id.clone());
        let pub_date = entry.upload_date.as_ref().and_then(|d| {
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
        });
        let duration_sec = entry.duration.map(|d| d as i64);

        emit_progress(
            app,
            &feed.id,
            &feed.name,
            "create",
            &format!("Creating: {title}"),
        );

        // Step 1: Create episode
        let create_body = CreateEpisodeRequest {
            video_id: video_id.clone(),
            title: title.clone(),
            description: entry.description.clone(),
            pub_date,
            duration_sec,
        };

        let resp: CreateEpisodeResponse = match state
            .api
            .read()
            .await
            .request_with_body(
                &format!("/api/v1/feeds/{}/episodes", feed.id),
                "POST",
                Some(&create_body),
                true,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log::warn!("Failed to create episode {title}: {e}");
                continue;
            }
        };

        // Steps 2-4: Download, upload, update
        if let Err(e) = process_episode(
            app,
            &state,
            &temp_dir,
            &feed.id,
            &feed.name,
            &video_id,
            &title,
            &resp.episode.id,
            &resp.upload_url,
            &resp.authorization_token,
            &resp.file_name,
        )
        .await
        {
            log::warn!("Episode {title} failed: {e}");
            let _ = update_episode_status(&state, &resp.episode.id, "failed", None).await;
        }
    }

    // Cleanup temp dir
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    emit_progress(app, &feed.id, &feed.name, "done", "Sync complete");
    Ok(())
}

async fn retry_episode(
    app: &AppHandle,
    state: &State<'_, AppState>,
    ep: &Episode,
) -> Result<(), AppError> {
    let temp_dir = std::env::temp_dir().join(format!("castify-retry-{}", uuid_simple()));
    tokio::fs::create_dir_all(&temp_dir).await?;

    // Get a fresh upload URL
    let url_resp: UploadURLResponse = state
        .api
        .read()
        .await
        .request_with_body(
            &format!("/api/v1/episodes/{}/upload-url", ep.id),
            "POST",
            Some(&serde_json::json!({})),
            true,
        )
        .await?;

    let result = process_episode(
        app,
        state,
        &temp_dir,
        &ep.feed_id,
        &ep.title,
        &ep.video_id,
        &ep.title,
        &ep.id,
        &url_resp.upload_url,
        &url_resp.authorization_token,
        &url_resp.file_name,
    )
    .await;

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn process_episode(
    app: &AppHandle,
    state: &State<'_, AppState>,
    temp_dir: &std::path::Path,
    feed_id: &str,
    feed_name: &str,
    video_id: &str,
    title: &str,
    episode_id: &str,
    upload_url: &str,
    auth_token: &str,
    file_name: &str,
) -> Result<(), AppError> {
    // Step 2: Download
    emit_progress(app, feed_id, feed_name, "download", &format!("Downloading: {title}"));
    let audio_path = extractor::extract_audio(app, video_id, temp_dir).await?;

    // Step 3: Upload
    emit_progress(app, feed_id, feed_name, "upload", &format!("Uploading: {title}"));
    uploader::upload_to_b2(&audio_path, upload_url, auth_token, file_name).await?;

    // Step 4: Update status
    let file_size = tokio::fs::metadata(&audio_path).await?.len();
    update_episode_status(state, episode_id, "ready", Some(file_size)).await?;

    emit_progress(
        app,
        feed_id,
        feed_name,
        "complete",
        &format!("Done: {title}"),
    );
    Ok(())
}

async fn update_episode_status(
    state: &State<'_, AppState>,
    episode_id: &str,
    status: &str,
    file_size: Option<u64>,
) -> Result<(), AppError> {
    let body = UpdateEpisodeRequest {
        status: status.to_string(),
        file_size,
    };
    state
        .api
        .read()
        .await
        .request_no_content(
            &format!("/api/v1/episodes/{episode_id}"),
            "PATCH",
            Some(&body),
            true,
        )
        .await
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{n:x}")
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
