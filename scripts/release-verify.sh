#!/bin/sh
set -eu

pnpm check

(
  cd src-tauri
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
)
