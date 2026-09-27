// @vitest-environment node
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { STABLE_NAMES, planStableCopies, writeStableCopies } from "./stable-assets.mjs";

const SCRIPTS = path.dirname(fileURLToPath(import.meta.url));

// The asset names v1.0.0 was actually published with.
const RELEASE = [
  "Perch_1.0.0_amd64.deb",
  "Perch_1.0.0_amd64.deb.sig",
  "Perch_1.0.0_amd64.AppImage",
  "Perch_1.0.0_amd64.AppImage.sig",
  "Perch_1.0.0_universal.dmg",
  "Perch_universal.app.tar.gz",
  "Perch_universal.app.tar.gz.sig",
  "Perch_1.0.0_x64-setup.exe",
  "Perch_1.0.0_x64-setup.exe.sig",
];

const dirs = [];
function fixtureDir(names) {
  const dir = mkdtempSync(path.join(tmpdir(), "perch-assets-"));
  dirs.push(dir);
  for (const name of names) writeFileSync(path.join(dir, name), name.endsWith(".sig") ? `sig of ${name}` : `bytes of ${name}`);
  return dir;
}
afterEach(() => {
  while (dirs.length) rmSync(dirs.pop(), { recursive: true, force: true });
});

describe("stable download names", () => {
  it("are exactly the four names the README links to", () => {
    expect(STABLE_NAMES).toEqual([
      "Perch-windows-x64-setup.exe",
      "Perch-macos-universal.dmg",
      "Perch-linux-x86_64.AppImage",
      "Perch-linux-amd64.deb",
    ]);
  });

  it("maps each versioned installer to its stable name, and never a .sig or updater bundle", () => {
    expect(planStableCopies(RELEASE)).toEqual([
      { from: "Perch_1.0.0_x64-setup.exe", to: "Perch-windows-x64-setup.exe" },
      { from: "Perch_1.0.0_universal.dmg", to: "Perch-macos-universal.dmg" },
      { from: "Perch_1.0.0_amd64.AppImage", to: "Perch-linux-x86_64.AppImage" },
      { from: "Perch_1.0.0_amd64.deb", to: "Perch-linux-amd64.deb" },
    ]);
  });

  it("handles release-candidate versions", () => {
    const rc = RELEASE.map((n) => n.replace("1.0.0", "1.1.0-rc.1"));
    expect(planStableCopies(rc).map((c) => c.from)).toContain("Perch_1.1.0-rc.1_x64-setup.exe");
  });

  it("ignores stable copies already in the release (a re-run of the finalize job)", () => {
    expect(planStableCopies([...RELEASE, ...STABLE_NAMES])).toHaveLength(4);
  });

  it("fails when an installer is missing", () => {
    expect(() => planStableCopies(RELEASE.filter((n) => !n.endsWith(".dmg")))).toThrow(/Perch-macos-universal\.dmg/);
  });

  it("fails when two files could be the same installer", () => {
    expect(() => planStableCopies([...RELEASE, "Perch_0.9.0_amd64.deb"])).toThrow(/Perch-linux-amd64\.deb/);
  });

  it("writes byte-identical copies", () => {
    const dir = fixtureDir(RELEASE);
    writeStableCopies(dir);
    for (const { from, to } of planStableCopies(RELEASE)) {
      expect(readFileSync(path.join(dir, to))).toEqual(readFileSync(path.join(dir, from)));
    }
  });
});

describe("gen-latest-json with stable copies present", () => {
  function run(dir) {
    const assets = readdirSync(dir).map((name) => ({ name, url: `https://example.test/v1.0.0/${name}` }));
    const assetsJson = path.join(dir, "..", `${path.basename(dir)}.json`);
    writeFileSync(assetsJson, JSON.stringify({ assets }));
    const out = path.join(dir, "latest.json");
    try {
      execFileSync(process.execPath, [
        path.join(SCRIPTS, "gen-latest-json.mjs"),
        "--tag", "v1.0.0", "--assets-dir", dir, "--assets-json", assetsJson, "--out", out,
      ], { stdio: "pipe" });
      return JSON.parse(readFileSync(out, "utf8"));
    } finally {
      rmSync(assetsJson, { force: true });
    }
  }

  it("still sees one .sig per platform and points the updater at the versioned files", () => {
    const dir = fixtureDir(RELEASE);
    writeStableCopies(dir);
    const latest = run(dir);
    const urls = Object.values(latest.platforms).map((p) => p.url.split("/").pop());
    expect(new Set(urls)).toEqual(new Set([
      "Perch_1.0.0_x64-setup.exe",
      "Perch_universal.app.tar.gz",
      "Perch_1.0.0_amd64.AppImage",
      "Perch_1.0.0_amd64.deb",
    ]));
  });

  it("refuses two signatures for the same platform", () => {
    const dir = fixtureDir([...RELEASE, "Perch-windows-x64-setup.exe", "Perch-windows-x64-setup.exe.sig"]);
    expect(() => run(dir)).toThrow(/windows-x86_64/);
  });
});
