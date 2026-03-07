# Castify build options

Two separate build targets (GUI and CLI are not bundled).

## 1. Desktop app (GUI only)

**Single binary:** the desktop app only. No CLI in this bundle.

- **Build:** `npm run tauri build` (or `cargo build --release` in `src-tauri`).
- **Output:** Platform bundle (e.g. `.app`, `.exe`, `.deb`) containing the `castify` binary.
- **Usage:** Double‑click / open app → GUI only.

## 2. CLI-only (power users / AI agents)

**Separate binary:** no GUI, no Tauri; minimal dependencies and size.

- **Build:** from repo root or `src-tauri`:
  ```bash
  cargo build --release --bin castify-cli --no-default-features --features cli
  ```
- **Output:** `target/release/castify-cli` (or `castify-cli.exe` on Windows).
- **Usage:** `castify-cli login ...`, `castify-cli feed list`, etc.

Use for scripting, CI, or headless environments. Distribute this binary alone (e.g. GitHub Releases) for users who don’t need the desktop app.

---

### Summary

| Build       | Command                                                          | Result                |
|-------------|------------------------------------------------------------------|------------------------|
| Desktop     | `npm run tauri build`                                            | App bundle (GUI only)  |
| CLI-only    | `cargo build --release --bin castify-cli --no-default-features --features cli` | Single `castify-cli` binary |

### NPM scripts (optional)

You can add to `package.json`:

```json
"scripts": {
  "tauri": "tauri",
  "build:cli": "cd src-tauri && cargo build --release --bin castify-cli --no-default-features --features cli"
}
```

Then: `npm run build:cli` → `src-tauri/target/release/castify-cli`.

### CLI config (castify-cli only)

- Config file: `~/.config/castify/config.json` (or `%APPDATA%\castify\config.json` on Windows).
- Stores: `jwt_token`, optional `server_url`.
- Server URL resolution: `--server` flag > env `CASTIFY_SERVER_URL` > config file > default.
