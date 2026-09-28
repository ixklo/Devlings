// @vitest-environment node
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { STABLE_NAMES, planStableCopies, writeStableCopies } from "./stable-assets.mjs";

const SCRIPTS = path.dirname(fileURLToPath(import.meta.url));

// The assets v1.2.0 was published with (a private-repo release: no macOS build), plus what a public
// release adds for macOS (docs/specs/2026-09-27-devlings-v1.1.md D22.1).
const RELEASE = [
  "Devlings_1.2.0_amd64.AppImage",
  "Devlings_1.2.0_amd64.AppImage.sig",
  "Devlings_1.2.0_amd64.deb",
  "Devlings_1.2.0_amd64.deb.sig",
  "Devlings_1.2.0_x64-setup.exe",
  "Devlings_1.2.0_x64-setup.exe.sig",
];
const MACOS = ["Devlings_1.2.0_universal.dmg", "Devlings_universal.app.tar.gz", "Devlings_universal.app.tar.gz.sig"];
const PUBLIC = [...RELEASE, ...MACOS];

const dirs = [];
function fixtureDir(names) {
  const dir = mkdtempSync(path.join(tmpdir(), "devlings-assets-"));
  dirs.push(dir);
  for (const name of names) writeFileSync(path.join(dir, name), name.endsWith(".sig") ? `sig of ${name}` : `bytes of ${name}`);
  return dir;
}
afterEach(() => {
  while (dirs.length) rmSync(dirs.pop(), { recursive: true, force: true });
});

describe("stable download names", () => {
  it("are exactly the four names the README and website link to", () => {
    expect(STABLE_NAMES).toEqual([
      "Devlings-windows-x64-setup.exe",
      "Devlings-linux-x86_64.AppImage",
      "Devlings-linux-amd64.deb",
      "Devlings-macos-universal.dmg",
    ]);
  });

  it("map each versioned installer to its stable name, and never a .sig or the updater bundle", () => {
    expect(planStableCopies(PUBLIC)).toEqual([
      { from: "Devlings_1.2.0_x64-setup.exe", to: "Devlings-windows-x64-setup.exe" },
      { from: "Devlings_1.2.0_amd64.AppImage", to: "Devlings-linux-x86_64.AppImage" },
      { from: "Devlings_1.2.0_amd64.deb", to: "Devlings-linux-amd64.deb" },
      { from: "Devlings_1.2.0_universal.dmg", to: "Devlings-macos-universal.dmg" },
    ]);
  });

  it("leave macOS out when it isn't built (--skip macos, while the repository is private)", () => {
    expect(planStableCopies(RELEASE, { skip: ["macos"] }).map((c) => c.to)).toEqual([
      "Devlings-windows-x64-setup.exe",
      "Devlings-linux-x86_64.AppImage",
      "Devlings-linux-amd64.deb",
    ]);
  });

  it("need the macOS installer unless told to skip it", () => {
    expect(() => planStableCopies(RELEASE)).toThrow(/Devlings-macos-universal\.dmg/);
  });

  it("handle release-candidate versions", () => {
    const rc = PUBLIC.map((n) => n.replace("1.2.0", "1.3.0-rc.1"));
    expect(planStableCopies(rc).map((c) => c.from)).toContain("Devlings_1.3.0-rc.1_x64-setup.exe");
  });

  it("ignore stable copies already in the release (a re-run of the finalize job)", () => {
    expect(planStableCopies([...PUBLIC, ...STABLE_NAMES])).toHaveLength(4);
  });

  it("fail when an installer is missing", () => {
    expect(() => planStableCopies(PUBLIC.filter((n) => !n.endsWith(".deb")))).toThrow(/Devlings-linux-amd64\.deb/);
  });

  it("fail when two files could be the same installer", () => {
    expect(() => planStableCopies([...PUBLIC, "Devlings_1.1.0_amd64.deb"])).toThrow(/Devlings-linux-amd64\.deb/);
  });

  it("write byte-identical copies", () => {
    const dir = fixtureDir(PUBLIC);
    const plan = writeStableCopies(dir);
    expect(plan).toHaveLength(4);
    for (const { from, to } of plan) expect(readFileSync(path.join(dir, to))).toEqual(readFileSync(path.join(dir, from)));
  });

  it("run from the command line as release.yml does", () => {
    const dir = fixtureDir(RELEASE);
    const out = execFileSync(process.execPath, [path.join(SCRIPTS, "stable-assets.mjs"), "--assets-dir", dir, "--skip", "macos"], {
      encoding: "utf8",
    });
    expect(out).toContain("Devlings_1.2.0_x64-setup.exe -> Devlings-windows-x64-setup.exe");
    expect(readdirSync(dir).filter((n) => n.startsWith("Devlings-")).sort()).toEqual([
      "Devlings-linux-amd64.deb",
      "Devlings-linux-x86_64.AppImage",
      "Devlings-windows-x64-setup.exe",
    ]);
  });
});

describe("gen-latest-json with the stable copies present", () => {
  function run(dir, extra = []) {
    const assets = readdirSync(dir).map((name) => ({ name, url: `https://example.test/v1.2.0/${name}` }));
    const assetsJson = path.join(dir, "..", `${path.basename(dir)}.json`);
    writeFileSync(assetsJson, JSON.stringify({ assets }));
    const out = path.join(dir, "latest.json");
    try {
      execFileSync(
        process.execPath,
        [path.join(SCRIPTS, "gen-latest-json.mjs"), "--tag", "v1.2.0", "--assets-dir", dir, "--assets-json", assetsJson, "--out", out, ...extra],
        { stdio: "pipe" },
      );
      return JSON.parse(readFileSync(out, "utf8"));
    } finally {
      rmSync(assetsJson, { force: true });
    }
  }

  it("still points the updater at the versioned files and their signatures", () => {
    const dir = fixtureDir(PUBLIC);
    writeStableCopies(dir);
    const latest = run(dir);
    expect(Object.fromEntries(Object.entries(latest.platforms).map(([k, p]) => [k, p.url.split("/").pop()]))).toEqual({
      "windows-x86_64": "Devlings_1.2.0_x64-setup.exe",
      "darwin-x86_64": "Devlings_universal.app.tar.gz",
      "darwin-aarch64": "Devlings_universal.app.tar.gz",
      "linux-x86_64": "Devlings_1.2.0_amd64.AppImage",
      "linux-x86_64-deb": "Devlings_1.2.0_amd64.deb",
    });
    expect(latest.platforms["windows-x86_64"].signature).toBe("sig of Devlings_1.2.0_x64-setup.exe.sig");
  });

  it("works for a private-repo release without macOS", () => {
    const dir = fixtureDir(RELEASE);
    writeStableCopies(dir, { skip: ["macos"] });
    const latest = run(dir, ["--require", "windows-x86_64,linux-x86_64,linux-x86_64-deb"]);
    expect(Object.keys(latest.platforms).sort()).toEqual(["linux-x86_64", "linux-x86_64-deb", "windows-x86_64"]);
  });

  it("refuses two signatures for the same platform", () => {
    const dir = fixtureDir([...PUBLIC, "Devlings-windows-x64-setup.exe", "Devlings-windows-x64-setup.exe.sig"]);
    expect(() => run(dir)).toThrow(/windows-x86_64/);
  });
});

describe("release.yml", () => {
  const workflow = readFileSync(path.join(SCRIPTS, "..", ".github", "workflows", "release.yml"), "utf8");
  const at = (text) => {
    const i = workflow.indexOf(text);
    expect(i, text).toBeGreaterThan(-1);
    return i;
  };

  it("makes the stable copies after latest.json and before the checksums and attestations", () => {
    const copies = at("node scripts/stable-assets.mjs --assets-dir release-assets");
    expect(at("node scripts/gen-latest-json.mjs")).toBeLessThan(copies);
    expect(copies).toBeLessThan(at("sha256sum * > SHA256SUMS.txt"));
    expect(copies).toBeLessThan(at("actions/attest-build-provenance"));
  });

  it("skips the macOS copy exactly when it skips the macOS build", () => {
    expect(workflow).toContain("github.event.repository.private && '--skip macos'");
  });

  it("uploads the copies with the checksums", () => {
    expect(workflow).toMatch(/gh release upload "\$TAG" release-assets\/latest\.json release-assets\/SHA256SUMS\.txt release-assets\/Devlings-\*/);
  });
});
