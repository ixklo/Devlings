#!/usr/bin/env node
// Confirms package.json, package-lock.json, src-tauri/Cargo.toml,
// src-tauri/Cargo.lock and src-tauri/tauri.conf.json all agree on the
// version, and optionally that a release tag matches too. Exits non-zero
// on any mismatch or missing value.
//
// Usage: node scripts/check-version.mjs [--tag vX.Y.Z[-rc.N]]
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { getCargoTomlVersion, getCargoLockPackageVersion } from "./lib/cargo-files.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

const args = process.argv.slice(2);
let tag = null;
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--tag") tag = args[i + 1];
}

const versions = {};

try {
  const pkg = readJson(path.join(root, "package.json"));
  versions["package.json"] = pkg.version;
} catch (e) {
  versions["package.json"] = null;
}

try {
  const lock = readJson(path.join(root, "package-lock.json"));
  versions["package-lock.json (root)"] = lock.version;
  versions['package-lock.json (packages[""])'] = lock.packages?.[""]?.version;
} catch (e) {
  versions["package-lock.json (root)"] = null;
  versions['package-lock.json (packages[""])'] = null;
}

try {
  const cargoToml = readFileSync(path.join(root, "src-tauri", "Cargo.toml"), "utf8");
  versions["src-tauri/Cargo.toml"] = getCargoTomlVersion(cargoToml);
} catch (e) {
  versions["src-tauri/Cargo.toml"] = null;
}

try {
  const cargoLock = readFileSync(path.join(root, "src-tauri", "Cargo.lock"), "utf8");
  versions["src-tauri/Cargo.lock"] = getCargoLockPackageVersion(cargoLock, "perch");
} catch (e) {
  versions["src-tauri/Cargo.lock"] = null;
}

try {
  const tauriConf = readJson(path.join(root, "src-tauri", "tauri.conf.json"));
  versions["src-tauri/tauri.conf.json"] = tauriConf.version;
} catch (e) {
  versions["src-tauri/tauri.conf.json"] = null;
}

if (tag) {
  const m = tag.match(/^v(\d+\.\d+\.\d+(?:-rc\.\d+)?)$/);
  if (!m) {
    console.error(`check-version: --tag "${tag}" isn't of the form vX.Y.Z or vX.Y.Z-rc.N`);
    process.exit(1);
  }
  versions["--tag"] = m[1];
}

let ok = true;
const values = new Set();
for (const [source, value] of Object.entries(versions)) {
  console.log(`${value ?? "(missing)"}  ${source}`);
  if (!value) ok = false;
  values.add(value);
}
if (values.size > 1) ok = false;

if (!ok) {
  console.error("check-version: mismatch");
  process.exit(1);
}
console.log(`check-version: OK (${[...values][0]})`);
