import { useEffect } from "react";
import { listen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";

export function useTauriListener<T>(event: string, callback: EventCallback<T>, deps: unknown[] = []) {
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<T>(event, callback).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, deps);
}
