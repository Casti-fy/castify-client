//! Credential/config storage abstraction for GUI (Tauri store) and CLI (file-based).
//! Trait and file implementation are always compiled; Tauri impl is behind feature "gui".

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persistent store for token and optional server URL.
pub trait CredentialStore {
    fn get_token(&self) -> Result<Option<String>, AppError>;
    fn set_token(&self, token: &str) -> Result<(), AppError>;
    fn delete_token(&self) -> Result<(), AppError>;
    fn get_server_url(&self) -> Result<Option<String>, AppError>;
    fn set_server_url(&self, url: &str) -> Result<(), AppError>;
}

/// File-based store for CLI: ~/.config/castify/config.json (or platform equivalent).
#[derive(Clone)]
pub struct FileStore {
    path: PathBuf,
}

#[derive(Default, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    jwt_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_url: Option<String>,
}

impl FileStore {
    /// Uses dirs::config_dir()/castify/config.json; creates dir if needed.
    pub fn new() -> Result<Self, AppError> {
        let dir = dirs::config_dir()
            .ok_or_else(|| AppError::Keychain("no config directory".into()))?
            .join("castify");
        std::fs::create_dir_all(&dir).map_err(|e| AppError::Keychain(e.to_string()))?;
        Ok(Self {
            path: dir.join("config.json"),
        })
    }

    fn load(&self) -> Result<ConfigFile, AppError> {
        let data = match std::fs::read_to_string(&self.path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ConfigFile::default());
            }
            Err(e) => return Err(AppError::Keychain(e.to_string())),
        };
        serde_json::from_str(&data).map_err(|e| AppError::Keychain(e.to_string()))
    }

    fn save(&self, config: &ConfigFile) -> Result<(), AppError> {
        let data = serde_json::to_string_pretty(config).map_err(|e| AppError::Keychain(e.to_string()))?;
        std::fs::write(&self.path, data).map_err(|e| AppError::Keychain(e.to_string()))
    }
}

impl CredentialStore for FileStore {
    fn get_token(&self) -> Result<Option<String>, AppError> {
        Ok(self.load()?.jwt_token)
    }

    fn set_token(&self, token: &str) -> Result<(), AppError> {
        let mut config = self.load()?;
        config.jwt_token = Some(token.to_string());
        self.save(&config)
    }

    fn delete_token(&self) -> Result<(), AppError> {
        let mut config = self.load()?;
        config.jwt_token = None;
        self.save(&config)
    }

    fn get_server_url(&self) -> Result<Option<String>, AppError> {
        Ok(self.load()?.server_url)
    }

    fn set_server_url(&self, url: &str) -> Result<(), AppError> {
        let mut config = self.load()?;
        config.server_url = Some(url.to_string());
        self.save(&config)
    }
}
