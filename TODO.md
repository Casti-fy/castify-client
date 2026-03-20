# TODO

## Features

- [ ] Cookie browser setting — add a dropdown in settings for the user to select which browser yt-dlp should load cookies from (chrome, firefox, safari, edge, brave, opera, vivaldi, whale). When set, pass `--cookies-from-browser <browser>` to yt-dlp and remove the `subscriber_only` / `needs_premium` filter so member-exclusive videos can be downloaded.

- [ ] On-demand deno download — don't ship deno in the bundle (~40MB savings). Instead, detect yt-dlp n-sig failures at runtime, download deno to a cache dir on demand, and retry with deno on PATH. Most users will never need it since yt-dlp's built-in JS interpreter handles n-sig most of the time.

- [ ] Auto-update yt-dlp — when downloads start failing (n-sig errors, throttling, format errors), check for a newer yt-dlp release on GitHub, download it to a cache dir, and use it instead of the bundled version. This avoids rebuilding/releasing the app every time YouTube breaks things.
