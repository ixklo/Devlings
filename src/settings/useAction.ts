import { useCallback, useRef, useState } from "react";
import { errorText } from "../shared/errors";

export interface Notice {
  ok: boolean;
  text: string;
}

/** Runs a command, tracks busy state, and keeps a short-lived success or error notice. */
export function useAction() {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const run = useCallback(async (action: () => Promise<unknown>, ok?: string): Promise<boolean> => {
    window.clearTimeout(timer.current);
    setBusy(true);
    setNotice(null);
    try {
      await action();
      if (ok) {
        setNotice({ ok: true, text: ok });
        timer.current = window.setTimeout(() => setNotice(null), 2400);
      }
      return true;
    } catch (e) {
      setNotice({ ok: false, text: errorText(e) });
      return false;
    } finally {
      setBusy(false);
    }
  }, []);

  const dismiss = useCallback(() => setNotice(null), []);
  return { busy, notice, run, dismiss };
}
