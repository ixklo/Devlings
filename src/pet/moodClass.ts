import type { Mood } from "../shared/types";

export const MOODS: Mood[] = ["setup", "needs_you", "failed", "working", "done", "listening", "sleeping", "idle"];

export function moodClass(mood: Mood): string {
  return `mood-${mood}`;
}

export function badgeFor(mood: Mood): string {
  switch (mood) {
    case "needs_you":
    case "failed":
      return "!";
    case "setup":
      return "?";
    default:
      return "";
  }
}
