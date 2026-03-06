use std::path::{Path, PathBuf};
use std::process::Stdio;

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::models::PlaylistEntry;

/// Resolve the absolute path of a sidecar binary.
fn resolve_sidecar(app: &AppHandle, name: &str) -> Result<PathBuf, AppError> {
    // In dev mode, sidecars are next to the built binary in target/debug/
    // In production, they are in the resource dir
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .ok_or_else(|| AppError::Other("cannot resolve exe dir".into()))?;

    let base_name = Path::new(name).file_name().unwrap_or(name.as_ref());
    let exe_name = if cfg!(windows) {
        format!("{}.exe", base_name.to_string_lossy())
    } else {
        base_name.to_string_lossy().to_string()
    };

    // Try exe dir first (where Tauri places sidecars)
    let candidate = exe_dir.join(&exe_name);
    if candidate.exists() {
        return Ok(candidate);
    }

    // Try resource dir
    let resource_dir = app.path().resource_dir().unwrap_or_default();
    let candidate = resource_dir.join(&exe_name);
    if candidate.exists() {
        return Ok(candidate);
    }

    Err(AppError::Other(format!("sidecar not found: {exe_name}")))
}

/// Run a sidecar binary and collect stdout/stderr, returning (exit_code, stdout, stderr).
async fn run_sidecar(
    app: &AppHandle,
    name: &str,
    args: Vec<String>,
) -> Result<(i32, String, String), AppError> {
    let bin_path = resolve_sidecar(app, name)?;
    let bin_name = bin_path.file_name().unwrap_or(name.as_ref()).to_string_lossy();
    log::info!("[sidecar] {} {}", bin_name, args.join(" "));

    let output = tokio::process::Command::new(&bin_path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| AppError::Other(format!("spawn {bin_name}: {e}")))?;

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if exit_code != 0 {
        log::error!("[sidecar] {} exited with code {}\nstderr: {}", bin_name, exit_code, stderr.trim());
    } else {
        log::info!("[sidecar] {} exited with code 0\nstdout: {}\nstderr: {}", bin_name, stdout.trim(), stderr.trim());
    }

    Ok((exit_code, stdout, stderr))
}

/// Pass 1: Fetch playlist entries. Uses flat-playlist for YouTube (fast, metadata
/// comes in pass 2) and full playlist for SoundCloud (flat mode lacks titles).
pub async fn fetch_playlist(
    app: &AppHandle,
    url: &str,
    max_items: u32,
) -> Result<Vec<PlaylistEntry>, AppError> {
    let is_soundcloud = url.contains("soundcloud.com");

    let mut args = vec!["--ignore-errors".to_string()];

    if !is_soundcloud {
        // YouTube: flat-playlist is fast, metadata fetched later in pass 2
        args.push("--flat-playlist".to_string());
        args.extend([
            "--match-filter".to_string(),
            "original_url!*=/shorts/ & live_status!=is_upcoming & live_status!=is_live & availability!=subscriber_only & availability!=needs_premium".to_string(),
        ]);
    }

    args.extend([
        "--dump-json".to_string(),
        "--playlist-end".to_string(),
        max_items.to_string(),
        url.to_string(),
    ]);

    let (code, stdout, stderr) = run_sidecar(app, "binaries/yt-dlp", args).await?;

    if code != 0 {
        return Err(AppError::ExtractionFailed(format!(
            "yt-dlp exit code {code}: {stderr}"
        )));
    }

    let entries: Vec<PlaylistEntry> = parse_playlist_lines(&stdout);
    log::info!("[fetch_playlist] got {} entries (soundcloud={})", entries.len(), is_soundcloud);
    Ok(entries)
}

/// Construct the direct URL for a single episode given the feed's source URL
/// and the episode's ID (as returned by yt-dlp in flat-playlist mode).
pub fn episode_url(feed_source_url: &str, video_id: &str) -> String {
    if feed_source_url.contains("soundcloud.com") {
        format!("https://api.soundcloud.com/tracks/{video_id}")
    } else {
        // Default to YouTube
        format!("https://www.youtube.com/watch?v={video_id}")
    }
}

/// Pass 2: Fetch full metadata for a single episode by URL.
pub async fn fetch_video_metadata(
    app: &AppHandle,
    url: &str,
) -> Result<PlaylistEntry, AppError> {
    let args = vec![
        "--ignore-errors".to_string(),
        "--dump-json".to_string(),
        "--skip-download".to_string(),
        "--no-playlist".to_string(),
        url.to_string(),
    ];

    let (code, stdout, stderr) = run_sidecar(app, "binaries/yt-dlp", args).await?;

    if code != 0 {
        return Err(AppError::ExtractionFailed(format!(
            "yt-dlp exit code {code}: {stderr}"
        )));
    }

    let line = stdout.lines().find(|l| !l.is_empty()).unwrap_or("");
    serde_json::from_str(line)
        .map_err(|e| AppError::Other(format!("parse video metadata: {e}")))
}

fn parse_playlist_lines(stdout: &str) -> Vec<PlaylistEntry> {
    stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            serde_json::from_str(line)
                .map_err(|e| log::warn!("[fetch_playlist] failed to parse line: {e}"))
                .ok()
        })
        .collect()
}

pub async fn extract_audio(
    app: &AppHandle,
    url: &str,
    video_id: &str,
    output_dir: &Path,
) -> Result<PathBuf, AppError> {
    let output_template = output_dir.join("%(id)s.%(ext)s");

    // Build yt-dlp args
    let mut args: Vec<String> = Vec::new();

    // Resolve ffmpeg sidecar path for --ffmpeg-location
    // Tauri sidecars are placed next to the app binary at runtime
    let resource_dir: PathBuf = app.path().resource_dir().unwrap_or_default();
    let ffmpeg_candidate = resource_dir.join("binaries").join("ffmpeg");
    if ffmpeg_candidate.exists() {
        if let Some(dir) = ffmpeg_candidate.parent() {
            args.push("--ffmpeg-location".to_string());
            args.push(dir.to_string_lossy().to_string());
        }
    }

    args.extend([
        "--no-playlist".to_string(),
        "--match-filter".to_string(),
        "live_status!=is_upcoming & live_status!=is_live".to_string(),
        "--retries".to_string(),
        "3".to_string(),
        "-x".to_string(),
        "--audio-format".to_string(),
        "m4a".to_string(),
        "--audio-quality".to_string(),
        "0".to_string(),
        "-o".to_string(),
        output_template.to_string_lossy().to_string(),
        url.to_string(),
    ]);

    let (code, _stdout, stderr) = run_sidecar(app, "binaries/yt-dlp", args).await?;

    if code != 0 {
        return Err(AppError::ExtractionFailed(stderr));
    }

    let audio_path = output_dir.join(format!("{video_id}.m4a"));
    if !audio_path.exists() {
        return Err(AppError::OutputNotFound);
    }

    Ok(audio_path)
}
