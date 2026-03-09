# Castify

Desktop app for converting YouTube playlists into podcast feeds. Built with [Tauri v2](https://v2.tauri.app/) (Rust backend + React frontend).

## Project Structure

```
src/                → Frontend (React + Vite + TypeScript)
  pages/            → Page components (Login, FeedsList, FeedDetail, Settings)
  components/       → Shared UI components
  lib/              → API client, types, utilities
src-tauri/          → Backend (Rust + Tauri)
  src/commands/     → Tauri commands (sync, feeds, auth, settings)
  src/services/     → Audio extraction (yt-dlp), file upload (B2)
  src/models.rs     → Shared data types
scripts/            → Build scripts for sidecar binaries (yt-dlp, ffmpeg)
```

## Development

```bash
npm install
cargo tauri dev
```

## Frontend-Backend Communication

The frontend and backend communicate through Tauri's IPC bridge — not HTTP:

- **`invoke()`** — Frontend calls a Rust command by name and awaits the result (like RPC)
- **`listen()`** / **`emit()`** — Backend pushes events to the frontend (e.g. sync progress updates)

They are tightly coupled and must run together via `cargo tauri dev`. Running the Vite dev server alone (`npm run dev`) works for UI/styling, but all `invoke()` calls will fail without the Tauri backend.

## Debugging

- **Frontend**: Open devtools in the app window with `Cmd+Option+I` (macOS)
- **Backend**: Rust logs appear in the terminal running `cargo tauri dev`

## Build

```bash
cargo tauri build
```
