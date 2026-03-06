use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::services::api_client::ApiClient;

pub struct SyncHandles {
    pub scan: Option<tokio::task::JoinHandle<()>>,
    pub download: Option<tokio::task::JoinHandle<()>>,
    pub upload: Option<tokio::task::JoinHandle<()>>,
}

pub struct AppState {
    pub api: Arc<RwLock<ApiClient>>,
    pub sync_handles: Arc<Mutex<SyncHandles>>,
}

impl AppState {
    pub fn new(base_url: &str) -> Self {
        Self {
            api: Arc::new(RwLock::new(ApiClient::new(base_url, None))),
            sync_handles: Arc::new(Mutex::new(SyncHandles {
                scan: None,
                download: None,
                upload: None,
            })),
        }
    }
}
