use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::AppError;
use crate::models::*;
use crate::services::{extractor, uploader};
use crate::state::AppState;

enum SyncMode {
    Force,
    Periodic,
}

fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

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

// ========== Helpers ==========

async fn fetch_all_feeds(app: &AppHandle) -> Result<Vec<Feed>, AppError> {
    let state = app.state::<AppState>();
    let result = state
        .api
        .read()
        .await
        .request("/api/v1/feeds", "GET", true)
        .await;
    result
}

async fn fetch_feed_detail(
    app: &AppHandle,
    feed_id: &str,
) -> Result<FeedDetailResponse, AppError> {
    let state = app.state::<AppState>();
    let result = state
        .api
        .read()
        .await
        .request(&format!("/api/v1/feeds/{feed_id}"), "GET", true)
        .await;
    result
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

fn temp_dir_for_feed(feed_id: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("castify-{feed_id}"))
}

/// Pick the best thumbnail from a list of playlist entries.
/// Prefers ~500px wide for podcast artwork; falls back to largest available.
fn best_thumbnail(entries: &[PlaylistEntry]) -> Option<String> {
    entries.iter()
        .flat_map(|e| e.thumbnails.iter())
        .filter(|t| t.width.unwrap_or(0) >= 100)
        .max_by_key(|t| {
            let w = t.width.unwrap_or(0);
            // Prefer 300-500px range (good for podcast artwork), then larger
            if (300..=500).contains(&w) { 10000 + w } else { w }
        })
        .map(|t| t.url.clone())
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

// ========== Pass 1: Scan ==========

async fn get_user_plan(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let result = {
        let api = state.api.read().await;
        api.request::<crate::models::User>("/api/v1/auth/me", "GET", true).await
    };
    match result {
        Ok(user) => user.plan,
        Err(_) => "starter".to_string(),
    }
}

fn plan_max_episodes(plan: &str) -> Option<u32> {
    match plan {
        "pro" | "unlimited" => None, // no limit
        _ => Some(20),               // starter: 20 episodes per feed
    }
}

fn plan_max_feeds(plan: &str) -> Option<usize> {
    match plan {
        "unlimited" => None,  // no limit
        "pro" => Some(15),
        _ => Some(3),         // starter: 3 feeds
    }
}

/// Truncate feed list to the plan's max_feeds limit.
/// Keeps the first N feeds (oldest created, as returned by the server).
fn cap_feeds_for_plan(feeds: Vec<Feed>, plan: &str) -> Vec<Feed> {
    match plan_max_feeds(plan) {
        Some(max) if feeds.len() > max => feeds.into_iter().take(max).collect(),
        _ => feeds,
    }
}

/// Minimum delay between scanning each feed to avoid a burst of "Fetching playlist" and rate limits.
const SCAN_FEED_SPACING: Duration = Duration::from_secs(2);

async fn run_scan(app: &AppHandle, feeds: &[Feed], mode: &SyncMode) {
    let plan = get_user_plan(app).await;
    for (i, feed) in feeds.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(SCAN_FEED_SPACING).await;
        }
        if let Err(e) = scan_feed(app, feed, mode, &plan).await {
            log::warn!("Scan feed {} failed: {e}", feed.name);
        }
    }
}

async fn scan_feed(app: &AppHandle, feed: &Feed, mode: &SyncMode, plan: &str) -> Result<(), AppError> {
    emit_progress(app, &feed.id, &feed.name, "fetch", "Fetching playlist...");

    let detail = fetch_feed_detail(app, &feed.id).await?;
    let existing_ids: HashSet<String> =
        detail.episodes.iter().map(|e| e.video_id.clone()).collect();

    let episode_cap = plan_max_episodes(plan);
    let max_items: u32 = episode_cap.unwrap_or(100).min(100);

    let entries = extractor::fetch_playlist(app, &feed.source_url, max_items).await?;
    let new_entries: Vec<&PlaylistEntry> = entries
        .iter()
        .filter(|e| {
            // Skip premieres, live streams, and members-only
            if matches!(e.live_status.as_deref(), Some("is_upcoming" | "is_live")) {
                return false;
            }
            if matches!(e.availability.as_deref(), Some("subscriber_only" | "needs_premium")) {
                return false;
            }
            e.id.as_ref().map_or(true, |id| !existing_ids.contains(id))
        })
        .collect();

    // Set feed artwork from the first entry's thumbnail (once, if not already set)
    if detail.feed.artwork_url.is_none() {
        if let Some(thumb_url) = best_thumbnail(&entries) {
            let state = app.state::<AppState>();
            let body = UpdateFeedRequest { artwork_url: thumb_url };
            let _ = state.api.read().await.request_no_content(
                &format!("/api/v1/feeds/{}", feed.id),
                "PATCH",
                Some(&body),
                true,
            ).await;
        }
    }

    if new_entries.is_empty() {
        emit_progress(app, &feed.id, &feed.name, "done", "Already up to date");
        return Ok(());
    }

    let state = app.state::<AppState>();
    for entry in &new_entries {
        let video_id = match &entry.id {
            Some(id) => id.clone(),
            None => continue,
        };
        let title = entry.title.clone().unwrap_or_else(|| video_id.clone());

        emit_progress(
            app,
            &feed.id,
            &feed.name,
            "create",
            &format!("Creating: {title}"),
        );

        let create_body = CreateEpisodeRequest {
            video_id,
            title: title.clone(),
            description: entry.description.clone(),
            pub_date: format_upload_date(entry.upload_date.as_deref()),
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

    emit_progress(
        app,
        &feed.id,
        &feed.name,
        "done",
        &format!("Found {} new episodes", new_entries.len()),
    );
    Ok(())
}

// ========== Pass 2: Download ==========

async fn run_downloads(app: &AppHandle, feeds: &[Feed], skip_delay: bool) {
    let max_concurrent = cpu_count().clamp(2, 4);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let mut tasks = tokio::task::JoinSet::new();

    for feed in feeds {
        let detail = match fetch_feed_detail(app, &feed.id).await {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Failed to fetch feed {}: {e}", feed.name);
                continue;
            }
        };

        let pending: Vec<Episode> = detail
            .episodes
            .into_iter()
            .filter(|e| e.status == "pending" || e.status == "failed")
            .collect();

        if pending.is_empty() {
            continue;
        }

        let temp_dir = temp_dir_for_feed(&feed.id);
        if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
            log::warn!("Failed to create temp dir: {e}");
            continue;
        }

        for ep in pending {
            // Check if already downloaded locally
            let audio_path = temp_dir.join(format!("{}.m4a", ep.video_id));
            if audio_path.exists() {
                log::info!("Already downloaded locally: {}", ep.title);
                let _ = update_episode_status(app, &ep.id, "uploading", None).await;
                continue;
            }

            let sem = semaphore.clone();
            let app = app.clone();
            let feed = feed.clone();
            tasks.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                if !skip_delay {
                    let delay = rand::thread_rng().gen_range(15..=30);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
                if let Err(e) = download_episode(&app, &feed, &ep).await {
                    log::warn!("Download episode {} failed: {e}", ep.title);
                }
            });
        }
    }

    while let Some(result) = tasks.join_next().await {
        if let Err(e) = result {
            log::warn!("Download task panicked: {e}");
        }
    }
}

async fn download_episode(
    app: &AppHandle,
    feed: &Feed,
    ep: &Episode,
) -> Result<(), AppError> {
    let temp_dir = temp_dir_for_feed(&feed.id);

    // Fetch metadata
    let ep_url = extractor::episode_url(&feed.source_url, &ep.video_id);
    emit_progress(
        app,
        &feed.id,
        &feed.name,
        "metadata",
        &format!("Fetching metadata: {}", ep.title),
    );
    match extractor::fetch_video_metadata(app, &ep_url).await {
        Ok(meta) => {
            let pub_date = format_upload_date(meta.upload_date.as_deref());
            let update_body = UpdateEpisodeMetadataRequest {
                description: meta.description.clone(),
                pub_date,
                duration_sec: meta.duration.map(|d| d as i64),
            };
            let state = app.state::<AppState>();
            let result = state
                .api
                .read()
                .await
                .request_no_content(
                    &format!("/api/v1/episodes/{}", ep.id),
                    "PATCH",
                    Some(&update_body),
                    true,
                )
                .await;
            if let Err(e) = result {
                log::warn!("Failed to update metadata for {}: {e}", ep.title);
            }
        }
        Err(e) => log::warn!("Failed to fetch metadata for {}: {e}", ep.title),
    }

    // Download audio
    emit_progress(
        app,
        &feed.id,
        &feed.name,
        "download",
        &format!("Downloading: {}", ep.title),
    );
    match extractor::extract_audio(app, &ep_url, &ep.video_id, &temp_dir).await {
        Ok(_) => {
            let _ = update_episode_status(app, &ep.id, "uploading", None).await;
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("Premieres in") || err_str.contains("is_upcoming") {
                log::info!("Skipping premiere, will retry later: {}", ep.title);
            } else {
                log::warn!("Download failed for {}: {e}", ep.title);
                let _ = update_episode_status(app, &ep.id, "failed", None).await;
            }
        }
    }

    Ok(())
}

// ========== Pass 3: Upload ==========

async fn run_uploads(app: &AppHandle, feeds: &[Feed]) {
    let max_concurrent = (cpu_count() / 2).max(1);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    let mut tasks = tokio::task::JoinSet::new();

    for feed in feeds {
        let detail = match fetch_feed_detail(app, &feed.id).await {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Failed to fetch feed {}: {e}", feed.name);
                continue;
            }
        };

        let uploading: Vec<Episode> = detail
            .episodes
            .into_iter()
            .filter(|e| e.status == "uploading")
            .collect();

        for ep in uploading {
            let sem = semaphore.clone();
            let app = app.clone();
            let feed = feed.clone();
            tasks.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                if let Err(e) = upload_episode(&app, &feed, &ep).await {
                    log::warn!("Upload episode {} failed: {e}", ep.title);
                }
            });
        }
    }

    while let Some(result) = tasks.join_next().await {
        if let Err(e) = result {
            log::warn!("Upload task panicked: {e}");
        }
    }

    // Clean up empty temp dirs
    for feed in feeds {
        let _ = tokio::fs::remove_dir(temp_dir_for_feed(&feed.id)).await;
    }
}

async fn upload_episode(
    app: &AppHandle,
    feed: &Feed,
    ep: &Episode,
) -> Result<(), AppError> {
    let temp_dir = temp_dir_for_feed(&feed.id);
    let audio_path = temp_dir.join(format!("{}.m4a", ep.video_id));

    if !audio_path.exists() {
        log::warn!("Audio file missing for {}, marking failed", ep.title);
        let _ = update_episode_status(app, &ep.id, "failed", None).await;
        return Ok(());
    }

    let state = app.state::<AppState>();

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
            return Ok(());
        }
    };

    // Upload
    emit_progress(
        app,
        &feed.id,
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
            let _ = update_episode_status(app, &ep.id, "ready", Some(file_size)).await;
            let _ = tokio::fs::remove_file(&audio_path).await;
            emit_progress(
                app,
                &feed.id,
                &feed.name,
                "complete",
                &format!("Done: {}", ep.title),
            );
        }
        Err(e) => {
            log::warn!("Upload failed for {}: {e}", ep.title);
            let _ = update_episode_status(app, &ep.id, "failed", None).await;
        }
    }

    Ok(())
}

// ========== Tauri Commands ==========

#[tauri::command]
pub async fn sync_feed(app: AppHandle, feed_id: String) -> Result<(), AppError> {
    // Check if this feed is within the user's plan limit
    let all_feeds = fetch_all_feeds(&app).await?;
    let plan = get_user_plan(&app).await;
    let allowed_feeds = cap_feeds_for_plan(all_feeds, &plan);
    if !allowed_feeds.iter().any(|f| f.id == feed_id) {
        return Err(AppError::Api("feed exceeds plan limit, upgrade to sync".to_string()));
    }

    let detail = fetch_feed_detail(&app, &feed_id).await?;
    let feeds = vec![detail.feed];

    // Pass 1: Scan (blocking)
    run_scan(&app, &feeds, &SyncMode::Force).await;

    // Pass 2 & 3: Download then Upload (background)
    tokio::spawn(async move {
        run_downloads(&app, &feeds, true).await;
        run_uploads(&app, &feeds).await;
    });

    Ok(())
}

#[tauri::command]
pub async fn start_periodic_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    interval_minutes: u64,
) -> Result<(), AppError> {
    let mut handles = state.sync_handles.lock().await;

    // Abort existing loops
    if let Some(h) = handles.scan.take() {
        h.abort();
    }
    if let Some(h) = handles.download.take() {
        h.abort();
    }
    if let Some(h) = handles.upload.take() {
        h.abort();
    }

    let scan_interval = Duration::from_secs(interval_minutes * 60);
    let download_interval = Duration::from_secs(120);
    let upload_interval = Duration::from_secs(30);

    // Scan loop: wait one full interval before first scan so we don't hit playlists immediately on startup
    let app_scan = app.clone();
    handles.scan = Some(tokio::spawn(async move {
        loop {
            tokio::time::sleep(scan_interval).await;
            let feeds = match fetch_all_feeds(&app_scan).await {
                Ok(f) => f,
                Err(e) => {
                    log::warn!("Periodic scan: failed to fetch feeds: {e}");
                    continue;
                }
            };
            let plan = get_user_plan(&app_scan).await;
            let feeds = cap_feeds_for_plan(feeds, &plan);
            run_scan(&app_scan, &feeds, &SyncMode::Periodic).await;
        }
    }));

    // Download loop
    let app_dl = app.clone();
    handles.download = Some(tokio::spawn(async move {
        loop {
            let feeds = match fetch_all_feeds(&app_dl).await {
                Ok(f) => f,
                Err(e) => {
                    log::warn!("Periodic download: failed to fetch feeds: {e}");
                    tokio::time::sleep(download_interval).await;
                    continue;
                }
            };
            let plan = get_user_plan(&app_dl).await;
            let feeds = cap_feeds_for_plan(feeds, &plan);
            run_downloads(&app_dl, &feeds, false).await;
            tokio::time::sleep(download_interval).await;
        }
    }));

    // Upload loop
    let app_ul = app.clone();
    handles.upload = Some(tokio::spawn(async move {
        loop {
            let feeds = match fetch_all_feeds(&app_ul).await {
                Ok(f) => f,
                Err(e) => {
                    log::warn!("Periodic upload: failed to fetch feeds: {e}");
                    tokio::time::sleep(upload_interval).await;
                    continue;
                }
            };
            let plan = get_user_plan(&app_ul).await;
            let feeds = cap_feeds_for_plan(feeds, &plan);
            run_uploads(&app_ul, &feeds).await;
            tokio::time::sleep(upload_interval).await;
        }
    }));

    Ok(())
}

#[tauri::command]
pub async fn stop_periodic_sync(state: State<'_, AppState>) -> Result<(), AppError> {
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
    Ok(())
}
