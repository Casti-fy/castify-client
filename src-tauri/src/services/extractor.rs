use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandEvent;

use crate::error::AppError;
use crate::models::PlaylistEntry;

/// Run a sidecar binary and collect stdout/stderr, returning (exit_code, stdout, stderr).
async fn run_sidecar(
    app: &AppHandle,
    name: &str,
    args: Vec<String>,
) -> Result<(i32, String, String), AppError> {
    let cmd = app
        .shell()
        .sidecar(name)
        .map_err(|e| AppError::Other(format!("sidecar {name}: {e}")))?
        .args(&args);

    let (mut rx, _child) = cmd
        .spawn()
        .map_err(|e| AppError::Other(format!("spawn {name}: {e}")))?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code: i32 = -1;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) => {
                stdout.push_str(&String::from_utf8_lossy(&line));
                stdout.push('\n');
            }
            CommandEvent::Stderr(line) => {
                stderr.push_str(&String::from_utf8_lossy(&line));
                stderr.push('\n');
            }
            CommandEvent::Terminated(payload) => {
                exit_code = payload.code.unwrap_or(-1);
            }
            CommandEvent::Error(err) => {
                return Err(AppError::Other(format!("{name} error: {err}")));
            }
            _ => {}
        }
    }

    Ok((exit_code, stdout, stderr))
}

pub async fn fetch_playlist(
    app: &AppHandle,
    url: &str,
    max_items: u32,
) -> Result<Vec<PlaylistEntry>, AppError> {
    let args = vec![
        "--ignore-errors".to_string(),
        "--flat-playlist".to_string(),
        "--dump-json".to_string(),
        "--playlist-end".to_string(),
        max_items.to_string(),
        url.to_string(),
    ];

    let (code, stdout, stderr) = run_sidecar(app, "binaries/yt-dlp", args).await?;

    if code != 0 {
        return Err(AppError::ExtractionFailed(format!(
            "yt-dlp exit code {code}: {stderr}"
        )));
    }

    let entries: Vec<PlaylistEntry> = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    Ok(entries)
}

pub async fn extract_audio(
    app: &AppHandle,
    video_id: &str,
    output_dir: &Path,
) -> Result<PathBuf, AppError> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
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
        "--retries".to_string(),
        "3".to_string(),
        "-x".to_string(),
        "--audio-format".to_string(),
        "m4a".to_string(),
        "--audio-quality".to_string(),
        "0".to_string(),
        "-o".to_string(),
        output_template.to_string_lossy().to_string(),
        url,
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

pub fn build_extract_command(video_id: &str) -> String {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    format!(
        "yt-dlp --no-playlist --retries 3 -x --audio-format m4a --audio-quality 0 -o \"%(id)s.%(ext)s\" \"{url}\""
    )
}
