#!/usr/bin/env node
// Sets Perch's version everywhere it's recorded: package.json, package-lock.json,
// src-tauri/Cargo.toml, src-tauri/Cargo.lock (the `perch` entry) and
// src-tauri/tauri.conf.json. Run `npm run version:check` after to confirm.
//
// Usage: node scripts/set-version.mjs <x.y.z[-rc.N]>
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

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

const touched = [];

// package.json
const pkgPath = path.join(root, "package.json");
const pkg = readJson(pkgPath);
pkg.version = version;
writeJson(pkgPath, pkg);
touched.push("package.json");

// package-lock.json: the root version and the packages[""] entry
const lockPath = path.join(root, "package-lock.json");
const lock = readJson(lockPath);
lock.version = version;
if (lock.packages?.[""]) lock.packages[""].version = version;
writeJson(lockPath, lock);
touched.push("package-lock.json");

// src-tauri/Cargo.toml: the [package] version
const cargoTomlPath = path.join(root, "src-tauri", "Cargo.toml");
const cargoToml = readFileSync(cargoTomlPath, "utf8");
const cargoTomlRe = /(\[package\][^[]*?\r?\nversion\s*=\s*)"[^"]*"/;
if (!cargoTomlRe.test(cargoToml)) fail("couldn't find [package] version in src-tauri/Cargo.toml");
writeFileSync(cargoTomlPath, cargoToml.replace(cargoTomlRe, (_m, pre) => `${pre}"${version}"`));
touched.push("src-tauri/Cargo.toml");

// src-tauri/Cargo.lock: the `perch` package entry
const cargoLockPath = path.join(root, "src-tauri", "Cargo.lock");
const cargoLock = readFileSync(cargoLockPath, "utf8");
const cargoLockRe = /(\[\[package\]\]\r?\nname = "perch"\r?\nversion = )"[^"]*"/;
if (!cargoLockRe.test(cargoLock)) fail("couldn't find the perch entry in src-tauri/Cargo.lock");
writeFileSync(cargoLockPath, cargoLock.replace(cargoLockRe, (_m, pre) => `${pre}"${version}"`));
touched.push("src-tauri/Cargo.lock");

// src-tauri/tauri.conf.json
const tauriConfPath = path.join(root, "src-tauri", "tauri.conf.json");
const tauriConf = readJson(tauriConfPath);
tauriConf.version = version;
writeJson(tauriConfPath, tauriConf);
touched.push("src-tauri/tauri.conf.json");

console.log(`set-version: ${version} written to:\n  ${touched.join("\n  ")}`);
