use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;

use crate::error::AppError;
use crate::models::UpdateEpisodeMetadataRequest;
use crate::services::{episode as episode_service, extractor, helpers};
use crate::state::{AppState, ChannelReceivers, Job, Priority};

pub async fn start_download_worker(state: AppState, mut channels: ChannelReceivers) {
    let max_concurrent = helpers::cpu_count().clamp(2, 4);
    log::info!("[download-queue] worker started (max_concurrent={max_concurrent})");
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let in_flight: Arc<tokio::sync::Mutex<HashSet<String>>> =
        Arc::new(tokio::sync::Mutex::new(HashSet::new()));

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

        let in_flight_key = format!("{}:{}", job.feed_id, job.video_id);
        {
            let mut active = in_flight.lock().await;
            if !active.insert(in_flight_key.clone()) {
                log::info!(
                    "[download-queue] skip duplicate in-flight {:?} ({})",
                    job.episode_title,
                    job.video_id,
                );
                continue;
            }
        }

        log::info!(
            "[download-queue] dequeue {:?} priority={} feed={} video={}",
            job.episode_title,
            job.priority.label(),
            job.feed_name,
            job.video_id,
        );

        let sem = semaphore.clone();
        let state = state.clone();
        let job_priority = job.priority;
        let job_for_task = job.clone();
        let in_flight = in_flight.clone();
        let episode_id = job.episode_id.clone();
        let episode_title = job.episode_title.clone();

        let permit = sem.acquire_owned().await.unwrap();
        log::info!(
            "[download-queue] start {:?} priority={} (in_flight={})",
            episode_title,
            job_priority.label(),
            in_flight.lock().await.len(),
        );
        tokio::spawn(async move {
            let _permit = permit;

            if job_priority != Priority::Urgent {
                let delay = rand::thread_rng().gen_range(15..=30);
                log::info!(
                    "[download-queue] waiting {delay}s before download (priority={})",
                    job_priority.label(),
                );
                tokio::time::sleep(Duration::from_secs(delay)).await;
            }

            if let Err(e) = process_download(&state, job_for_task).await {
                log::warn!("Download job failed (episode {}): {e}", episode_id);
            }

            in_flight.lock().await.remove(&in_flight_key);
            log::info!("[download-queue] done {:?} (in_flight={})", episode_title, in_flight.lock().await.len());
        });
    }
}

async fn process_download(state: &AppState, job: Job) -> Result<(), AppError> {
    let feed_id = &job.feed_id;
    let episode_id = &job.episode_id;

    let temp_dir = helpers::temp_dir_for_feed(feed_id);
    if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
        log::warn!("Failed to create temp dir: {e}");
        return Ok(());
    }

    let audio_path = temp_dir.join(format!("{}.m4a", job.video_id));
    let ep_url = job.episode_url.clone();

    if state.cancelled_feeds.read().await.contains(&job.feed_id) {
        log::info!("Feed cancelled, skipping download: {}", job.episode_title);
        return Ok(());
    }

    if audio_path.exists() {
        log::info!("Already downloaded locally: {}", job.episode_title);
        state.sync_channels.send_upload(job).await;
        return Ok(());
    }

    // One yt-dlp --dump-json call gives us pub_date (when missing/ancient) and
    // chapters (when authored). PATCH only the fields actually present.
    match extractor::fetch_episode_metadata(state, &ep_url, job.pub_date.as_deref()).await {
        Ok(meta) if meta.pub_date_patch.is_some() || meta.chapters.is_some() => {
            let body = UpdateEpisodeMetadataRequest {
                description: None,
                pub_date: meta.pub_date_patch,
                duration_sec: None,
                chapters: meta.chapters,
            };
            if let Err(e) = episode_service::update_metadata(state, episode_id, &body).await {
                log::warn!("[fetch_episode_metadata] failed to update episode {episode_id}: {e}");
            }
        }
        Ok(_) => {}
        Err(e) => log::warn!("[fetch_episode_metadata] failed for {ep_url}: {e}"),
    }

    helpers::emit_progress(
        state,
        feed_id,
        &job.feed_name,
        "download",
        &format!("Downloading: {}", job.episode_title),
    );
    log::info!(
        "[download-queue] downloading {:?} video={} url={}",
        job.episode_title,
        job.video_id,
        ep_url,
    );
    let _ = episode_service::update_status(state, episode_id, "pending", None).await;

    match extractor::download_audio(state, &ep_url, &job.video_id, &temp_dir).await {
        Ok(_) => {
            if !state.cancelled_feeds.read().await.contains(&job.feed_id) {
                state.sync_channels.send_upload(job).await;
            }
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("Premieres in") || err_str.contains("is_upcoming") {
                log::info!("Skipping premiere, will retry later: {}", job.episode_title);
            } else {
                log::warn!("Download failed for {}: {e}", job.episode_title);
                let _ = episode_service::update_status(state, episode_id, "failed", None).await;
            }
        }
    }

    Ok(())
}
