# Tauri CLI 2.12.1 dependency review

- Review date: 2026-10-09.
- Change: [PR #12](https://github.com/Sealdot/clipriva-source/pull/12),
  `@tauri-apps/cli` 2.11.4 -> 2.12.1 and its 11 platform packages.
- Dependency source commit: `fc7328689935d9957c648bd171fd31f965d6f461`.
  The follow-up changes only audit documentation and portable evidence. The lockfile hashes
  below bind this review to the dependency set, independent of later documentation commits.
- Tools: Node.js 24.14.0, pnpm 11.9.0, Rust/Cargo 1.97.1; local `aarch64-apple-darwin` host.

## Scope and results

| Input | Components | Missing licenses | Restricted without a permissive alternative | Unsupported expressions |
| --- | ---: | ---: | ---: | ---: |
| Current-platform installed Node metadata, including build/test dependencies | 127 | 0 | 0 | 0 |
| Reachable all-feature macOS ARM64 Rust registry graph | 317 | 0 | 0 | 0 |
| Reachable all-feature, all-platform Rust registry graph | 518 | 0 | 0 | 0 |

These are fresh metadata results, not reused historical counts. The current-platform Node
review does not claim coverage of every optional Node platform package. As a separate check,
all 12 changed CLI package/version entries were queried from the public npm registry: each
reports `Apache-2.0 OR MIT`, and each registry `dist.integrity` matches `pnpm-lock.yaml`.
This checks the changed platform packages' declarations, not their embedded binary contents.

Rust `r-efi` 5.3.0 and 6.0.0 mention LGPL as an OR alternative alongside MIT and Apache-2.0;
select the permissive alternative when preparing notices. They are absent from the selected
macOS ARM64 graph. The reviewer preserves original declarations and records 23 legacy
slash-to-OR normalizations in the target graph and 34 in the all-platform graph. No missing,
unsupported, or restricted-without-alternative declaration was found in the reviewed inputs.

The fresh production-only npm audit returned zero reported vulnerabilities. This does not
scan development-only packages, the CLI executable's embedded Rust dependencies, or prove
that all vulnerabilities are known. RustSec results and native builds are separately reported
by CI. The Rust lockfile and application dependencies are unchanged by this upgrade.

## Inspectable evidence

- [macOS ARM64 license report](tauri-cli-2.12.1-evidence/macos-arm64.json).
- [All-platform Rust license report](tauri-cli-2.12.1-evidence/all-platforms.json).
- [All changed npm CLI platform declarations and integrity checks](tauri-cli-2.12.1-evidence/npm-cli-platform-packages.json).

Lockfile SHA-256 values:

```text
pnpm-lock.yaml: 1dc7358f94418f456427698b57bd59e3d7b3733d43a7017dabc20046fc07b39c
src-tauri/Cargo.lock: 3b46a8006f79e76aa99ed7baf1ac27d714ce56154f2055071513dbeb94d58a28
```

The committed reports contain component declarations and relative lockfile names, without raw
Cargo/pnpm metadata or local personal paths. GitHub CI regenerates portable reports on the
final candidate in the `source-evidence-<commit>` artifact. Use the exact run's results on the
[PR checks page](https://github.com/Sealdot/clipriva-source/pull/12/checks) for final build/test,
RustSec and runner-specific evidence; these metadata reports are not a substitute for those
checks, a binary NOTICE review, signing, notarization, or real-machine acceptance.

## Reproduction

Run from the source root using its pinned Node/pnpm/Rust versions. Keep temporary raw metadata
outside the repository, because it contains local filesystem paths.

```bash
review_dir=$(mktemp -d)
pnpm install --frozen-lockfile
pnpm licenses list --json > "$review_dir/node-licenses.json"
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --all-features \
  --filter-platform aarch64-apple-darwin --format-version 1 > "$review_dir/cargo-target.json"
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --all-features \
  --format-version 1 > "$review_dir/cargo-all.json"
python3 scripts/review-dependency-licenses.py \
  --node-metadata "$review_dir/node-licenses.json" --cargo-metadata "$review_dir/cargo-target.json" \
  --target aarch64-apple-darwin --commit "$(git rev-parse HEAD)" --output "$review_dir/target"
python3 scripts/review-dependency-licenses.py \
  --node-metadata "$review_dir/node-licenses.json" --cargo-metadata "$review_dir/cargo-all.json" \
  --target all-platforms --commit "$(git rev-parse HEAD)" --output "$review_dir/all-platforms"
pnpm audit --prod --registry=https://registry.npmjs.org --json
```

For each entry in the CLI registry report, fetch its `source` URL, confirm the exact name and
version, read the declared `license`, and compare `dist.integrity` with the corresponding
locked package entry. These public metadata checks require no application network feature.

## Privacy, security and compatibility impact

This upgrade changes development/build tooling and audit records. It adds no application
network route, telemetry, stored data type, or OS permission. Upstream 2.12.0 also changes
Android templates, Windows bundling and updater-signature tooling; this macOS project's
configuration does not enable those new options. Its native build and test results must
still pass for the final candidate. This change does not establish binary-release readiness.
