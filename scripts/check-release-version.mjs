import { readFileSync } from "node:fs";

const root = new URL("..", import.meta.url);
const packageJson = JSON.parse(readFileSync(new URL("package.json", root), "utf8"));
const tauriConfig = JSON.parse(readFileSync(new URL("src-tauri/tauri.conf.json", root), "utf8"));
const cargoToml = readFileSync(new URL("src-tauri/Cargo.toml", root), "utf8");
const cargoLock = readFileSync(new URL("src-tauri/Cargo.lock", root), "utf8");
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const cargoPackage = cargoLock.match(
  /\[\[package\]\]\nname = "clipriva"\nversion = "([^"]+)"/,
)?.[1];
const versions = [packageJson.version, tauriConfig.version, cargoVersion, cargoPackage];

if (versions.some((version) => !version)) {
  throw new Error(
    "Release version is missing from package.json, tauri.conf.json, Cargo.toml, or Cargo.lock.",
  );
}

if (new Set(versions).size !== 1) {
  throw new Error(`Release versions must match; found ${versions.join(", ")}.`);
}

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(versions[0])) {
  throw new Error(`Release version is not valid SemVer: ${versions[0]}.`);
}

console.log(
  `Release version ${versions[0]} is consistent across package.json, Cargo.toml, Cargo.lock, and Tauri.`,
);
