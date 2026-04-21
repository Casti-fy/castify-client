use crate::models::SyncProgressEvent;
use crate::state::AppState;

pub fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

pub fn emit_progress(
    state: &AppState,
    feed_id: &str,
    feed_name: &str,
    step: &str,
    message: &str,
) {
    if let Some(emitter) = state.on_progress.get() {
        emitter(SyncProgressEvent {
            feed_id: feed_id.to_string(),
            feed_name: feed_name.to_string(),
            step: step.to_string(),
            message: message.to_string(),
        });
    }
}

pub fn temp_dir_for_feed(feed_id: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("castify-{feed_id}"))
}

/// Remove all `castify-*` directories under the system temp dir.
/// Used on startup to clear leftover audio from prior runs, and by
/// the `clear_sync_cache` command.
pub fn clear_all_temp_dirs() {
    let tmp = std::env::temp_dir();
    let entries = match std::fs::read_dir(&tmp) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("Failed to read temp dir {}: {e}", tmp.display());
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("castify-"))
        {
            if let Err(e) = std::fs::remove_dir_all(&path) {
                log::warn!("Failed to remove temp dir {}: {e}", path.display());
            }
        }
    }
}
