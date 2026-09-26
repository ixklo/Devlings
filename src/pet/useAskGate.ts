import { useCallback, useState } from "react";
import { api } from "../shared/api";
import { errorText } from "../shared/errors";

/**
 * Wraps an Ask send with the first-send credits notice: when the backend
 * rejects with "credits_notice", hold the prompt until the user confirms,
 * mark the notice seen, then send it again.
 */
export function useAskGate(send: (text: string) => Promise<void>) {
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = useCallback(
    async (text: string): Promise<boolean> => {
      setError(null);
      setBusy(true);
      try {
        await send(text);
        return true;
      } catch (e) {
        const msg = errorText(e);
        if (msg === "credits_notice") setPending(text);
        else setError(msg);
        return false;
      } finally {
        setBusy(false);
      }
    },
    [send],
  );

  const confirm = useCallback(async (): Promise<boolean> => {
    const text = pending;
    setPending(null);
    if (text === null) return false;
    try {
      await api.markCreditsNoticeSeen();
    } catch (e) {
      setError(errorText(e));
      return false;
    }
    return submit(text);
  }, [pending, submit]);

  const cancel = useCallback(() => setPending(null), []);
  const clearError = useCallback(() => setError(null), []);

  return { pending, error, busy, submit, confirm, cancel, clearError };
}
