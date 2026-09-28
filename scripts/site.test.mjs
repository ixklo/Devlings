// @vitest-environment node
// The README and the website: download links, images and the anchors the app links to.
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { STABLE_NAMES } from "./stable-assets.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(path.join(ROOT, p), "utf8");
const readme = read("README.md");
const site = read("site/index.html");
const DOWNLOAD = /https:\/\/github\.com\/ixklo\/devlings\/releases\/latest\/download\/([^"')\s]+)/g;
// Every platform is built now that the repository is public (macOS was left out while it was private).
const BUILT = STABLE_NAMES;

/** Width and height of a PNG or GIF, from its header. */
function size(file) {
  const b = readFileSync(file);
  if (b.subarray(0, 8).equals(Buffer.from("\x89PNG\r\n\x1a\n", "latin1"))) return [b.readUInt32BE(16), b.readUInt32BE(20)];
  if (b.subarray(0, 3).toString("latin1") === "GIF") return [b.readUInt16LE(6), b.readUInt16LE(8)];
  throw new Error(`${file}: not a PNG or GIF`);
}

describe.each([
  ["README.md", readme],
  ["site/index.html", site],
])("download links in %s", (_, text) => {
  it("use the stable names of the installers that are built, and nothing else", () => {
    const names = [...text.matchAll(DOWNLOAD)].map((m) => m[1]);
    expect(new Set(names)).toEqual(new Set(BUILT));
  });

  it("label macOS as beta", () => {
    expect(text).toMatch(/macOS[^\n]*beta/);
  });
});

describe("README", () => {
  it("keeps the Cost section the app's onboarding links to (#cost)", () => {
    expect(read("src/settings/Onboarding.tsx")).toContain("#cost`");
    expect(readme).toMatch(/^## Cost$/m);
  });

  it("only shows images that exist", () => {
    const images = [...readme.matchAll(/(?:src|srcset)="(docs\/[^"]+)"/g)].map((m) => m[1]);
    expect(images.length).toBeGreaterThan(10);
    for (const image of images) expect(existsSync(path.join(ROOT, image)), image).toBe(true);
  });

  it("has a dark version of every screenshot it shows", () => {
    const light = [...readme.matchAll(/<img src="docs\/screenshots\/([a-z]+)\.(png|gif)"/g)].map((m) => `${m[1]}.${m[2]}`);
    for (const name of light.filter((n) => n !== "pets.gif")) {
      expect(readme, name).toContain(`srcset="docs/screenshots/${name.replace(".", "-dark.")}"`);
    }
  });

  it("links to the guide for making a pet", () => {
    expect(readme).toContain("(docs/making-pets.md)");
    expect(existsSync(path.join(ROOT, "docs", "making-pets.md"))).toBe(true);
  });
});

describe("website", () => {
  // When it's published, img/ holds docs/screenshots/*, docs/pets/lineup.png and docs/social-preview.png.
  const source = (name) =>
    [path.join(ROOT, "docs", "screenshots", name), path.join(ROOT, "docs", "pets", name), path.join(ROOT, "docs", name)].find(
      (p) => existsSync(p) && (name !== "lineup.png" || p.includes("pets")),
    );

  it("only uses images from docs/", () => {
    const images = [...site.matchAll(/(?:src|srcset|content)="(?:https:\/\/ixklo\.github\.io\/devlings\/)?img\/([^"]+)"/g)].map((m) => m[1]);
    expect(images.length).toBeGreaterThan(5);
    for (const name of images) expect(source(name), `img/${name}`).toBeDefined();
  });

  it("gives each image the size it's drawn at (half the 2x file)", () => {
    for (const m of site.matchAll(/<img src="img\/([^"]+)" width="(\d+)" height="(\d+)"/g)) {
      const [w, h] = size(source(m[1]));
      expect([Number(m[2]), Number(m[3])], m[1]).toEqual([Math.round(w / 2), Math.round(h / 2)]);
    }
  });

  it("loads nothing from other sites: no scripts, style sheets, fonts or remote images", () => {
    expect(site).not.toMatch(/<script/);
    expect(site).not.toMatch(/<link[^>]+rel="stylesheet"/);
    expect(site).not.toMatch(/<(img|source|iframe)[^>]+src(set)?="https?:/);
    expect(site).not.toMatch(/@import|url\(/);
  });

  it("says it isn't affiliated with Anthropic", () => {
    expect(site).toContain("not affiliated with Anthropic");
    expect(readme).toContain("not affiliated with Anthropic");
  });
});
