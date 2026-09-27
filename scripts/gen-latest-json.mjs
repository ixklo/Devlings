#!/usr/bin/env node
// Builds the updater's latest.json from the signature files Tauri's
// bundler already produced and uploaded as release assets (one *.sig per
// platform's updater artifact, alongside the artifact itself). Run once,
// in release.yml's finalize job, after all three platform builds have
// uploaded their assets to the same (still-draft) release — building this
// centrally, instead of having each matrix job's tauri-action upload its
// own updater JSON, avoids three parallel jobs racing to read-modify-write
// the same file.
//
// Deliberately doesn't hardcode exact artifact filenames (NSIS's installer
// name, whether an archive is .tar.gz-wrapped, etc.) since those are an
// implementation detail of the tauri-bundler version in use. Instead it
// looks at whatever *.sig files actually got uploaded and classifies each
// by its base filename's extension/substring, which is stable across
// bundler versions.
//
// Usage:
//   node scripts/gen-latest-json.mjs --tag vX.Y.Z --assets-dir <dir>
//     --assets-json <file with {assets:[{name,url}]}> --out <path>
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";

function fail(message) {
  console.error(`gen-latest-json: ${message}`);
  process.exit(1);
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i];
    if (!key?.startsWith("--")) fail(`unexpected argument "${key}"`);
    out[key.slice(2)] = argv[i + 1];
  }
  return out;
}

const args = parseArgs(process.argv.slice(2));
for (const required of ["tag", "assets-dir", "assets-json", "out"]) {
  if (!args[required]) fail(`missing --${required}`);
}

const versionMatch = args.tag.match(/^v(\d+\.\d+\.\d+(?:-rc\.\d+)?)$/);
if (!versionMatch) fail(`--tag "${args.tag}" isn't of the form vX.Y.Z or vX.Y.Z-rc.N`);
const version = versionMatch[1];

const assetsDir = args["assets-dir"];
const { assets } = JSON.parse(readFileSync(args["assets-json"], "utf8"));
const urlByName = new Map(assets.map((a) => [a.name, a.url]));

// Each entry maps a classifier over an artifact's base filename (the *.sig
// file's name with ".sig" removed) to the updater platform key(s) it
// satisfies. macOS ships one universal binary, so its single .app.tar.gz
// covers both Apple Silicon and Intel. The updater looks for
// "{os}-{arch}-{installer}" before "{os}-{arch}", so a .deb install needs its
// own "linux-x86_64-deb" entry; without it, it would fall back to the
// AppImage and refuse it as "not a valid deb package".
const CLASSIFIERS = [
  { test: (name) => /\.(exe|msi)$/i.test(name), keys: ["windows-x86_64"] },
  { test: (name) => /\.app\.tar\.gz$/i.test(name), keys: ["darwin-x86_64", "darwin-aarch64"] },
  { test: (name) => /\.deb$/i.test(name), keys: ["linux-x86_64-deb"] },
  { test: (name) => /appimage/i.test(name), keys: ["linux-x86_64"] },
];

const sigFiles = readdirSync(assetsDir).filter((f) => f.endsWith(".sig"));
if (sigFiles.length === 0) fail(`no *.sig files found in ${assetsDir}`);

const platforms = {};
const unrecognized = [];

for (const sigFile of sigFiles) {
  const baseName = sigFile.slice(0, -".sig".length);
  const classifier = CLASSIFIERS.find((c) => c.test(baseName));
  if (!classifier) {
    unrecognized.push(sigFile);
    continue;
  }
  const url = urlByName.get(baseName);
  if (!url) fail(`found ${sigFile} but no matching release asset named "${baseName}"`);
  const signature = readFileSync(path.join(assetsDir, sigFile), "utf8").trim();
  for (const key of classifier.keys) {
    platforms[key] = { signature, url };
  }
}

if (unrecognized.length > 0) {
  fail(`couldn't classify these signature files by platform: ${unrecognized.join(", ")}`);
}

const REQUIRED_KEYS = ["windows-x86_64", "darwin-x86_64", "darwin-aarch64", "linux-x86_64", "linux-x86_64-deb"];
const missing = REQUIRED_KEYS.filter((k) => !platforms[k]);
if (missing.length > 0) {
  fail(`missing updater platform(s): ${missing.join(", ")} (found sig files: ${sigFiles.join(", ")})`);
}

const latest = {
  version,
  notes: `Perch ${version}. See the GitHub release for details.`,
  pub_date: new Date().toISOString(),
  platforms,
};

writeFileSync(args.out, `${JSON.stringify(latest, null, 2)}\n`);
console.log(`gen-latest-json: wrote ${args.out} for ${Object.keys(platforms).length} platform(s)`);
