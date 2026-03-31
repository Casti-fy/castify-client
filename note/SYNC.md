# Sync Flow

## Architecture

Three long-lived tokio tasks running in parallel, communicating via priority channels:

```
+---------------+       +-------------------+       +------------------+
|   Scan Task   |--Job->| Download Worker   |--Job->|  Upload Worker   |
|  (periodic)   |       | (2-4 concurrent)  |       | (cpu/2 concur.)  |
+---------------+       +-------------------+       +------------------+
       |                        |                          |
       +------------------------+--------------------------+
                     emit_progress() -> UI
```

## Entry Points

### 1. App startup -- `auto_start_sync()`

```
has auth token?
  +- no  -> skip
  +- yes -> startup_recovery()     <- fetches incomplete episodes in 1 request
            start_periodic_sync()  <- spawns all 3 tasks
```

- `startup_recovery()` calls `GET /api/v1/episodes?status=pending,failed` to fetch all incomplete episodes across all feeds in a single request. All returned episodes are pushed to the download channel with `Normal` priority. The download worker checks if the audio file exists on disk to skip redundant downloads.

### 2. User creates a feed -- `scan_new_feed()`

```
spawn artwork fetch (parallel, via tokio::spawn)
run_scan(feeds=[new_feed], max=5, Urgent)
  -> creates episodes on server (status: "pending")
  -> pushes jobs to download channel
await artwork handle
```

### 3. User manually syncs a feed -- `sync_single_feed()`

```
push_feed_episodes(Urgent)   <- re-queues existing incomplete episodes
run_scan(max=20, Urgent)     <- discovers new episodes, queues them
```

## Scan Task (periodic loop)

**Location:** `sync.rs`, `sync_scan.rs`

```
loop {
  check if sync_interval has elapsed (polled every 60s)
  if not elapsed -> sleep 60s, continue
  |
  fetch all feeds from server
  |
  for each feed (2s spacing between feeds):
    emit "fetch" progress
    fetch feed detail (existing episodes from server)
    yt-dlp fetch_playlist(source_url, max_items=5)
    |
    filter out video_ids that already exist as episodes
    |
    no new entries? -> emit "done: Already up to date", continue
    |
    for each new entry:
      emit "create" progress
      POST /api/v1/feeds/{id}/episodes  -> episode created as "pending"
      push Job to download channel with current priority
    |
    emit "done: Found N new episodes"
  |
  requeue_incomplete(High)  <- fetch all pending/failed episodes, push to download channel
}
```

After scanning for new episodes, the periodic sync also re-queues any incomplete episodes (pending or failed) via the `GET /api/v1/episodes?status=pending,failed` endpoint. This ensures failed episodes are retried on every sync cycle without the N+1 problem of fetching each feed's detail individually.

## Download Worker

**Location:** `sync_download.rs`

- Concurrency: `cpu_count().clamp(2, 4)` via semaphore
- Dedup: `HashSet<"feed_id:video_id">`, clears at 1000 entries
- Priority: biased `select!` -- urgent > high > normal

```
loop {
  receive Job from priority channels
  |
  skip if feed is in cancelled_feeds set
  skip if already seen (dedup)
  |
  acquire semaphore permit
  spawn task:
    non-urgent? -> random 15-30s delay
    |
    create temp dir: /tmp/castify-{feed_id}/
    audio path: /tmp/castify-{feed_id}/{video_id}.m4a
    |
    audio file already on disk?
      +- yes -> forward Job to upload channel (no server PATCH)
      |         return
      +- no  -> emit "download" progress
                yt-dlp download_audio(url, video_id, temp_dir)
                  +- ok       -> forward Job to upload channel (no server PATCH)
                  +- premiere -> do nothing (stays "pending", retried next cycle)
                  +- error    -> PATCH status to "failed"
}
```

**Note:** The download worker never PATCHes the server on success. It only PATCHes on failure. The file on disk is the checkpoint between download and upload stages.

## Upload Worker

**Location:** `sync_upload.rs`

- Concurrency: `cpu_count() / 2` via semaphore
- Dedup: `HashSet<episode_id>`, clears at 1000 entries
- Priority: biased `select!` -- urgent > high > normal
- Retry: up to 6 attempts with exponential backoff (1s, 2s, 4s, 8s, 16s, 32s, capped at 45s)

```
loop {
  receive Job from priority channels
  |
  skip if feed is in cancelled_feeds set
  skip if already seen (dedup)
  |
  acquire semaphore permit
  spawn task:
    audio file missing?
      +- yes -> PATCH status to "failed", return
    |
    emit "upload" progress
    |
    for attempt in 1..=6:
      GET presigned upload URL from server
        +- error -> retry or PATCH "failed"
      |
      PUT file to B2 (upload_to_b2)
        +- ok        -> PATCH status to "ready" (with file_size)
        |               emit "complete" progress
        |               return
        +- transient -> sleep(backoff), retry
        +- fatal     -> PATCH status to "failed", return
}
```

## Server Endpoint: Incomplete Episodes

```
GET /api/v1/episodes?status=pending,failed&limit=100
```

Returns all incomplete episodes across all feeds for the authenticated user in a single query. Used by startup recovery to avoid the N+1 problem.

Query params:
- `status` -- comma-separated statuses to filter (default: `pending,failed`)
- `limit` -- max results, 1-1000 (default: 100)

Server query:
```sql
SELECT e.*, f.name AS feed_name, f.source_url AS feed_source_url
FROM episodes e
JOIN feeds f ON e.feed_id = f.id
WHERE f.user_id = ?
  AND e.status IN ('pending', 'failed')
  AND e.expired_at IS NULL
  AND f.deleted_at IS NULL
ORDER BY e.created_at ASC
LIMIT 100
```

Response:
```json
[
  {
    "id": "...",
    "feed_id": "...",
    "feed_name": "My Podcast",
    "feed_source_url": "https://youtube.com/...",
    "video_id": "abc123",
    "title": "Episode 1",
    "status": "failed",
    "pub_date": "2026-01-15T00:00:00Z",
    "duration_sec": 3600
  }
]
```

## Priority System

| Priority | When used | Download behavior |
|---|---|---|
| `Urgent` | User-initiated (create feed, manual sync) | Immediate, no delay |
| `High` | Periodic scan finds new episodes | 15-30s random delay |
| `Normal` | Startup recovery of incomplete episodes | 15-30s random delay |

Workers use biased `tokio::select!` so urgent jobs always dequeue first.

## Channels

Two independent channel sets (download and upload), each with 3 priority lanes:

| Channel | Buffer size |
|---|---|
| `urgent` | 64 |
| `high` | 64 |
| `normal` | 256 |

Channels are held in `SyncChannels`. Senders are behind `RwLock` (cloneable, replaceable). Receivers are behind `Mutex<Option<>>` -- taken once when workers start.

## Progress Events

Emitted via `ProgressEmitter` callback (set by GUI to forward to webview). Steps:

| Step | Emitted by | Meaning |
|---|---|---|
| `fetch` | scan | Fetching playlist metadata from source |
| `create` | scan | Creating a new episode on the server |
| `done` | scan | Scan complete for this feed |
| `download` | download worker | Downloading audio via yt-dlp |
| `upload` | upload worker | Uploading audio to B2 |
| `complete` | upload worker | Episode fully processed and ready |

Events are fire-and-forget, not persisted. Lost on restart.

## Stop / Restart

- `stop_periodic_sync()`: aborts all 3 task handles, resets channels (creates fresh sender/receiver pairs).
- `start_periodic_sync()`: spawns new tasks, takes receivers from fresh channels.
- Channel reset ensures old senders become disconnected and old workers (if any) will exit.

## Feed Cancellation

`cancelled_feeds` is a `RwLock<HashSet<String>>` in `AppState`. Both download and upload workers check this set before processing a job. When a feed is cancelled, its in-flight jobs are silently skipped.

## Temp File Management

Audio files are stored at `/tmp/castify-{feed_id}/{video_id}.m4a`. These are **never cleaned up** (known issue). The download worker checks for existing files to avoid redundant downloads.
