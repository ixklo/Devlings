import { useNow } from "../shared/time";
import type { UsageInfo } from "../shared/types";
import { usageWarning } from "../shared/usage";

/** The mini chat's footer note when the last Ask said the plan is near or at its limit (v1.0 S4). */
export function UsageNote({ usage }: { usage: UsageInfo | null }) {
  const now = useNow();
  const text = usage ? usageWarning(usage, now) : null;
  return text ? <p className="usage-note">{text}</p> : null;
}
