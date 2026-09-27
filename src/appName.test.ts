import { describe, expect, it } from "vitest";
import indexHtml from "../index.html?raw";

/**
 * The app is called Devlings (docs/specs/2026-09-27-devlings-v1.1.md). "Perch" is now only the teal
 * bird, one of the bundled pets. Every line of the frontend's own source (tests excluded) that still
 * says "Perch" must be listed here, and each one is the bird, not the app.
 */
const ALLOWED: Record<string, string[]> = {
  // The browser preview's pet list, its default pet name, and its copy of the backend's rename rule
  // (a pet still called "Perch", the name every install starts with, takes the new pet's name).
  "./shared/mock.ts": [
    `{ id: "perch", displayName: "Perch", description: "A teal songbird that keeps an eye on your sessions.", source: "bundled" },`,
    `petName: "Perch",`,
    `if (c.petName === "Perch" || c.petName === old?.displayName) c.petName = next.displayName;`,
  ],
};

const sources = import.meta.glob<string>(["./**/*.{ts,tsx,css}", "!./**/*.test.{ts,tsx}", "!./test/**"], {
  query: "?raw",
  import: "default",
  eager: true,
});

function oldNameLines(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.includes("Perch") || l.includes("yeetstick") || l.includes("github.com/ixklo/perch"));
}

describe("the app's name", () => {
  it("is Devlings everywhere the frontend shows it", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(20);
    const offenders: string[] = [];
    for (const [file, text] of Object.entries(sources)) {
      for (const line of oldNameLines(text)) {
        if (!(ALLOWED[file] ?? []).includes(line)) offenders.push(`${file}: ${line}`);
      }
    }
    for (const line of oldNameLines(indexHtml)) offenders.push(`index.html: ${line}`);
    expect(offenders).toEqual([]);
  });

  it("keeps every allowed line real", () => {
    for (const [file, lines] of Object.entries(ALLOWED)) {
      const found = oldNameLines(sources[file] ?? "");
      for (const line of lines) expect(found, file).toContain(line);
    }
  });

  it("titles the page Devlings", () => {
    expect(indexHtml).toContain("<title>Devlings</title>");
  });
});
