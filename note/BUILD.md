# Castify Build Pipeline

## Local development

```bash
npm install
bash scripts/build-dependencies.sh   # builds yt-dlp, ffmpeg, deno sidecars
npm run tauri dev                     # starts dev server + Tauri app
```

## Production build

```bash
npm run tauri build -- --target <triple>
```

This runs:
1. `vite build` — bundles React frontend into `dist/`
2. `cargo build --release` — compiles Rust backend
3. Tauri bundler — creates platform-specific installer (`.dmg`, `.exe`, `.deb`, `.AppImage`)

## CI/CD (GitHub Actions)

Triggered by pushing a `v*` tag or manual dispatch. Builds 5 targets in parallel:

| Runner | Target | Output |
|--------|--------|--------|
| `macos-latest` | `aarch64-apple-darwin` | `.dmg` |
| `macos-15-intel` | `x86_64-apple-darwin` | `.dmg` |
| `windows-latest` | `x86_64-pc-windows-msvc` | `.exe` (NSIS) |
| `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | `.deb` + `.AppImage` |
| `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | `.deb` + `.AppImage` |

### Build steps (per target)

1. **Setup** — install Rust, Node.js 22, Python 3.12, platform deps
2. **Sidecar binaries** (`scripts/build-dependencies.sh`):
   - **yt-dlp** — built from source with PyInstaller (YouTube + SoundCloud extractors only)
   - **ffmpeg** — compiled from source (minimal audio-only config)
   - **deno** — downloaded prebuilt from GitHub releases
3. **macOS only** — import Apple Developer certificate into temp keychain
4. **Tauri build** — `tauri build --target <triple>`:
   - Bundles frontend + Rust binary + sidecars into `.app`
   - Signs all binaries with Apple Developer ID (macOS)
   - Notarizes with Apple (macOS)
   - Creates `.dmg` / `.exe` / `.deb` / `.AppImage`
5. **Post-build fixes**:
   - **macOS** — re-sign yt-dlp sidecar inside DMG with `disable-library-validation` entitlement (Tauri strips custom entitlements during signing), then re-notarize
   - **Linux** — repack AppImage with fuse3-compatible runtime (no libfuse2 needed on Debian 13+)
6. **Release** — uploads all artifacts to `Casti-fy/castify-web` GitHub Releases

### Sidecar binaries

Tauri sidecars must be named with the target triple suffix:

```
src-tauri/binaries/
  yt-dlp-aarch64-apple-darwin
  ffmpeg-aarch64-apple-darwin
  deno-aarch64-apple-darwin
```

At runtime, Tauri resolves the correct binary for the current platform.

### macOS code signing

yt-dlp is built with PyInstaller, which bundles a Python.framework inside the binary. At runtime it extracts Python to a temp dir and loads it via `dlopen`. On Apple Silicon, macOS rejects this because the extracted Python has a different Team ID than the signed app.

Fix: the yt-dlp sidecar is signed with `com.apple.security.cs.disable-library-validation` entitlement (`src-tauri/entitlements/yt-dlp.plist`). This must be applied **after** the Tauri build because Tauri re-signs all binaries without custom entitlements.

### Windows ffmpeg

ffmpeg is statically linked on Windows (`--extra-ldflags="-static"`) to avoid requiring MinGW runtime DLLs (e.g., `libwinpthread-1.dll`) on end-user machines.

### Linux .deb dependencies

The `.deb` specifies alternative dependencies for compatibility with both older Ubuntu and Debian 13+ (t64 transition):

```
libwebkit2gtk-4.1-0 | libwebkit2gtk-4.1-0t64
libgtk-3-0 | libgtk-3-0t64
libayatana-appindicator3-1 | libappindicator3-1
```

## Required CI secrets

| Secret | Description |
|--------|-------------|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` |
| `APPLE_SIGNING_IDENTITY` | e.g., `Developer ID Application: Name (TEAM_ID)` |
| `APPLE_ID` | Apple ID email for notarization |
| `APPLE_PASSWORD` | App-specific password for notarization |
| `APPLE_TEAM_ID` | Apple Developer Team ID |
| `RELEASE_TOKEN` | GitHub PAT with write access to `Casti-fy/castify-web` |
