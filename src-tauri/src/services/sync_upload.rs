use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use crate::error::AppError;
use crate::models::UploadURLResponse;
use crate::services::{episode as episode_service, helpers, uploader};
use crate::state::{AppState, ChannelReceivers, Job};

const SEEN_CAP: usize = 1000;
/// B2 can return 503 under load; refetch upload URL before each retry (B2 recommendation).
const UPLOAD_MAX_ATTEMPTS: u32 = 6;

fn upload_backoff_secs(attempt: u32) -> u64 {
    (2u64.saturating_pow(attempt.saturating_sub(1))).min(45)
}

async fn process_upload(state: &AppState, job: Job) -> Result<(), AppError> {
    let feed_id = &job.feed_id;
    let episode_id = &job.episode_id;

    let temp_dir = helpers::temp_dir_for_feed(feed_id);
    let audio_path = temp_dir.join(format!("{}.m4a", job.video_id));

    if !audio_path.exists() {
        log::warn!(
            "Audio file missing for {}, marking failed",
            job.episode_title
        );
        let _ = episode_service::update_status(state, episode_id, "failed", None).await;
        return Ok(());
    }

    helpers::emit_progress(
        state,
        feed_id,
        &job.feed_name,
        "upload",
        &format!("Uploading: {}", job.episode_title),
    );

    for attempt in 1..=UPLOAD_MAX_ATTEMPTS {
        let url_resp: UploadURLResponse =
            match episode_service::get_upload_url(state, episode_id).await {
                Ok(r) => r,
                Err(e) => {
                    if attempt > 1 {
                        log::warn!(
                            "get_upload_url retry ({}/{}) for {}: {e}",
                            attempt,
                            UPLOAD_MAX_ATTEMPTS,
                            job.episode_title
                        );
                        if attempt < UPLOAD_MAX_ATTEMPTS {
                            tokio::time::sleep(Duration::from_secs(
                                upload_backoff_secs(attempt),
                            ))
                            .await;
                            continue;
                        }
                    } else {
                        log::warn!("Failed to get upload URL for {}: {e}", job.episode_title);
                    }
                    let _ = episode_service::update_status(state, episode_id, "failed", None).await;
                    return Ok(());
                }
            };

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
                let _ = episode_service::update_status(state, episode_id, "ready", Some(file_size))
                    .await;
                helpers::emit_progress(
                    state,
                    feed_id,
                    &job.feed_name,
                    "complete",
                    &format!("Done: {}", job.episode_title),
                );
                return Ok(());
            }
            Err(e) => {
                let transient = uploader::upload_error_is_transient(&e);
                if transient && attempt < UPLOAD_MAX_ATTEMPTS {
                    log::warn!(
                        "Upload attempt {}/{} for {}: {e}, retrying in {}s",
                        attempt,
                        UPLOAD_MAX_ATTEMPTS,
                        job.episode_title,
                        upload_backoff_secs(attempt)
                    );
                    tokio::time::sleep(Duration::from_secs(upload_backoff_secs(attempt))).await;
                    continue;
                }
                log::warn!("Upload failed for {}: {e}", job.episode_title);
                let _ = episode_service::update_status(state, episode_id, "failed", None).await;
                return Ok(());
            }
        }
    }

    Ok(())
}

pub async fn start_upload_worker(state: AppState, mut channels: ChannelReceivers) {
    let max_concurrent = helpers::cpu_count() / 2;
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

        if state.cancelled_feeds.read().await.contains(&job.feed_id) {
            continue;
        }

        if !seen.insert(job.episode_id.clone()) {
            continue;
        }
        if seen.len() >= SEEN_CAP {
            seen.clear();
        }

        let sem = semaphore.clone();
        let state = state.clone();
        let job_for_task = job.clone();

        let permit = sem.acquire_owned().await.unwrap();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = process_upload(&state, job_for_task.clone()).await {
                log::warn!(
                    "Upload job failed (episode {}): {e}",
                    job_for_task.episode_id
                );
            }
        });
    }
}
