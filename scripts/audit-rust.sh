#!/usr/bin/env bash
set -euo pipefail
# Development tooling only; no app networking or shipped scanner.
case "$(uname -m)" in
  arm64) arch=aarch64; digest=ec7ca4263769593df4d909be85b94a6b79efa2897be5d2bb8ebd516e823175af ;;
  x86_64) arch=x86_64; digest=847831323de932155b226ab60ee4a180e13e5d007a019f0d4b7b4d89a6de2ab2 ;;
  *) exit 1 ;;
esac
tool_dir="$RUNNER_TEMP/clipriva-audit-tool"
evidence="$RUNNER_TEMP/clipriva-source-evidence"
mkdir -p "$tool_dir" "$evidence"
asset="cargo-audit-$arch-apple-darwin-v0.22.2"
curl --fail --location --retry 3 "https://github.com/rustsec/rustsec/releases/download/cargo-audit/v0.22.2/$asset.tgz" -o "$tool_dir/tool.tgz"
printf '%s  %s\n' "$digest" "$tool_dir/tool.tgz" | shasum -a 256 -c -
tar -xzf "$tool_dir/tool.tgz" -C "$tool_dir"
git clone --depth 1 https://github.com/RustSec/advisory-db.git "$tool_dir/advisory-db"
{
  "$tool_dir/$asset/cargo-audit" --version
  printf 'tool_sha256=%s\n' "$digest"
  printf 'database_commit=%s\n' "$(git -C "$tool_dir/advisory-db" rev-parse HEAD)"
} > "$evidence/rust-audit-provenance.txt"
# Raw findings remain temporary; publish only the reviewed aggregate and provenance.
"$tool_dir/$asset/cargo-audit" audit --file src-tauri/Cargo.lock \
  --db "$tool_dir/advisory-db" --no-fetch --json > "$tool_dir/rust-audit.json" 2> "$tool_dir/rust-audit-errors.txt"
python3 scripts/review-rust-advisories.py "$evidence" "$tool_dir"
