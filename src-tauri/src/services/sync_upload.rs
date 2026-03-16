use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tauri::AppHandle;

use crate::error::AppError;
use crate::models::UploadURLResponse;
use crate::services::{episode as episode_service, helpers, uploader};
use crate::state::{ChannelReceivers, Job};

const SEEN_CAP: usize = 1000;

fn temp_dir_for_feed(feed_id: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("castify-{feed_id}"))
}

async fn process_upload(app: &AppHandle, job: Job) -> Result<(), AppError> {
    let feed_id = &job.feed_id;
    let episode_id = &job.episode_id;

    let temp_dir = temp_dir_for_feed(feed_id);
    let audio_path = temp_dir.join(format!("{}.m4a", job.video_id));

    if !audio_path.exists() {
        log::warn!(
            "Audio file missing for {}, marking failed",
            job.episode_title
        );
        let _ = episode_service::update_status(app, episode_id, "failed", None).await;
        return Ok(());
    }

    let url_resp: UploadURLResponse = match episode_service::get_upload_url(app, episode_id).await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to get upload URL for {}: {e}", job.episode_title);
            let _ = episode_service::update_status(app, episode_id, "failed", None).await;
            return Ok(());
        }
    };

    helpers::emit_progress(
        app,
        feed_id,
        &job.feed_name,
        "upload",
        &format!("Uploading: {}", job.episode_title),
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
            let _ = episode_service::update_status(app, episode_id, "ready", Some(file_size))
                .await;
            // let _ = tokio::fs::remove_file(&audio_path).await;
            // let _ = tokio::fs::remove_dir(temp_dir_for_feed(feed_id)).await;
            helpers::emit_progress(
                app,
                feed_id,
                &job.feed_name,
                "complete",
                &format!("Done: {}", job.episode_title),
            );
        }
        Err(e) => {
            log::warn!("Upload failed for {}: {e}", job.episode_title);
            let _ = episode_service::update_status(app, episode_id, "failed", None).await;
        }
    }

    Ok(())
}

pub async fn start_upload_worker(app: AppHandle, mut channels: ChannelReceivers) {
    let max_concurrent = (helpers::cpu_count() / 2).max(1);
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
        let job_for_task = job.clone();

        let permit = sem.acquire_owned().await.unwrap();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = process_upload(&app, job_for_task.clone()).await {
                log::warn!(
                    "Upload job failed (episode {}): {e}",
                    job_for_task.episode_id
                );
            }
        });
    }
}

