# Installing castify-cli

`castify-cli` is the standalone CLI (no GUI). Use it for scripting, CI, or headless setups.

---

## Option 1: Download from GitHub Releases (recommended)

When we publish a release, CLI binaries are attached per platform.

1. Open [Releases](https://github.com/Apollo-Audio/castify-client/releases) and pick the latest version.
2. Download the archive for your OS/arch, e.g.:
   - **macOS Apple Silicon:** `castify-cli-aarch64-apple-darwin.tar.gz`
   - **macOS Intel:** `castify-cli-x86_64-apple-darwin.tar.gz`
   - **Linux x64:** `castify-cli-x86_64-unknown-linux-gnu.tar.gz`
   - **Windows x64:** `castify-cli-x86_64-pc-windows-msvc.zip`
3. Unpack and put the binary in your PATH:
   - **macOS / Linux:** e.g. `sudo mv castify-cli /usr/local/bin/` or `mv castify-cli ~/bin/`
   - **Windows:** e.g. add the folder to `PATH` or copy `castify-cli.exe` to a directory already in `PATH`
4. Confirm: `castify-cli status`

---

## Option 2: Build from source (Rust required)

```bash
# From the repo root
git clone https://github.com/Apollo-Audio/castify-client.git
cd castify-client
cargo build --release --bin castify-cli --no-default-features --features cli -p castify
```

Binary: `src-tauri/target/release/castify-cli` (or `castify-cli.exe` on Windows).

---

## Option 3: cargo install (Rust required)

If the crate is published to crates.io:

```bash
cargo install castify --bin castify-cli --no-default-features --features cli
```

Then run `castify-cli` from anywhere.

---

## Config after install

- **Config file:** `~/.config/castify/config.json` (or `%APPDATA%\castify\config.json` on Windows).
- **Login:** `castify-cli login` (prompts for email/password) or `castify-cli login -u you@example.com -p secret`.
- **Server URL:** Set via `--server URL`, or env `CASTIFY_SERVER_URL`, or once logged in with `--server` it’s saved in config.
