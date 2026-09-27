import { describe, expect, it } from "vitest";
import type { UsageInfo } from "./types";
import { clockTime, usageSummary, usageWarning } from "./usage";

// Local times, so the expectations hold in any time zone. The locale is pinned to en-US.
const at = (day: number, h: number, m: number) => new Date(2026, 8, day, h, m).getTime();
const NOW = at(26, 14, 30); // Saturday 26 September 2026, 2:30 PM
const L = "en-US";

const usage = (over: Partial<UsageInfo> = {}): UsageInfo => ({
  status: "allowed",
  resetsAt: at(26, 15, 10),
  utilization: 0.42,
  kind: "five_hour",
  seenAt: at(26, 14, 5),
  ...over,
});

describe("clockTime", () => {
  it("shows just the time for today, the weekday within a week, and the date beyond", () => {
    expect(clockTime(at(26, 15, 10), NOW, L)).toBe("3:10 PM");
    expect(clockTime(at(26, 9, 5), NOW, L)).toBe("9:05 AM");
    expect(clockTime(at(29, 9, 0), NOW, L)).toBe("Tue 9:00 AM");
    expect(clockTime(at(25, 23, 0), NOW, L)).toBe("Fri 11:00 PM");
    expect(clockTime(at(30 + 5, 9, 0), NOW, L)).toBe("Oct 5, 9:00 AM");
  });
});

describe("usageSummary (Settings)", () => {
  it("gives the percent, the limit, the reset time and when it was seen", () => {
    expect(usageSummary(usage(), NOW, L)).toBe(
      "Plan usage: 42% of your 5-hour limit, resets 3:10 PM (as of 2:05 PM, from your last Ask)",
    );
  });

  it("names each kind of limit, and falls back to the plan limit", () => {
    const line = (kind?: string) => usageSummary(usage({ kind, resetsAt: undefined }), NOW, L);
    expect(line("seven_day")).toBe("Plan usage: 42% of your weekly limit (as of 2:05 PM, from your last Ask)");
    expect(line("seven_day_opus")).toContain("of your weekly Opus limit");
    expect(line("seven_day_sonnet")).toContain("of your weekly Sonnet limit");
    expect(line("brand_new_window")).toContain("of your plan limit");
    expect(line("constructor")).toContain("of your plan limit");
    expect(line(undefined)).toContain("of your plan limit");
  });

  it("rounds and bounds the percent", () => {
    expect(usageSummary(usage({ utilization: 0.006 }), NOW, L)).toContain(" 1% of");
    expect(usageSummary(usage({ utilization: 0 }), NOW, L)).toContain(" 0% of");
    expect(usageSummary(usage({ utilization: 0.999 }), NOW, L)).toContain(" 100% of");
  });

  it("says what the status means when there's no percent", () => {
    const line = (status: string) => usageSummary(usage({ status, utilization: undefined }), NOW, L);
    expect(line("allowed")).toBe("Plan usage: within your 5-hour limit, resets 3:10 PM (as of 2:05 PM, from your last Ask)");
    expect(line("allowed_warning")).toContain("Plan usage: close to your 5-hour limit, resets 3:10 PM");
    expect(line("rejected")).toContain("Plan usage: your 5-hour limit is used up, resets 3:10 PM");
    expect(line("something_new")).toContain("Plan usage: no details reported, resets 3:10 PM");
  });

  it("says a used-up limit is used up even with a percent", () => {
    expect(usageSummary(usage({ status: "rejected", utilization: 1 }), NOW, L)).toContain("your 5-hour limit is used up");
  });

  it("leaves out the reset time when it's unknown, and says when it has already passed", () => {
    expect(usageSummary(usage({ resetsAt: undefined }), NOW, L)).toBe(
      "Plan usage: 42% of your 5-hour limit (as of 2:05 PM, from your last Ask)",
    );
    expect(usageSummary(usage({ resetsAt: at(26, 14, 20) }), NOW, L)).toBe(
      "Plan usage: 42% of your 5-hour limit, reset at 2:20 PM (as of 2:05 PM, from your last Ask)",
    );
  });

  it("puts a weekday on a reset or sighting that isn't today", () => {
    const weekly = usage({ kind: "seven_day", resetsAt: at(29, 9, 0), seenAt: at(25, 22, 0) });
    expect(usageSummary(weekly, NOW, L)).toBe(
      "Plan usage: 42% of your weekly limit, resets Tue 9:00 AM (as of Fri 10:00 PM, from your last Ask)",
    );
  });
});

describe("usageWarning (mini chat footer)", () => {
  it("stays quiet while the plan is fine or the status is unknown", () => {
    expect(usageWarning(usage(), NOW, L)).toBeNull();
    expect(usageWarning(usage({ status: "something_new" }), NOW, L)).toBeNull();
  });

  it("warns near the limit, with the as-of time", () => {
    expect(usageWarning(usage({ status: "allowed_warning", utilization: 0.82 }), NOW, L)).toBe(
      "Near your 5-hour limit (82%), resets 3:10 PM · as of 2:05 PM",
    );
    expect(usageWarning(usage({ status: "allowed_warning", utilization: undefined, resetsAt: undefined }), NOW, L)).toBe(
      "Near your 5-hour limit · as of 2:05 PM",
    );
  });

  it("says when the limit is reached", () => {
    expect(usageWarning(usage({ status: "rejected", kind: "seven_day", resetsAt: at(29, 9, 0) }), NOW, L)).toBe(
      "Weekly limit reached, resets Tue 9:00 AM · as of 2:05 PM",
    );
    expect(usageWarning(usage({ status: "rejected", kind: undefined, resetsAt: at(26, 14, 0) }), NOW, L)).toBe(
      "Plan limit reached · as of 2:05 PM",
    );
  });
});
