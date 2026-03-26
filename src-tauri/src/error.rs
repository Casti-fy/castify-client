use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("API error: {0}")]
    Api(String),

    #[error("HTTP {0}")]
    Status(u16),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Network error: {0}")]
    Network(String),

    #[error("yt-dlp not found")]
    YtdlpNotFound,

    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),

    #[error("Output file not found")]
    OutputNotFound,

    #[error("Upload failed: HTTP {0}")]
    UploadFailed(u16),

    #[error("Keychain error: {0}")]
    Keychain(String),

    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Other(e.to_string())
    }
}
