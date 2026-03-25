# TODO

## Features

- [ ] Episode analytics — track episode consumption by logging `/audio` requests server-side. Log feed ID, episode ID, timestamp, `User-Agent`, and `Range` header. Use range-header analysis to distinguish full listens from partial fetches. Filter bot/crawler traffic. Hash IPs for unique listener estimation. v1: simple request logging per episode; v2: deduplicate range requests and add a dashboard.

- [ ] Cookie browser setting — add a dropdown in settings for the user to select which browser yt-dlp should load cookies from (chrome, firefox, safari, edge, brave, opera, vivaldi, whale). When set, pass `--cookies-from-browser <browser>` to yt-dlp and remove the `subscriber_only` / `needs_premium` filter so member-exclusive videos can be downloaded.

- [ ] On-demand deno download — don't ship deno in the bundle (~40MB savings). Instead, detect yt-dlp n-sig failures at runtime, download deno to a cache dir on demand, and retry with deno on PATH. Most users will never need it since yt-dlp's built-in JS interpreter handles n-sig most of the time.

- [ ] Auto-update yt-dlp — when downloads start failing (n-sig errors, throttling, format errors), check for a newer yt-dlp release on GitHub, download it to a cache dir, and use it instead of the bundled version. This avoids rebuilding/releasing the app every time YouTube breaks things.

- [ ] Feed scan order option — when creating a feed, allow the user to choose a scan order: **latest** (default), **popular**, or **oldest**. This controls how yt-dlp sorts/fetches videos from the source (e.g., playlist or channel). Useful for backfilling old content or grabbing the most-viewed episodes first.

- [ ] Client identification — set a `User-Agent` header on the API client (e.g., `Castify/0.1.0 (macos; aarch64)`) so the server knows which platform/version users are on. Use `env!("CARGO_PKG_VERSION")`, `std::env::consts::OS`, and `std::env::consts::ARCH`.

- [ ] Guest mode — allow unauthenticated users to add a feed and browse episodes without registering or downloading. Limit to 1 feed and 5 episodes. This lets users try the core experience before committing to an account.

- [x] App auto-update — use Tauri's built-in updater plugin (`tauri-plugin-updater`) to check for new app versions on startup. Show an in-app notification/banner when an update is available (e.g., "Castify v0.2.0 is available — click to update"). The update endpoint can point to the GitHub Releases on `Casti-fy/castify-web`. Tauri's updater supports delta updates and signature verification out of the box.

- [] CLI tool — restore the CLI binary (`castify-cli`) for scripting, CI, and headless use. The service layer is already decoupled from Tauri (`services/` has no GUI deps). Previous implementation exists in git history (commit `13e6389`). Steps: (1) re-create `cli.rs` with clap subcommands (login, logout, feed list/add, sync), (2) re-create `storage.rs` for file-based credential storage at `~/.config/castify/`, (3) add `[[bin]]` target in Cargo.toml with `--no-default-features --features cli`. Main challenge: services take `&AppHandle` — need to abstract over a trait or create a lightweight CLI-only app context.

## Marketing

- [] Earnings call feeds — create "Castify Official" showcase feeds for top earnings calls (AAPL, TSLA, NVDA, MSFT, GOOG, META, AMZN). Steps: (1) make RSS builder directory-compliant (add `<itunes:owner>`, `<itunes:email>`, `<itunes:type>`, `<podcast:guid>`), (2) create official account that owns these feeds, (3) brand with "Powered by Castify" in episode descriptions + link back to site, (4) submit feeds to Podcast Index and Apple Podcasts for discoverability. These are public content with no copyright issues and target a financially motivated audience likely to convert.

- [ ] Government / public affairs feeds — Fed press conferences, FOMC meetings, congressional hearings, White House briefings. Public domain, time-sensitive, same financially motivated audience as earnings calls. Natural extension of the earnings call feeds.

- [ ] Tech conference talk feeds — curate feeds from CC-licensed conferences (PyCon, RustConf, Linux Foundation events). Developer audience likely to convert into Castify users who create their own feeds. Verify each conference's license before including.

- [ ] Public domain audiobook feeds — create showcase feeds from CC-licensed sources (MIT OpenCourseWare, LibriVox, Project Gutenberg audiobooks). Curate a few themed feeds (e.g., "Classic Sci-Fi Audiobooks", "Philosophy Lectures"). Stick to explicitly CC-licensed or public domain content only. Evergreen content that doesn't expire, but lower urgency than earnings calls — good as a second wave.
