use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::error::AppError;
use crate::models::UploadURLResponse;
use crate::services::{episode as episode_service, feeds as feeds_service, uploader};
use crate::state::ChannelReceivers;

const SEEN_CAP: usize = 1000;

fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

fn temp_dir_for_feed(feed_id: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("castify-{feed_id}"))
}

fn emit_progress(app: &AppHandle, feed_id: &str, feed_name: &str, step: &str, message: &str) {
    let _ = app.emit(
        "sync-progress",
        crate::models::SyncProgressEvent {
            feed_id: feed_id.to_string(),
            feed_name: feed_name.to_string(),
            step: step.to_string(),
            message: message.to_string(),
        },
    );
}

async fn process_upload(
    app: &AppHandle,
    feed_id: &str,
    episode_id: &str,
) -> Result<(), AppError> {
    let detail = feeds_service::fetch_feed_detail(app, feed_id).await?;
    let feed = &detail.feed;
    let ep = detail
        .episodes
        .iter()
        .find(|e| e.id == episode_id)
        .ok_or_else(|| AppError::Other(format!("Episode {episode_id} not found")))?;

    if ep.status != "uploading" {
        return Ok(());
    }

    let temp_dir = temp_dir_for_feed(feed_id);
    let audio_path = temp_dir.join(format!("{}.m4a", ep.video_id));

    if !audio_path.exists() {
        log::warn!("Audio file missing for {}, marking failed", ep.title);
        let _ = episode_service::update_status(app, &ep.id, "failed", None).await;
        return Ok(());
    }

    let url_resp: UploadURLResponse = match episode_service::get_upload_url(app, &ep.id).await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to get upload URL for {}: {e}", ep.title);
            let _ = episode_service::update_status(app, &ep.id, "failed", None).await;
            return Ok(());
        }
    };

    emit_progress(
        app,
        feed_id,
        &feed.name,
        "upload",
        &format!("Uploading: {}", ep.title),
    );

    match uploader::upload_to_b2(
        &audio_path,
        &url_resp.upload_url,
        &url_resp.authorization_token,
        &url_resp.file_name,
    )
    .await
    {
        Ok(()) => {
            let file_size = tokio::fs::metadata(&audio_path)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            let _ = episode_service::update_status(app, &ep.id, "ready", Some(file_size)).await;
            let _ = tokio::fs::remove_file(&audio_path).await;
            let _ = tokio::fs::remove_dir(temp_dir_for_feed(feed_id)).await;
            emit_progress(
                app,
                feed_id,
                &feed.name,
                "complete",
                &format!("Done: {}", ep.title),
            );
        }
        Err(e) => {
            log::warn!("Upload failed for {}: {e}", ep.title);
            let _ = episode_service::update_status(app, &ep.id, "failed", None).await;
        }
    }

    Ok(())
}

pub async fn start_upload_worker(app: AppHandle, mut channels: ChannelReceivers) {
    let max_concurrent = (cpu_count() / 2).max(1);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let mut seen: HashSet<String> = HashSet::new();

    loop {
        let job = tokio::select! {
            biased;
            Some(job) = channels.urgent_rx.recv() => job,
            Some(job) = channels.high_rx.recv() => job,
            Some(job) = channels.normal_rx.recv() => job,
            else => {
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        };

        if !seen.insert(job.episode_id.clone()) {
            continue;
        }
        if seen.len() >= SEEN_CAP {
            seen.clear();
        }

        let sem = semaphore.clone();
        let app = app.clone();
        let feed_id = job.feed_id.clone();
        let episode_id = job.episode_id.clone();

        let permit = sem.acquire_owned().await.unwrap();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = process_upload(&app, &feed_id, &episode_id).await {
                log::warn!("Upload job failed (episode {episode_id}): {e}");
            }
        });
    }
}

