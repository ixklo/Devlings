import type { UsageInfo } from "./types";

// Plan usage lines (v1.0 S4). The numbers come from the last Ask run, so every line says when they were seen and
// nothing here implies they're live.

const LIMITS = new Map<string, string>([
  ["five_hour", "5-hour limit"],
  ["seven_day", "weekly limit"],
  ["seven_day_opus", "weekly Opus limit"],
  ["seven_day_sonnet", "weekly Sonnet limit"],
  ["seven_day_overage_included", "weekly limit"],
]);

const DAY_MS = 86_400_000;

function limitName(kind: string | undefined): string {
  return (kind && LIMITS.get(kind)) || "plan limit";
}

function percent(utilization: number): number {
  return Math.min(100, Math.max(0, Math.round(utilization * 100)));
}

/** Some locales put a narrow no-break space before AM/PM; a plain space reads the same and wraps predictably. */
const plain = (s: string) => s.replace(/ /g, " ");

/** A local time: "3:10 PM" today, "Tue 9:00 AM" within about a week, "Oct 5, 9:00 AM" beyond. */
export function clockTime(ms: number, now: number, locale?: string): string {
  const d = new Date(ms);
  const time = plain(d.toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" }));
  if (d.toDateString() === new Date(now).toDateString()) return time;
  if (Math.abs(ms - now) < 6.5 * DAY_MS) return `${d.toLocaleDateString(locale, { weekday: "short" })} ${time}`;
  return `${d.toLocaleDateString(locale, { month: "short", day: "numeric" })}, ${time}`;
}

function resetPart(u: UsageInfo, now: number, locale?: string): string {
  if (u.resetsAt === undefined) return "";
  const t = clockTime(u.resetsAt, now, locale);
  return u.resetsAt > now ? `, resets ${t}` : `, reset at ${t}`;
}

/**
 * The Settings line, e.g. "Plan usage: 42% of your 5-hour limit, resets 3:10 PM (as of 2:05 PM, from your last Ask)".
 * Missing fields are left out.
 */
export function usageSummary(u: UsageInfo, now: number, locale?: string): string {
  const limit = limitName(u.kind);
  let main: string;
  if (u.status === "rejected") main = `your ${limit} is used up`;
  else if (u.utilization !== undefined) main = `${percent(u.utilization)}% of your ${limit}`;
  else if (u.status === "allowed") main = `within your ${limit}`;
  else if (u.status === "allowed_warning") main = `close to your ${limit}`;
  else main = "no details reported";
  return `Plan usage: ${main}${resetPart(u, now, locale)} (as of ${clockTime(u.seenAt, now, locale)}, from your last Ask)`;
}

/**
 * The mini chat's footer line, only when the plan is near or at its limit, e.g. "Near your 5-hour limit (82%),
 * resets 3:10 PM · as of 2:05 PM". Null otherwise, including for statuses Perch doesn't know.
 */
export function usageWarning(u: UsageInfo, now: number, locale?: string): string | null {
  const limit = limitName(u.kind);
  let main: string;
  if (u.status === "allowed_warning") {
    main = `Near your ${limit}${u.utilization !== undefined ? ` (${percent(u.utilization)}%)` : ""}`;
  } else if (u.status === "rejected") {
    main = `${limit.charAt(0).toUpperCase()}${limit.slice(1)} reached`;
  } else {
    return null;
  }
  const resets = u.resetsAt !== undefined && u.resetsAt > now ? `, resets ${clockTime(u.resetsAt, now, locale)}` : "";
  return `${main}${resets} · as of ${clockTime(u.seenAt, now, locale)}`;
}
