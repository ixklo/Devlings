import { describe, expect, it } from "vitest";
import { badgeFor, moodClass, MOODS } from "./moodClass";

describe("moodClass", () => {
  it("gives every mood its own class", () => {
    const classes = MOODS.map(moodClass);
    expect(new Set(classes).size).toBe(MOODS.length);
    expect(moodClass("needs_you")).toBe("mood-needs_you");
  });

  it("shows badges only when the pet wants attention", () => {
    expect(badgeFor("needs_you")).toBe("!");
    expect(badgeFor("failed")).toBe("!");
    expect(badgeFor("setup")).toBe("?");
    for (const m of ["working", "done", "listening", "sleeping", "idle"] as const) {
      expect(badgeFor(m)).toBe("");
    }
  });
});
