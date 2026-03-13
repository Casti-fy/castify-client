use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppError;
use crate::services::{episode as episode_service, extractor, feeds as feeds_service};
use crate::state::{AppState, ChannelReceivers, Job, Priority};

const SEEN_CAP: usize = 1000;

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

async fn push_to_upload(app: &AppHandle, feed_id: &str, episode_id: &str, priority: Priority) {
    let state = app.state::<AppState>();
    state
        .sync_channels
        .send_upload(Job {
            feed_id: feed_id.to_string(),
            episode_id: episode_id.to_string(),
            priority,
        })
        .await;
}

async fn process_download(
    app: &AppHandle,
    feed_id: &str,
    episode_id: &str,
    priority: Priority,
) -> Result<(), AppError> {
    let detail = feeds_service::fetch_feed_detail(app, feed_id).await?;
    let feed = &detail.feed;
    let ep = detail
        .episodes
        .iter()
        .find(|e| e.id == episode_id)
        .ok_or_else(|| AppError::Other(format!("Episode {episode_id} not found")))?;

    if ep.status != "pending" && ep.status != "failed" {
        return Ok(());
    }

    let temp_dir = temp_dir_for_feed(feed_id);
    if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
        log::warn!("Failed to create temp dir: {e}");
        return Ok(());
    }

    let audio_path = temp_dir.join(format!("{}.m4a", ep.video_id));
    if audio_path.exists() {
        log::info!("Already downloaded locally: {}", ep.title);
        let _ = episode_service::update_status(app, &ep.id, "uploading", None).await;
        push_to_upload(app, feed_id, episode_id, priority).await;
        return Ok(());
    }

    let ep_url = extractor::episode_url(&feed.source_url, &ep.video_id);

    emit_progress(
        app,
        feed_id,
        &feed.name,
        "download",
        &format!("Downloading: {}", ep.title),
    );

    match extractor::extract_audio(app, &ep_url, &ep.video_id, &temp_dir).await {
        Ok(_) => {
            let _ = episode_service::update_status(app, &ep.id, "uploading", None).await;
            push_to_upload(app, feed_id, episode_id, priority).await;
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("Premieres in") || err_str.contains("is_upcoming") {
                log::info!("Skipping premiere, will retry later: {}", ep.title);
            } else {
                log::warn!("Download failed for {}: {e}", ep.title);
                let _ = episode_service::update_status(app, &ep.id, "failed", None).await;
            }
        }
    }

    Ok(())
}

fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

pub async fn start_download_worker(app: AppHandle, mut channels: ChannelReceivers) {
    let max_concurrent = cpu_count().clamp(2, 4);
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
        let job_priority = job.priority;
        let feed_id = job.feed_id.clone();
        let episode_id = job.episode_id.clone();

        let permit = sem.acquire_owned().await.unwrap();
        tokio::spawn(async move {
            let _permit = permit;

            if job_priority != Priority::Urgent {
                let delay = rand::thread_rng().gen_range(15..=30);
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }

            if let Err(e) = process_download(&app, &feed_id, &episode_id, job_priority).await {
                log::warn!("Download job failed (episode {episode_id}): {e}");
            }
        });
    }
}

