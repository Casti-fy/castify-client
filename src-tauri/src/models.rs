use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub id: String,
    pub name: String,
    #[serde(rename = "source_url")]
    pub source_url: String,
    pub description: Option<String>,
    #[serde(rename = "feed_slug")]
    pub feed_slug: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistEntry {
    pub id: Option<String>,
    pub title: Option<String>,
    pub upload_date: Option<String>,
    pub duration: Option<f64>,
    pub description: Option<String>,
}

// -- Event payloads --

#[derive(Debug, Clone, Serialize)]
pub struct SyncProgressEvent {
    pub feed_id: String,
    pub feed_name: String,
    pub step: String,
    pub message: String,
}
