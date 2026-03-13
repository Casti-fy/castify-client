use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::services::api_client::ApiClient;

// ── Priority & Job ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Urgent, // user-initiated: create_feed, sync_feed
    High,   // periodic scan discovers new episode
    Normal, // startup recovery from API
}

#[derive(Debug, Clone)]
pub struct Job {
    pub feed_id: String,
    pub episode_id: String,
    pub priority: Priority,
}

// ── Channel Types ───────────────────────────────────────────────────────────

/// Sender-side of the three priority channels. Stored in AppState (cloneable).
#[derive(Clone)]
pub struct ChannelSenders {
    pub urgent_tx: mpsc::Sender<Job>,
    pub high_tx: mpsc::Sender<Job>,
    pub normal_tx: mpsc::Sender<Job>,
}

impl ChannelSenders {
    /// Send a job to the channel matching its priority.
    pub async fn send(&self, job: Job) {
        let result = match job.priority {
            Priority::Urgent => self.urgent_tx.send(job).await,
            Priority::High => self.high_tx.send(job).await,
            Priority::Normal => self.normal_tx.send(job).await,
        };
        if let Err(e) = result {
            log::warn!("Failed to send job to channel: {e}");
        }
    }
}

/// Receiver-side of the three priority channels. Moved into the worker task.
pub struct ChannelReceivers {
    pub urgent_rx: mpsc::Receiver<Job>,
    pub high_rx: mpsc::Receiver<Job>,
    pub normal_rx: mpsc::Receiver<Job>,
}

/// Create a matched pair of senders + receivers for one worker.
pub fn create_worker_channels() -> (ChannelSenders, ChannelReceivers) {
    let (urgent_tx, urgent_rx) = mpsc::channel(64);
    let (high_tx, high_rx) = mpsc::channel(64);
    let (normal_tx, normal_rx) = mpsc::channel(256);
    (
        ChannelSenders {
            urgent_tx,
            high_tx,
            normal_tx,
        },
        ChannelReceivers {
            urgent_rx,
            high_rx,
            normal_rx,
        },
    )
}

// ── Sync Handles ────────────────────────────────────────────────────────────

pub struct SyncHandles {
    pub scan: Option<tokio::task::JoinHandle<()>>,
    pub download: Option<tokio::task::JoinHandle<()>>,
    pub upload: Option<tokio::task::JoinHandle<()>>,
}

// ── Sync Channels ───────────────────────────────────────────────────────────

/// Holds both sender and receiver sides for download and upload workers.
/// Senders are behind RwLock so they can be replaced on stop/restart.
/// Receivers are behind Mutex<Option<>> — taken once when workers start.
pub struct SyncChannels {
    pub download_tx: RwLock<ChannelSenders>,
    pub upload_tx: RwLock<ChannelSenders>,
    pub download_rx: Mutex<Option<ChannelReceivers>>,
    pub upload_rx: Mutex<Option<ChannelReceivers>>,
}

impl SyncChannels {
    pub fn new() -> Self {
        let (dl_tx, dl_rx) = create_worker_channels();
        let (ul_tx, ul_rx) = create_worker_channels();
        Self {
            download_tx: RwLock::new(dl_tx),
            upload_tx: RwLock::new(ul_tx),
            download_rx: Mutex::new(Some(dl_rx)),
            upload_rx: Mutex::new(Some(ul_rx)),
        }
    }

    /// Replace all channels with fresh ones. Called on stop so restart works.
    pub async fn reset(&self) {
        let (dl_tx, dl_rx) = create_worker_channels();
        let (ul_tx, ul_rx) = create_worker_channels();
        *self.download_tx.write().await = dl_tx;
        *self.upload_tx.write().await = ul_tx;
        *self.download_rx.lock().await = Some(dl_rx);
        *self.upload_rx.lock().await = Some(ul_rx);
    }

    /// Send a job to the download channel matching its priority.
    pub async fn send_download(&self, job: Job) {
        self.download_tx.read().await.send(job).await;
    }

    /// Send a job to the upload channel matching its priority.
    pub async fn send_upload(&self, job: Job) {
        self.upload_tx.read().await.send(job).await;
    }
}

// ── App State ───────────────────────────────────────────────────────────────

pub struct AppState {
    pub api: Arc<RwLock<ApiClient>>,
    pub sync_handles: Arc<Mutex<SyncHandles>>,
    pub sync_channels: Arc<SyncChannels>,
    pub cached_limits: Arc<RwLock<Option<crate::models::PlanLimits>>>,
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
            sync_channels: Arc::new(SyncChannels::new()),
            cached_limits: Arc::new(RwLock::new(None)),
        }
    }
}
