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
    log::info!("[fetch_playlist] got {} entries", entries.len());
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

pub async fn fetch_channel_artwork_url(
    app: &AppHandle,
    url: &str,
) -> Result<Option<String>, AppError> {
    // yt-dlp --flat-playlist --dump-single-json --playlist-items 0 https://www.youtube.com/@TuanTienTi2911
    let args = vec![
        "--flat-playlist".to_string(),
        "--dump-single-json".to_string(),
        "--playlist-items".to_string(),
        "0".to_string(),
        url.to_string(),
    ];

    let (code, stdout, stderr) = run_sidecar(app, "binaries/yt-dlp", args).await?;

    if code != 0 {
        log::warn!("[fetch_channel_artwork_url] yt-dlp exit code {code}: {stderr}");
        return Ok(None);
    }

    // Parse the JSON and extract the highest-resolution thumbnail URL
    let json: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| AppError::Other(format!("parse channel json: {e}")))?;

    let artwork_url = json
        .get("thumbnails")
        .and_then(|t| t.as_array())
        .and_then(|arr| arr.first())
        .and_then(|t| t.get("url"))
        .and_then(|u| u.as_str())
        .map(|s| s.to_string());

    log::info!("[fetch_channel_artwork_url] artwork_url: {:?}", artwork_url);
    Ok(artwork_url)
}

pub async fn download_audio(
    app: &AppHandle,
    url: &str,
    video_id: &str,
    output_dir: &Path,
) -> Result<PathBuf, AppError> {
    // ~24MB per 1h podcast
    // yt-dlp --no-playlist --retries 3 -x --audio-format m4a --audio-quality 0 -o "%(id)s.%(ext)s" https://www.youtube.com/watch?v=TjUhXbGdLYo
    // yt-dlp --match-filter "live_status!=is_upcoming & live_status!=is_live" --no-playlist --retries 3 -x --audio-format m4a --audio-quality 0 -o "%(id)s.%(ext)s" https://www.youtube.com/watch?v=1oG0ru5S4Qw
    // https://api.soundcloud.com/tracks/1212266641
    // --ffmpeg-location "<resource_dir>/binaries" # system may not have
    // --match-filter "live_status!=is_upcoming & live_status!=is_live" # youtube, need?
    let audio_path = output_dir.join(format!("{video_id}.m4a"));
    if audio_path.exists() {
        return Ok(audio_path);
    }

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

    args.push("--no-playlist".to_string());

    // YouTube-specific filter: skip premieres and live streams.
    if url.contains("youtube.com") {
        args.extend([
            "--match-filter".to_string(),
            "live_status!=is_upcoming & live_status!=is_live".to_string(),
        ]);
    }

    let output_template = output_dir.join("%(id)s.tmp.%(ext)s");
    args.extend([
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

    let temp_path = output_dir.join(format!("{video_id}.tmp.m4a"));
    if !temp_path.exists() {
        return Err(AppError::OutputNotFound);
    }

    // ffmpeg -i <video_id>.tmp.m4a -ac 1 -b:a 48k -y <video_id>.m4a
    // Re-encode to mono 32k and save as final <video_id>.m4a
    let audio_path = output_dir.join(format!("{video_id}.m4a"));
    let ffmpeg_args = vec![
        "-i".to_string(),
        temp_path.to_string_lossy().to_string(),
        "-ac".to_string(),
        "1".to_string(),
        "-b:a".to_string(),
        "48k".to_string(),
        "-y".to_string(),
        audio_path.to_string_lossy().to_string(),
    ];
    let (code, _stdout, stderr) = run_sidecar(app, "binaries/ffmpeg", ffmpeg_args).await?;
    if code != 0 {
        let _ = std::fs::remove_file(&temp_path);
        return Err(AppError::ExtractionFailed(stderr));
    }
    let _ = std::fs::remove_file(&temp_path);

    Ok(audio_path)
}

