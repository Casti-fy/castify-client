# Episode State Machine

## Server States (3)

| Status | Description |
|---|---|
| `pending` | Episode created or being processed by a client |
| `ready` | Audio uploaded to B2, available in RSS feed |
| `failed` | Last processing attempt failed, no client is currently working on it |

## State Diagram

```
+---------+          +---------+
| pending |--------->|  ready  |
+---------+          +---------+
     |
     v
 +--------+
 | failed |-------> pending (client retries)
 +--------+
```

## Client Logic (no local state)

The file system is the state. The download worker checks if `{video_id}.m4a` exists on disk:
- No file -> download via yt-dlp, then forward to upload worker
- File exists -> skip download, forward to upload worker directly

The upload worker uploads to B2, then PATCHes the server to `ready`.

## Transitions

### pending -> ready
- **Trigger:** Upload worker successfully uploads audio to B2.
- **Where:** `sync_upload.rs:82`
- **HTTP:** `PATCH /api/v1/episodes/{id}` with `status=ready` and `file_size`.
- **Note:** This is the only PATCH in the happy path. No intermediate status updates.

### pending -> failed
- **Trigger:** One of:
  - Download fails with a non-transient error (`sync_download.rs:100`)
  - Audio file is missing on disk when upload worker picks it up (`sync_upload.rs:30`)
  - Failed to get presigned upload URL after retries (`sync_upload.rs:64`)
  - Upload to B2 failed after 6 retry attempts (`sync_upload.rs:107`)
- **HTTP:** `PATCH /api/v1/episodes/{id}` with `status=failed`.

### pending -> pending (implicit, no status change)
- **Trigger:** Video is an upcoming premiere (`Premieres in` or `is_upcoming` in error). No status update is written; the episode stays `pending` and will be retried on the next sync cycle.
- **Where:** `sync_download.rs:96-97`

### failed -> pending (re-queue)
- **Trigger:** On startup recovery or periodic sync, failed episodes are re-queued into the download channel. The download worker checks if the audio file exists on disk to skip redundant downloads.
- **Where:** `sync.rs` startup_recovery and push_feed_episodes

## Design Decisions

### Why no `uploading` server status
The server doesn't need to know what stage the client is in (downloading vs uploading). It only needs to know: is someone working on it (`pending`), is it done (`ready`), or did it fail (`failed`). The file on disk is the checkpoint between download and upload stages.

### Why no local state machine
The download worker checks if the `.m4a` file exists. If yes, it skips download and forwards to upload. This means failed uploads don't cause redundant re-downloads -- the file check handles routing automatically.

### HTTP calls per episode
- **Happy path:** 1 PATCH (`pending` -> `ready`)
- **Failure path:** 1 PATCH (`pending` -> `failed`)
- **Previous design:** 2 PATCHes per episode (`pending` -> `uploading` -> `ready`)
