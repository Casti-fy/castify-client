use std::collections::HashSet;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::models::{CreateEpisodeRequest, Feed, PlaylistEntry};
use crate::services::{episode as episode_service, extractor, feeds as feeds_service, helpers};
use crate::state::{AppState, Job, Priority};

const SCAN_FEED_SPACING: Duration = Duration::from_secs(2);

pub fn format_pub_date(upload_date: Option<&str>, timestamp: Option<i64>) -> Option<String> {
    fn epoch_days_to_ymd(days: i32) -> (i32, u32, u32) {
        let z = days + 719468;
        let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
        let doe = (z - era * 146097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe as i32 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (if m <= 2 { y + 1 } else { y }, m, d)
    }

    if let Some(d) = upload_date {
        if d.len() == 8 {
            return Some(format!(
                "{}-{}-{}T00:00:00Z",
                &d[..4],
                &d[4..6],
                &d[6..8]
            ));
        }
    }
    timestamp.and_then(|ts| {
        let days_since_epoch = (ts / 86400) as i32;
        let date = epoch_days_to_ymd(days_since_epoch);
        let s = format!("{:04}{:02}{:02}", date.0, date.1, date.2);
        format_pub_date(Some(&s), None)
    })
}

pub async fn run_scan(app: &AppHandle, feeds: &[Feed], max_items: u32, priority: Priority) {
    for (i, feed) in feeds.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(SCAN_FEED_SPACING).await;
        }
        if let Err(e) = scan_feed(app, feed, max_items, priority).await {
            log::warn!("Scan feed {} failed: {e}", feed.name);
        }
    }
}

async fn scan_feed(
    app: &AppHandle,
    feed: &Feed,
    max_items: u32,
    priority: Priority,
) -> Result<(), AppError> {
    helpers::emit_progress(app, &feed.id, &feed.name, "fetch", "Fetching playlist...");

    let detail = feeds_service::fetch_feed_detail(app, &feed.id).await?;

    let existing_ids: HashSet<String> =
        detail.episodes.iter().map(|e| e.video_id.clone()).collect();

    let entries = extractor::fetch_playlist(app, &feed.source_url, max_items).await?;
    let new_entries: Vec<&PlaylistEntry> = entries
        .iter()
        .filter(|e| e.id.as_ref().map_or(true, |id| !existing_ids.contains(id)))
        .collect();

    if new_entries.is_empty() {
        helpers::emit_progress(app, &feed.id, &feed.name, "done", "Already up to date");
        return Ok(());
    }

    for entry in &new_entries {
        let video_id = match &entry.id {
            Some(id) => id.clone(),
            None => continue,
        };
        let title = entry.title.clone().unwrap_or_else(|| video_id.clone());

        helpers::emit_progress(
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
            pub_date: format_pub_date(entry.upload_date.as_deref(), entry.timestamp),
            duration_sec: entry.duration.map(|d| d as i64),
        };

        match episode_service::create_episode(app, &feed.id, &create_body).await {
            Ok(resp) => {
                let state = app.state::<AppState>();
                let episode_id = resp.episode.id;
                let episode_title = resp.episode.title.clone();
                let video_id = resp.episode.video_id.clone();
                let episode_url = extractor::episode_url(&feed.source_url, &video_id);

                state
                    .sync_channels
                    .send_download(Job {
                        feed_id: feed.id.clone(),
                        feed_name: feed.name.clone(),
                        episode_id,
                        episode_title,
                        video_id,
                        episode_url,
                        priority,
                    })
                    .await;
            }
            Err(e) => {
                log::warn!("Failed to create episode {title}: {e}");
            }
        }
    }

    helpers::emit_progress(
        app,
        &feed.id,
        &feed.name,
        "done",
        &format!("Found {} new episodes", new_entries.len()),
    );
    Ok(())
}

