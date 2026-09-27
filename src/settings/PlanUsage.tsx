import { useNow } from "../shared/time";
import type { UsageInfo } from "../shared/types";
import { usageSummary } from "../shared/usage";

/** Settings → Cost: the plan usage the last Ask reported (v1.0 S4). Never live; the line says when it was seen. */
export function PlanUsage({ usage }: { usage: UsageInfo | null }) {
  const now = useNow();
  return <p className="row-desc plan-usage">{usage ? usageSummary(usage, now) : "Plan usage shows here after your next Ask."}</p>;
}
