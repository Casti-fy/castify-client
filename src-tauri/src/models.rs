use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanLimits {
    pub max_feeds: i64,
    pub max_episodes_per_feed: i64,
    pub retention_days: i64,
    pub max_file_size: i64,
    pub max_total_file_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub plan: String,
    pub limits: PlanLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub id: String,
    pub name: String,
    #[serde(rename = "source_url")]
    pub source_url: String,
    pub description: Option<String>,
    #[serde(rename = "artwork_url")]
    pub artwork_url: Option<String>,
    #[serde(rename = "feed_slug")]
    pub feed_slug: String,
    #[serde(rename = "feed_url", default)]
    pub feed_url: String,
    #[serde(rename = "episode_count", default)]
    pub episode_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    #[serde(rename = "feed_id")]
    pub feed_id: String,
    #[serde(rename = "video_id")]
    pub video_id: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "pub_date")]
    pub pub_date: Option<String>,
    #[serde(rename = "duration_sec")]
    pub duration_sec: Option<i64>,
    pub status: String,
}

// -- Request types --

#[derive(Debug, Serialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct CreateFeedRequest {
    pub name: String,
    #[serde(rename = "source_url")]
    pub source_url: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateEpisodeRequest {
    #[serde(rename = "video_id")]
    pub video_id: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "pub_date")]
    pub pub_date: Option<String>,
    #[serde(rename = "duration_sec")]
    pub duration_sec: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct UpdateEpisodeRequest {
    pub status: String,
    #[serde(rename = "file_size", skip_serializing_if = "Option::is_none")]
    pub file_size: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct UpdateFeedRequest {
    #[serde(rename = "artwork_url", skip_serializing_if = "Option::is_none")]
    pub artwork_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateEpisodeMetadataRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "pub_date", skip_serializing_if = "Option::is_none")]
    pub pub_date: Option<String>,
    #[serde(rename = "duration_sec", skip_serializing_if = "Option::is_none")]
    pub duration_sec: Option<i64>,
}

// -- Response types --

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateFeedResponse {
    pub feed: Feed,
    #[serde(rename = "feed_url")]
    pub feed_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateEpisodeResponse {
    pub episode: Episode,
    #[serde(rename = "upload_url")]
    pub upload_url: String,
    #[serde(rename = "authorization_token")]
    pub authorization_token: String,
    #[serde(rename = "file_name")]
    pub file_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FeedDetailResponse {
    pub feed: Feed,
    pub episodes: Vec<Episode>,
    #[serde(rename = "feed_url")]
    pub feed_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UploadURLResponse {
    #[serde(rename = "upload_url")]
    pub upload_url: String,
    #[serde(rename = "authorization_token")]
    pub authorization_token: String,
    #[serde(rename = "file_name")]
    pub file_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

// Typed scaffold, schema for JSON dictionary comming from yt-dlp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistEntry {
    pub url: Option<String>,
    pub id: Option<String>,
    pub title: Option<String>,
    pub timestamp: Option<i64>, // sound cloud
    pub release_timestamp: Option<i64>, // youtube, premier
    pub live_status: Option<String>, // youtube, live
    pub availability: Option<String>, // youtube, subscriber_only, needs_premium
    pub duration: Option<f64>,
    pub description: Option<String>,
    pub extractor: Option<String>,
}

impl PlaylistEntry {
    pub fn pub_date(&self) -> Option<String> {
        // Choose source-specific timestamp (YouTube: release_timestamp, SoundCloud: timestamp)
        let ts = if self.extractor.as_deref() == Some("youtube") {
            self.release_timestamp
        } else if self.extractor.as_deref() == Some("soundcloud") {
            self.timestamp
        } else {
            None
        }?;

        // Derive UTC datetime from Unix timestamp using chrono
        use chrono::{TimeZone, Utc};
        let dt = Utc.timestamp_opt(ts, 0).single()?;
        Some(dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
    }
}

// -- Event payloads --

#[derive(Debug, Clone, Serialize)]
pub struct SyncProgressEvent {
    pub feed_id: String,
    pub feed_name: String,
    pub step: String,
    pub message: String,
}
