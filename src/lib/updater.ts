import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ask } from "@tauri-apps/plugin-dialog";

let checking = false;

export async function checkForAppUpdate(): Promise<void> {
  if (checking) return;
  checking = true;
  try {
    const update = await check();
    if (update) {
      const confirmed = await ask(
        `A new version (${update.version}) is available. Update and restart now?`,
        { title: "Update Available", kind: "info" },
      );
      if (confirmed) {
        await update.downloadAndInstall();
        await relaunch();
      }
    }
  } catch (err) {
    console.error("Update check failed:", err);
  } finally {
    checking = false;
  }
}

function msUntilMidnightUTC(): number {
  const now = new Date();
  const tomorrow = new Date(
    Date.UTC(
      now.getUTCFullYear(),
      now.getUTCMonth(),
      now.getUTCDate() + 1,
      0,
      0,
      0,
      0,
    ),
  );
  return Math.max(tomorrow.getTime() - now.getTime(), 60_000);
}

export function scheduleMidnightUTCCheck(): () => void {
  const ONE_DAY_MS = 24 * 60 * 60 * 1000;
  let intervalId: ReturnType<typeof setInterval> | undefined;

  const timeoutId = setTimeout(() => {
    checkForAppUpdate();
    intervalId = setInterval(checkForAppUpdate, ONE_DAY_MS);
  }, msUntilMidnightUTC());

  return () => {
    clearTimeout(timeoutId);
    if (intervalId !== undefined) clearInterval(intervalId);
  };
}
