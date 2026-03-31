import { check, type DownloadEvent } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { message } from "@tauri-apps/plugin-dialog";

let checking = false;

export interface UpdateProgress {
  phase: "checking" | "downloading" | "installing";
  /** 0–100, or undefined if content-length unknown */
  percent?: number;
}

export async function checkForAppUpdate(
  silent = true,
  onProgress?: (progress: UpdateProgress) => void,
): Promise<void> {
  if (checking) return;
  checking = true;
  try {
    onProgress?.({ phase: "checking" });
    const update = await check();
    if (update) {
      let totalBytes: number | undefined;
      let downloadedBytes = 0;

      onProgress?.({ phase: "downloading", percent: 0 });

      await update.downloadAndInstall((event: DownloadEvent) => {
        if (event.event === "Started") {
          totalBytes = event.data.contentLength ?? undefined;
        } else if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          const percent = totalBytes
            ? Math.min(Math.round((downloadedBytes / totalBytes) * 100), 100)
            : undefined;
          onProgress?.({ phase: "downloading", percent });
        } else if (event.event === "Finished") {
          onProgress?.({ phase: "installing" });
        }
      });

      await relaunch();
    } else if (!silent) {
      await message("You're on the latest version.", { title: "No Updates", kind: "info" });
    }
  } catch (err) {
    console.error("Update check failed:", err);
    if (!silent) {
      const detail = err instanceof Error ? err.message : String(err);
      await message(`Could not check for updates:\n${detail}`, { title: "Update Error", kind: "error" });
    }
  } finally {
    checking = false;
    onProgress?.(undefined as any);
  }
}

export function scheduleMidnightUTCCheck(): () => void {
  // App.tsx already triggers an initial silent check on startup.
  // Here we repeat checks every hour.
  const ONE_HOUR_MS = 60 * 60 * 1000;
  const intervalId = setInterval(checkForAppUpdate, ONE_HOUR_MS);

  return () => clearInterval(intervalId);
}
