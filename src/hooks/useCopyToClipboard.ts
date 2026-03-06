import { useState, useCallback, useRef } from "react";

export function useCopyToClipboard(timeout = 1500) {
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const copy = useCallback(
    (text: string, id = "default") => {
      navigator.clipboard.writeText(text);
      setCopiedId(id);
      clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => setCopiedId(null), timeout);
    },
    [timeout]
  );

  return { copiedId, copy } as const;
}
