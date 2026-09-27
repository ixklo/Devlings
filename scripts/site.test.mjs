// @vitest-environment node
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { STABLE_NAMES } from "./stable-assets.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(path.join(ROOT, p), "utf8");
const site = read("site/index.html");
const readme = read("README.md");
const DOWNLOAD = /https:\/\/github\.com\/yeetstick\/perch\/releases\/latest\/download\/([^"')\s]+)/g;

describe("website", () => {
  it("states the current stable version in its structured data", () => {
    const { version } = JSON.parse(read("package.json"));
    const ld = JSON.parse(site.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/)[1]);
    expect(ld["@type"]).toBe("SoftwareApplication");
    // A release candidate doesn't change what the site offers; a stable bump must.
    if (!version.includes("-")) expect(ld.softwareVersion, "update softwareVersion in site/index.html").toBe(version);
  });

  it("only uses images the Pages workflow copies in (docs/screenshots and the social preview)", () => {
    const images = [...site.matchAll(/(?:src|srcset|href|content)="(?:https:\/\/yeetstick\.github\.io\/perch\/)?img\/([^"]+)"/g)].map((m) => m[1]);
    expect(images.length).toBeGreaterThan(0);
    for (const name of images) {
      const found = existsSync(path.join(ROOT, "docs", "screenshots", name)) || name === "social-preview.png";
      expect(found, `img/${name}`).toBe(true);
    }
  });

  it("loads nothing from other origins except links", () => {
    expect(site).not.toMatch(/<script(?![^>]*application\/ld\+json)/);
    expect(site).not.toMatch(/<link[^>]+rel="stylesheet"/);
    expect(site).not.toMatch(/<(img|source|iframe)[^>]+src="https?:/);
  });
});

describe.each([["README.md", readme], ["site/index.html", site]])("download links in %s", (_, text) => {
  it("use each stable name, and nothing else", () => {
    const names = [...text.matchAll(DOWNLOAD)].map((m) => m[1]);
    expect(new Set(names)).toEqual(new Set(STABLE_NAMES));
  });
});
