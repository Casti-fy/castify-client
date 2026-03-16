use tauri::{AppHandle, Emitter};

pub fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

pub fn emit_progress(
    app: &AppHandle,
    feed_id: &str,
    feed_name: &str,
    step: &str,
    message: &str,
) {
    let _ = app.emit(
        "sync-progress",
        crate::models::SyncProgressEvent {
            feed_id: feed_id.to_string(),
            feed_name: feed_name.to_string(),
            step: step.to_string(),
            message: message.to_string(),
        },
    );
}

pub fn temp_dir_for_feed(feed_id: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("castify-{feed_id}"))
}
