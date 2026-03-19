use std::path::{Path, PathBuf};
use std::process::Stdio;

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::models::PlaylistEntry;

fn prepend_sidecar_deno_to_path(app: &AppHandle) -> Option<(String, String)> {
    // yt-dlp may use Deno for YouTube n-challenge solving.
    // We always ship `deno` as a sidecar, so prefer that deterministic runtime.
    let path_sep = if cfg!(windows) { ';' } else { ':' };
    let current_path = std::env::var("PATH").unwrap_or_default();

    // Sidecars live next to the executable in dev, and under resource dir in production.
    let mut sidecar_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            sidecar_dirs.push(dir.to_path_buf());
        }
    }
    let resource_dir = app.path().resource_dir().unwrap_or_default();
    if !resource_dir.as_os_str().is_empty() {
        sidecar_dirs.push(resource_dir);
    }

    let target_triple = option_env!("TAURI_ENV_TARGET_TRIPLE").unwrap_or("");
    let deno_name = if cfg!(windows) { "deno.exe" } else { "deno" };
    let deno_name_suffixed = if target_triple.is_empty() {
        None
    } else if cfg!(windows) {
        Some(format!("deno-{target_triple}.exe"))
    } else {
        Some(format!("deno-{target_triple}"))
    };

    let deno_dir = sidecar_dirs
        .into_iter()
        .find(|dir| {
            if !dir.is_dir() {
                return false;
            }
            if dir.join(deno_name).is_file() {
                return true;
            }
            deno_name_suffixed
                .as_deref()
                .is_some_and(|n| dir.join(n).is_file())
        })?;

    let deno_dir_str = deno_dir.to_string_lossy().to_string();
    if current_path.split(path_sep).any(|p| p == deno_dir_str) {
        return None;
    }

    Some((
        "PATH".to_string(),
        format!("{deno_dir_str}{path_sep}{current_path}"),
    ))
}

fn maybe_deno_js_runtime_arg(app: &AppHandle) -> Option<String> {
    // Prefer explicitly wiring Deno to yt-dlp so it can be used even if PATH resolution differs
    // between dev/prod packaging.
    let deno_path = resolve_sidecar(app, "binaries/deno").ok()?;
    Some(format!("deno:{}", deno_path.to_string_lossy()))
}

fn maybe_ffmpeg_location_args(app: &AppHandle) -> Vec<String> {
    // Always point yt-dlp at the bundled ffmpeg directory so its postprocessors work
    // even when system ffmpeg isn't on PATH.
    let Ok(ffmpeg_path) = resolve_sidecar(app, "binaries/ffmpeg") else {
        return Vec::new();
    };
    let Some(dir) = ffmpeg_path.parent() else {
        return Vec::new();
    };
    vec![
        "--ffmpeg-location".to_string(),
        dir.to_string_lossy().to_string(),
    ]
}

fn ytdlp_base_args(app: &AppHandle) -> Vec<String> {
    let mut args = Vec::new();
    args.extend(maybe_ffmpeg_location_args(app));

    // Download the EJS challenge solver so YouTube serves real formats.
    args.push("--remote-components".to_string());
    args.push("ejs:github".to_string());

    // Explicitly wire the bundled Deno runtime to yt-dlp's JS runtime system.
    if let Some(runtime) = maybe_deno_js_runtime_arg(app) {
        args.push("--js-runtimes".to_string());
        args.push(runtime);
    }

    args
}

/// Resolve the absolute path of a sidecar binary.
fn resolve_sidecar(app: &AppHandle, name: &str) -> Result<PathBuf, AppError> {
    // In dev mode, sidecars are next to the built binary in target/debug/
    // In production, they are in the resource dir
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .ok_or_else(|| AppError::Other("cannot resolve exe dir".into()))?;

    let base_name = Path::new(name).file_name().unwrap_or(name.as_ref());
    let base_name = base_name.to_string_lossy().to_string();
    let target_triple = option_env!("TAURI_ENV_TARGET_TRIPLE").unwrap_or("");

    // Tauri sidecars are commonly shipped as `<name>-<target_triple>` (and `.exe` on Windows)
    // but in dev they may also be present as `<name>`.
    let mut candidates: Vec<String> = Vec::new();
    if cfg!(windows) {
        candidates.push(format!("{base_name}.exe"));
        if !target_triple.is_empty() {
            candidates.push(format!("{base_name}-{target_triple}.exe"));
        }
    } else {
        candidates.push(base_name.clone());
        if !target_triple.is_empty() {
            candidates.push(format!("{base_name}-{target_triple}"));
        }
    }

    // Try exe dir first (where Tauri places sidecars in dev)
    for exe_name in &candidates {
        let candidate = exe_dir.join(exe_name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    // Try resource dir
    let resource_dir = app.path().resource_dir().unwrap_or_default();
    for exe_name in &candidates {
        let candidate = resource_dir.join(exe_name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(AppError::Other(format!(
        "sidecar not found: tried {}",
        candidates.join(", ")
    )))
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

    let mut cmd = tokio::process::Command::new(&bin_path);
    cmd.args(&args);

    if let Some((k, v)) = prepend_sidecar_deno_to_path(app) {
        cmd.env(k, v);
    }

    let output = cmd
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
    // yt-dlp --flat-playlist --dump-json --playlist-end 10 https://www.youtube.com/@TuanTienTi2911
    // yt-dlp --flat-playlist --dump-json --playlist-end 10 https://www.youtube.com/@dinhcuhanoi # likely have premier
    // yt-dlp --flat-playlist --dump-json --playlist-end 10 https://www.youtube.com/@VTCNewstintuc # likely have live

    let mut args = vec!["--ignore-errors".to_string()];
    args.extend(ytdlp_base_args(app));

    args.extend([
        "--flat-playlist".to_string(), // less error
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

    let mut entries: Vec<PlaylistEntry> = parse_playlist_lines(&stdout);
    // filter out YouTube Shorts, live streams, and non-public availability
    entries.retain(|entry| {
        !entry.url.as_deref().unwrap_or("").contains("/shorts/")
            && entry.live_status.as_deref() != Some("is_live")
            && entry.availability.as_deref().map(|a| a == "public").unwrap_or(true)
    });

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
    // yt-dlp --dump-json --skip-download --no-playlist https://www.youtube.com/watch?v=TjUhXbGdLYo
    let mut args = vec!["--ignore-errors".to_string()];
    args.extend(ytdlp_base_args(app));
    args.extend([
        "--dump-json".to_string(),
        "--skip-download".to_string(),
        "--no-playlist".to_string(),
        url.to_string(),
    ]);

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
    let mut args = vec!["--flat-playlist".to_string()];
    args.extend(ytdlp_base_args(app));
    args.extend([
        "--dump-single-json".to_string(),
        "--playlist-items".to_string(),
        "0".to_string(),
        url.to_string(),
    ]);

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
        .and_then(|arr| {
            arr.iter()
                .find(|t| t.get("id").and_then(|id| id.as_str()) == Some("avatar_uncropped"))
        })
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
    let temp_path = output_dir.join(format!("{video_id}.tmp.m4a"));

    if audio_path.exists() {
        return Ok(audio_path);
    }

    let find_tmp_input = || -> Option<PathBuf> {
        if temp_path.exists() {
            return Some(temp_path.clone());
        }
        let rd = std::fs::read_dir(output_dir).ok()?;
        for entry in rd.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.starts_with(&format!("{video_id}.tmp."))
                && !name.ends_with(".part")
                && !name.ends_with(".ytdl")
            {
                return Some(p);
            }
        }
        None
    };

    // If a previous run already left a temp download, skip yt-dlp and just extract audio.
    let mut tmp_input = find_tmp_input();

    if tmp_input.is_none() && !temp_path.exists() {
        // Common yt-dlp args (independent of YouTube client selection)
        let mut common_args: Vec<String> = Vec::new();


        // Download the EJS challenge solver so YouTube serves real formats.
        common_args.push("--remote-components".to_string());
        common_args.push("ejs:github".to_string());

        // Explicitly wire the bundled Deno runtime to yt-dlp's JS runtime system.
        if let Some(runtime) = maybe_deno_js_runtime_arg(app) {
            common_args.push("--js-runtimes".to_string());
            common_args.push(runtime);
        }

        common_args.push("--no-playlist".to_string());

        let output_template = output_dir.join("%(id)s.tmp.%(ext)s");
        let base_download_args = vec![
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
        ];

        let needs_cookies = |stderr: &str| {
            stderr.contains("Sign in")
                || stderr.contains("bot")
                || stderr.contains("HTTP Error 429")
                || stderr.contains("Login required")
        };

        let needs_soundcloud_proxy = |stderr: &str| {
            stderr.contains("[soundcloud]")
                && stderr.contains("not available from your location")
                && stderr.contains("geo restriction")
        };

        let soundcloud_proxy = "http://45f8da4c3a:mIebKzA1@207.182.30.55:4444";

        let needs_fallback = |stderr: &str| {
            stderr.contains("Only images are available for download")
                || stderr.contains("n challenge solving failed")
                || stderr.contains("Some formats may be missing")
                || stderr.contains("missing a url")
                || stderr.contains("Requested format is not available")
        };

        let is_android_po_token_block = |stderr: &str| {
            stderr.contains("android client https formats require a GVS PO Token")
        };

        // Try clients in order. Avoid forcing android by default because some videos
        // require PO tokens and will fail.
        let client_modes: [Option<&str>; 3] = [
            None,
            Some("youtube:player_client=web"),
            Some("youtube:player_client=android"),
        ];

        let mut last_stderr = String::new();
        let mut success = false;

        for client in client_modes {
            // Skip android if we already saw PO-token blocking.
            if client == Some("youtube:player_client=android") && is_android_po_token_block(&last_stderr)
            {
                continue;
            }

            let mut attempt_args = common_args.clone();
            if let Some(client_arg) = client {
                attempt_args.push("--extractor-args".to_string());
                attempt_args.push(client_arg.to_string());
            }
            attempt_args.extend(base_download_args.clone());

            // First attempt for this client: no cookies.
            let (code1, _stdout1, stderr1) =
                run_sidecar(app, "binaries/yt-dlp", attempt_args.clone()).await?;
            if code1 == 0 {
                success = true;
                break;
            }
            last_stderr = stderr1.clone();

            // SoundCloud geo restriction: retry with proxy.
            if needs_soundcloud_proxy(&stderr1) {
                let mut proxy_args = attempt_args.clone();
                if let Some(u) = proxy_args.pop() {
                    proxy_args.push("--proxy".to_string());
                    proxy_args.push(soundcloud_proxy.to_string());
                    proxy_args.push(u);
                }
                let (codep, _stdoutp, stderrp) =
                    run_sidecar(app, "binaries/yt-dlp", proxy_args).await?;
                if codep == 0 {
                    success = true;
                    break;
                }
                last_stderr = stderrp;
            }

            // Retry with cookies only for auth/bot style failures.
            if needs_cookies(&stderr1) {
                let mut cookie_args = attempt_args;
                cookie_args.extend([
                    "--cookies-from-browser".to_string(),
                    "chrome".to_string(),
                ]);
                let (code2, _stdout2, stderr2) =
                    run_sidecar(app, "binaries/yt-dlp", cookie_args.clone()).await?;
                if code2 == 0 {
                    success = true;
                    break;
                }
                last_stderr = stderr2;

                // SoundCloud geo restriction even with cookies: retry with proxy.
                if needs_soundcloud_proxy(&last_stderr) {
                    let mut proxy_args = cookie_args;
                    if let Some(u) = proxy_args.pop() {
                        proxy_args.push("--proxy".to_string());
                        proxy_args.push(soundcloud_proxy.to_string());
                        proxy_args.push(u);
                    }
                    let (codep, _stdoutp, stderrp) =
                        run_sidecar(app, "binaries/yt-dlp", proxy_args).await?;
                    if codep == 0 {
                        success = true;
                        break;
                    }
                    last_stderr = stderrp;
                }
            }

            // If this was an android PO-token block, continue to other clients (web/default).
            if client == Some("youtube:player_client=android") && is_android_po_token_block(&last_stderr)
            {
                continue;
            }

            // Otherwise only continue trying other clients for "formats missing" style failures.
            if !needs_fallback(&last_stderr) {
                break;
            }
        }

        if !success {
            // If yt-dlp failed only at postprocessing time, it may have still
            // downloaded the media file (e.g. `<id>.tmp.mp4`). In that case, we can
            // continue to our own ffmpeg extraction step.
            let postproc_failed = last_stderr.contains("Postprocessing:")
                && last_stderr.contains("audio conversion failed");
            if !(postproc_failed && find_tmp_input().is_some()) {
                return Err(AppError::ExtractionFailed(last_stderr));
            }
        }
    }

    if tmp_input.is_none() {
        tmp_input = find_tmp_input();
    }

    let input_path = tmp_input.ok_or_else(|| {
        AppError::ExtractionFailed(format!("no temp file found for {video_id}"))
    })?;

    // ffmpeg -i <tmp.(m4a|mp4|webm...)> -ac 1 -b:a 48k -vn -map 0:a:0 -y <id>.m4a
    // Re-encode to mono 48k, drop video stream, and save as final <id>.m4a
    let ffmpeg_args = vec![
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
        "-vn".to_string(), // drop video stream
        "-map".to_string(), "0:a:0".to_string(), // take first audio stream only
        "-ac".to_string(), "1".to_string(), // mono
        "-b:a".to_string(), "48k".to_string(), // 48k bitrate
        "-f".to_string(), "mov".to_string(), //mov, ipod
        "-c:a".to_string(), "aac".to_string(), // AAC codec
        "-y".to_string(), // overwrite
        audio_path.to_string_lossy().to_string(), // output path
    ];
    let (code, _stdout, stderr) = run_sidecar(app, "binaries/ffmpeg", ffmpeg_args).await?;
    if code != 0 {
        // let _ = std::fs::remove_file(&temp_path);
        return Err(AppError::ExtractionFailed(stderr));
    }
    // let _ = std::fs::remove_file(&temp_path);

    Ok(audio_path)
}

