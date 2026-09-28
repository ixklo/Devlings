#!/usr/bin/env node
// Writes version-free copies of the installers next to the versioned ones, so links of the form
// https://github.com/ixklo/devlings/releases/latest/download/<name> (README, website) keep working
// across releases. Run in release.yml's finalize job after the release assets are downloaded and
// before SHA256SUMS.txt and the attestations are made, so both cover the copies too (the bytes are
// identical, so the same digest verifies either name).
//
// The copies get no .sig: the updater reads latest.json, which gen-latest-json.mjs builds from the
// versioned files' signatures only.
//
// Usage:
//   node scripts/stable-assets.mjs --assets-dir <dir> [--skip macos]
//
// --skip macos: no macOS build to copy (release.yml passes it while the repository is private).
import { copyFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Tauri's installer names are Devlings_<version>_<arch>.<ext> (docs/specs/2026-09-27-devlings-v1.1.md D22.1).
const STABLE = [
  { platform: "windows", name: "Devlings-windows-x64-setup.exe", pattern: /^Devlings_[^_]+_x64-setup\.exe$/ },
  { platform: "linux", name: "Devlings-linux-x86_64.AppImage", pattern: /^Devlings_[^_]+_amd64\.AppImage$/ },
  { platform: "linux", name: "Devlings-linux-amd64.deb", pattern: /^Devlings_[^_]+_amd64\.deb$/ },
  { platform: "macos", name: "Devlings-macos-universal.dmg", pattern: /^Devlings_[^_]+_universal\.dmg$/ },
];

export const STABLE_NAMES = STABLE.map((s) => s.name);

/** For each stable name (minus skipped platforms), the one versioned asset it copies. Throws unless exactly one matches. */
export function planStableCopies(fileNames, { skip = [] } = {}) {
  return STABLE.filter((s) => !skip.includes(s.platform)).map(({ name, pattern }) => {
    const matches = fileNames.filter((f) => pattern.test(f));
    if (matches.length !== 1) {
      throw new Error(`expected exactly one asset for ${name}, found ${matches.length ? matches.join(", ") : "none"}`);
    }
    return { from: matches[0], to: name };
  });
}

export function writeStableCopies(dir, options) {
  const plan = planStableCopies(readdirSync(dir), options);
  for (const { from, to } of plan) copyFileSync(path.join(dir, from), path.join(dir, to));
  return plan;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const arg = (name) => {
    const i = process.argv.indexOf(`--${name}`);
    return i > 0 ? process.argv[i + 1] : undefined;
  };
  const dir = arg("assets-dir");
  if (!dir) {
    console.error("stable-assets: missing --assets-dir");
    process.exit(1);
  }
  const skip = (arg("skip") ?? "").split(",").filter(Boolean);
  try {
    for (const { from, to } of writeStableCopies(dir, { skip })) console.log(`stable-assets: ${from} -> ${to}`);
  } catch (e) {
    console.error(`stable-assets: ${e instanceof Error ? e.message : e}`);
    process.exit(1);
  }
}
