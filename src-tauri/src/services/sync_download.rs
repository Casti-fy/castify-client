use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::services::{episode as episode_service, extractor, helpers};
use crate::state::{AppState, ChannelReceivers, Job, Priority};

const SEEN_CAP: usize = 1000;

async fn process_download(app: &AppHandle, job: Job) -> Result<(), AppError> {
    let feed_id = &job.feed_id;
    let episode_id = &job.episode_id;

    let temp_dir = helpers::temp_dir_for_feed(feed_id);
    if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
        log::warn!("Failed to create temp dir: {e}");
        return Ok(());
    }

    let audio_path = temp_dir.join(format!("{}.m4a", job.video_id));
    let ep_url = job.episode_url.clone();

    if audio_path.exists() {
        log::info!("Already downloaded locally: {}", job.episode_title);
        let _ = episode_service::update_status(app, episode_id, "uploading", None).await;
        let state = app.state::<AppState>();
        state.sync_channels.send_upload(job).await;
        return Ok(());
    }

    // Optional: we could still backfill metadata here using ep_url without needing feed_detail,
    // but to keep backend/API pressure minimal, we skip it in this refactor.

    helpers::emit_progress(
        app,
        feed_id,
        &job.feed_name,
        "download",
        &format!("Downloading: {}", job.episode_title),
    );

    match extractor::download_audio(app, &ep_url, &job.video_id, &temp_dir).await {
        Ok(_) => {
            let _ = episode_service::update_status(app, episode_id, "uploading", None).await;

            let state = app.state::<AppState>();
            state.sync_channels.send_upload(job).await;
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("Premieres in") || err_str.contains("is_upcoming") {
                log::info!("Skipping premiere, will retry later: {}", job.episode_title);
            } else {
                log::warn!("Download failed for {}: {e}", job.episode_title);
                let _ = episode_service::update_status(app, episode_id, "failed", None).await;
            }
        }
    }

    Ok(())
}

pub async fn start_download_worker(app: AppHandle, mut channels: ChannelReceivers) {
    let max_concurrent = helpers::cpu_count().clamp(2, 4);
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

        // Skip jobs for feeds that have been deleted.
        if app
            .app_handle()
            .state::<AppState>()
            .cancelled_feeds
            .read()
            .await
            .contains(&job.feed_id)
        {
            continue;
        }

        // De-dupe by (feed_id, video_id) since multiple episodes can point to the same
        // underlying media ID (e.g. SoundCloud track id). Running two yt-dlp downloads
        // into the same temp dir with the same output template can race on fragment files.
        let seen_key = format!("{}:{}", job.feed_id, job.video_id);
        if !seen.insert(seen_key) {
            continue;
        }
        if seen.len() >= SEEN_CAP {
            seen.clear();
        }

        let sem = semaphore.clone();
        let app = app.clone();
        let job_priority = job.priority;
        let job_for_task = job.clone();

        let permit = sem.acquire_owned().await.unwrap();
        tokio::spawn(async move {
            let _permit = permit;

            if job_priority != Priority::Urgent {
                let delay = rand::thread_rng().gen_range(15..=30);
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }

            if let Err(e) = process_download(&app, job_for_task).await {
                log::warn!("Download job failed (episode {}): {e}", job.episode_id);
            }
        });
    }
}

