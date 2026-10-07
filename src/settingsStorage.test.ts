import { describe, expect, it } from "vitest";
import indexHtml from "../index.html?raw";

/**
 * Settings live in the user's profile: the backend saves them to config.json and projects.json in the per-user
 * app-data folder (src-tauri/src/store.rs). The webview's own storage (cookies, localStorage and the rest) is
 * browser data kept apart from those files, so the frontend never keeps anything there: anything a page wants to
 * remember goes through a backend command instead.
 */
const WEBVIEW_STORAGE = /\b(?:localStorage|sessionStorage|indexedDB|document\.cookie|cookieStore)\b/;

const sources = import.meta.glob<string>(["./**/*.{ts,tsx}", "!./**/*.test.{ts,tsx}", "!./test/**"], {
  query: "?raw",
  import: "default",
  eager: true,
});

function storageLines(file: string, text: string): string[] {
  return text
    .split(/\r?\n/)
    .filter((l) => WEBVIEW_STORAGE.test(l))
    .map((l) => `${file}: ${l.trim()}`);
}

describe("where settings are stored", () => {
  it("never in cookies or the webview's storage", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(20);
    const offenders = Object.entries(sources).flatMap(([file, text]) => storageLines(file, text));
    offenders.push(...storageLines("index.html", indexHtml));
    expect(offenders).toEqual([]);
  });

  it("catches each kind of webview storage", () => {
    for (const line of [
      `localStorage.setItem("petScale", "0.6")`,
      `window.sessionStorage.getItem("x")`,
      `document.cookie = "a=b"`,
      `indexedDB.open("devlings")`,
      `await cookieStore.set("a", "b")`,
    ]) {
      expect(storageLines("x.ts", line), line).toHaveLength(1);
    }
  });
});
