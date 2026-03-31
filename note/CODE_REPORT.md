# Castify Client — Code Review Report

## Architecture Overview

Tauri v2 desktop app (Rust backend + React frontend) that converts YouTube/SoundCloud channels into podcast RSS feeds. Downloads audio via yt-dlp, uploads to B2, serves via API. Recent refactor cleanly decoupled services from Tauri — good foundation.

---

## CRITICAL Issues

### 1. Hardcoded Proxy Credentials in Source Code
**`src-tauri/src/services/extractor.rs:426`**
```rust
let soundcloud_proxy = "http://45f8da4c3a:mIebKzA1@207.182.30.55:4444";
```
Proxy username/password committed to source. This is a **security vulnerability** — anyone who reads the repo gets your proxy credentials. Should be an env var or fetched from the API at runtime.

### 2. Scan Loop is a Busy-Wait CPU Burner
**`src-tauri/src/services/sync.rs:121-139`**
```rust
loop {
    let interval_minutes = read_sync_interval(&state_scan);
    let interval = Duration::from_secs(interval_minutes * 60);
    if let Some(last) = last_scan {
        if last.elapsed() < interval {
            continue;  // ← TIGHT LOOP, no sleep!
        }
    }
    // ... do scan ...
}
```
When it's not time to scan yet, this `continue`s without sleeping — burning 100% of a CPU core spinning on `read_sync_interval()` and time checks. Needs a `tokio::time::sleep` on the `continue` branch.

### 3. Upload Worker Concurrency Can Be Zero
**`src-tauri/src/services/sync_upload.rs:117`**
```rust
let max_concurrent = helpers::cpu_count() / 2;
```
On a single-core machine (or wasm), `cpu_count()` returns 1 → `1 / 2 = 0` → `Semaphore::new(0)` → uploads are permanently blocked. Needs `.max(1)`.

### 4. No Token Expiration / Refresh Handling
**`src-tauri/src/services/api_client.rs`** — JWT is stored and used forever. If the token expires, every API call returns 401 but there's no automatic refresh, no re-auth prompt pushed to the frontend, and no cache invalidation. The user sees silent failures until they manually log out/in.

---

## HIGH Severity

### 5. Downloaded Temp Files Are Never Cleaned Up
**`src-tauri/src/services/extractor.rs:572-575`** — The temp file cleanup is commented out:
```rust
// let _ = std::fs::remove_file(&temp_path);
```
And `temp_dir_for_feed()` in `helpers.rs:27` writes to the system temp dir but never cleans up. Over time this accumulates gigabytes of `.m4a`, `.tmp.m4a`, `.webm`, etc. files. There's a `clear_sync_cache` command but it requires manual user action.

### 6. `seen` Set Eviction Nukes Deduplication
**`sync_download.rs:38-40` and `sync_upload.rs:139-141`**
```rust
if seen.len() >= SEEN_CAP {
    seen.clear();
}
```
When the set hits 1000 entries, it clears **everything** — allowing all previously-seen jobs to be re-processed. This is a poor-man's LRU. At minimum, clear the oldest half, or use an actual bounded LRU cache (`lru` crate).

### 7. `download_audio` Duplicates `ytdlp_base_args` Logic
**`extractor.rs:383-395`** manually builds `--remote-components` and `--js-runtimes` args, while `ytdlp_base_args()` at line 82 does exactly the same thing. This means changes to one location won't be reflected in the other — a maintenance trap.

### 8. Race Condition: Double Sync Start
**`auth.rs:52-54`** — `apply_auth` spawns `auto_start_sync`, but `auto_start_sync` is also called during app startup (`lib.rs:64`). If login completes while startup sync is still initializing, you get two scan loops and two worker pairs running concurrently on the same channels — double-processing jobs.

---

## MEDIUM Severity

### 9. No Router — Manual Page State
**`src/App.tsx`** — Page navigation is a discriminated union in React state. No URL routing means: no deep linking, no browser back/forward, no shareable URLs. Fine for MVP, but becomes painful fast.

### 10. Frontend Has Zero Error Boundaries
If any component throws during render, the entire app white-screens. A single `ErrorBoundary` wrapper would prevent this.

### 11. No Input Sanitization on Feed Source URLs
**Feed creation** passes user-provided URLs directly to yt-dlp as command arguments. While `tokio::process::Command` doesn't use shell interpolation (so no injection), a malicious URL with `--` prefixed arguments could theoretically inject yt-dlp flags. Should validate URL format before passing to the extractor.

### 12. `FileConfigStore::save_file` Silently Ignores Write Errors
**`config_store.rs:61`**
```rust
let _ = std::fs::write(path, json);
```
If the disk is full or permissions fail, token save silently succeeds but the data is lost. User logs in, app "remembers" token in memory, then on restart it's gone.

### 13. `ExitRequested` Prevents All Exits
**`lib.rs:129-135`** — `ExitRequested` handler calls `api.prevent_exit()` and hides the window. But the "Quit" tray menu item calls `app.exit(0)` which should work... except that `prevent_exit` is called unconditionally. This could cause issues on OS shutdown or force-quit scenarios.

### 14. `block_on` During Tauri Setup
**`lib.rs:56-58`**
```rust
tauri::async_runtime::block_on(async {
    state.api.write().await.set_token(Some(token));
});
```
Blocking on an async lock inside the synchronous `setup` closure. Currently safe because nothing else is running yet, but fragile — any future code that acquires the lock before this point will deadlock.

---

## LOW Severity / Code Quality

### 15. Zero Tests
No unit tests, no integration tests, no snapshot tests. The entire sync pipeline, extractor parsing, upload retry logic, and config store are completely untested.

### 16. HTTP Method as String
**`api_client.rs:38-44`** — HTTP methods are matched as strings (`"POST"`, `"GET"`, etc.). A typo like `"DELTE"` silently falls through to GET. Should be an enum.

### 17. `Content-Type: application/json` Set Unconditionally
**`api_client.rs:46`** — Even GET/DELETE requests without bodies get a JSON content-type header. Harmless but sloppy.

### 18. Frontend Types Are Manually Duplicated
`src/lib/types.ts` manually defines types that must match the Rust models in `src-tauri/src/models.rs`. No codegen, no validation — drift is inevitable.

### 19. No Lint / Format Config
No `.eslintrc`, no `prettier.config`, no `rustfmt.toml`, no `clippy.toml`. Relying on developer discipline for consistency.

### 20. `run_sync_for_feeds` Is a Pointless Wrapper
**`sync.rs:91-98`** — This function just calls `sync_scan::run_scan` with the same args. Zero value-add, adds indirection.

---

## Summary Scorecard

| Area | Rating | Notes |
|------|--------|-------|
| **Architecture** | Good | Clean service/command split, solid Tauri decoupling |
| **Security** | Poor | Hardcoded proxy creds, no token refresh, no URL validation |
| **Reliability** | Moderate | Busy-wait loop, race conditions, zero-concurrency edge case |
| **Resource Mgmt** | Poor | Temp files leak, no cleanup, unbounded memory in seen sets |
| **Testing** | None | Zero test coverage |
| **Frontend** | Minimal | Works but bare — no routing, no error boundaries, no state lib |
| **Code Quality** | Decent | Well-structured Rust, good use of traits and async patterns |

**Top 3 fixes to prioritize:**
1. Add `tokio::time::sleep` to the scan loop (CPU burn — affects all users now)
2. Remove hardcoded proxy credentials from source
3. Clamp upload concurrency to `max(1)` (blocks uploads on low-core machines)
