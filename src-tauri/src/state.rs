use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::services::api_client::ApiClient;

pub struct AppState {
    pub api: Arc<RwLock<ApiClient>>,
    pub sync_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl AppState {
    pub fn new(base_url: &str) -> Self {
        let token = crate::services::keychain::get_token().ok();
        Self {
            api: Arc::new(RwLock::new(ApiClient::new(base_url, token))),
            sync_handle: Arc::new(Mutex::new(None)),
        }
    }
}
