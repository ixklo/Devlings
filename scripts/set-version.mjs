#!/usr/bin/env node
// Sets Devlings' version everywhere it's recorded: package.json, package-lock.json,
// src-tauri/Cargo.toml, src-tauri/Cargo.lock (the `perch` entry) and
// src-tauri/tauri.conf.json. Run `npm run version:check` after to confirm.
//
// Every file is read and validated (the version line/field is confirmed to
// exist) before anything is written, so a file this script doesn't
// recognize aborts the whole run rather than leaving the version bumped in
// some files and not others.
//
// Usage: node scripts/set-version.mjs <x.y.z[-rc.N]>
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { getCargoTomlVersion, setCargoTomlVersion, getCargoLockPackageVersion, setCargoLockPackageVersion } from "./lib/cargo-files.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const VERSION_RE = /^\d+\.\d+\.\d+(?:-rc\.\d+)?$/;

function fail(message) {
  console.error(`set-version: ${message}`);
  process.exit(1);
}

const version = process.argv[2];
if (!version) fail("usage: set-version.mjs <x.y.z[-rc.N]>");
if (!VERSION_RE.test(version)) {
  fail(`"${version}" isn't x.y.z or x.y.z-rc.N (e.g. 1.0.0 or 1.0.0-rc.1)`);
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function writeJson(file, data) {
  writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`);
}

const pkgPath = path.join(root, "package.json");
const lockPath = path.join(root, "package-lock.json");
const cargoTomlPath = path.join(root, "src-tauri", "Cargo.toml");
const cargoLockPath = path.join(root, "src-tauri", "Cargo.lock");
const tauriConfPath = path.join(root, "src-tauri", "tauri.conf.json");

// --- Validate every file first. Nothing is written until every read below
// has succeeded, so a missing file or section aborts before any file is
// touched, rather than leaving the version bumped in some files and not
// others. ---

let pkg, lock, cargoTomlText, cargoLockText, tauriConf;
try {
  pkg = readJson(pkgPath);
  lock = readJson(lockPath);
  if (!lock.packages?.[""]) throw new Error(`package-lock.json: packages[""] entry not found`);

  cargoTomlText = readFileSync(cargoTomlPath, "utf8");
  getCargoTomlVersion(cargoTomlText); // throws if [package] version isn't found

  cargoLockText = readFileSync(cargoLockPath, "utf8");
  getCargoLockPackageVersion(cargoLockText, "perch"); // throws if the perch entry isn't found

  tauriConf = readJson(tauriConfPath);
} catch (e) {
  fail(`validation failed, nothing was written: ${e.message}`);
}

// --- Everything validated: now write every file. ---

pkg.version = version;
writeJson(pkgPath, pkg);

lock.version = version;
lock.packages[""].version = version;
writeJson(lockPath, lock);

writeFileSync(cargoTomlPath, setCargoTomlVersion(cargoTomlText, version));
writeFileSync(cargoLockPath, setCargoLockPackageVersion(cargoLockText, "perch", version));

tauriConf.version = version;
writeJson(tauriConfPath, tauriConf);

console.log(
  `set-version: ${version} written to:\n  package.json\n  package-lock.json\n  src-tauri/Cargo.toml\n  src-tauri/Cargo.lock\n  src-tauri/tauri.conf.json`,
);
