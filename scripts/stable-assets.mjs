#!/usr/bin/env node
// Writes version-free copies of the four installers next to the versioned
// ones, so https://github.com/yeetstick/perch/releases/latest/download/<name>
// links (README, website) keep working across releases. Run in
// release.yml's finalize job after the release assets are downloaded and
// before SHA256SUMS.txt and the attestations are made, so both cover the
// copies too (identical bytes, so the same digest verifies either name).
//
// The copies get no .sig: the updater reads latest.json, which
// gen-latest-json.mjs builds from the versioned files' signatures only.
//
// Usage:
//   node scripts/stable-assets.mjs --assets-dir <dir>
import { copyFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const STABLE = [
  { name: "Perch-windows-x64-setup.exe", pattern: /^Perch_[^_]+_x64-setup\.exe$/ },
  { name: "Perch-macos-universal.dmg", pattern: /^Perch_[^_]+_universal\.dmg$/ },
  { name: "Perch-linux-x86_64.AppImage", pattern: /^Perch_[^_]+_amd64\.AppImage$/ },
  { name: "Perch-linux-amd64.deb", pattern: /^Perch_[^_]+_amd64\.deb$/ },
];

export const STABLE_NAMES = STABLE.map((s) => s.name);

/** For each stable name, the one versioned asset it copies. Throws unless exactly one matches. */
export function planStableCopies(fileNames) {
  return STABLE.map(({ name, pattern }) => {
    const matches = fileNames.filter((f) => pattern.test(f));
    if (matches.length !== 1) {
      throw new Error(`expected exactly one asset for ${name}, found ${matches.length ? matches.join(", ") : "none"}`);
    }
    return { from: matches[0], to: name };
  });
}

export function writeStableCopies(dir) {
  const plan = planStableCopies(readdirSync(dir));
  for (const { from, to } of plan) copyFileSync(path.join(dir, from), path.join(dir, to));
  return plan;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const i = process.argv.indexOf("--assets-dir");
  const dir = i > 0 ? process.argv[i + 1] : undefined;
  if (!dir) {
    console.error("stable-assets: missing --assets-dir");
    process.exit(1);
  }
  try {
    for (const { from, to } of writeStableCopies(dir)) console.log(`stable-assets: ${from} -> ${to}`);
  } catch (e) {
    console.error(`stable-assets: ${e instanceof Error ? e.message : e}`);
    process.exit(1);
  }
}
