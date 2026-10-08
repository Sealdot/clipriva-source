# Public source development preview

The maintainer authorized source-only publication in `Sealdot/clipriva-source`, based on
`b4ec2fe443440b88a602288f1b799735096c8088`. The application version remains
`2.0.0-alpha.1`; the source tag is `v2.0.0-alpha.1-source-preview.1`. No feature,
permission, storage, runtime network boundary, application source, test assertion or asset changes
are included. Original private history, main and workspace are preserved. The public history starts
at the independent allow-listed root `009f7e3632aca8454340438989d97a50df838e4d`.

## Scope and final evidence

Only committed source, documentation and unchanged project assets are archived. Runtime databases,
clipboard contents, logs, real/private configuration, installed dependencies, caches and build
products are excluded. Every Release lists its exact source commit and successful CI URL. Its
`SOURCE_RECORD.json`, `SHA256SUMS` and portable CI evidence bind the archive to that commit.
The post-publication Release verification run downloads the archive, checks its checksum and every
file byte/executable mode against that commit, then replaces the checkout with downloaded source
before fresh dependency installation and all source tests. No app bundle is published or installed.

The CI contract includes frozen pnpm install; lint, TypeScript, all frontend tests and frontend
build; locked Cargo metadata for the macOS target and all-platform graph; Node and Rust license
review; production Node advisory scan; RustSec audit; fmt; locked all-target/all-feature Clippy
with warnings denied; Rust tests; and explicit release-mode execution of the otherwise ignored
10,000-item search benchmark with its existing 100 ms assertion. Default frontend worker settings
are retained. A clean tracked/untracked source gate follows the checks.

## Security advisory decisions

Tool: upstream `cargo-audit` 0.22.2, downloaded from RustSec releases with pinned SHA-256;
development tooling only, MIT OR Apache-2.0. Scan the entire Rust lockfile without ignored
advisories or OS filters. CI records the fetched RustSec database commit and fails vulnerability
entries, unknown informational warnings, target-reachable unsoundness and registry-query errors.

Preflight database `b8a1a33e246a0a9a3b5f377248c41a503defec74` reports zero vulnerability
entries, six unmaintained warnings and one unsound warning. Local yanked-registry queries timed
out, so the local scan is incomplete for that part; final runner evidence is required before release.

| Warning | Target assessment and decision |
| --- | --- |
| RUSTSEC-2024-0370, proc-macro-error 1.0.4 | Absent from the macOS target graph; retained in unsupported GTK tooling graph. No patched version in advisory. |
| RUSTSEC-2025-0081 / 0075 / 0080 / 0100 / 0098, five unic 0.9.0 crates | Present transitively through Tauri's HTML tooling on macOS. Maintenance risk remains; no patched versions in advisories. Track upstream replacement rather than changing framework behavior during this frozen preview. |
| RUSTSEC-2024-0429, glib 0.18.5 | Actual upstream unsoundness, patched in >=0.20.0. Absent from the macOS target graph; Linux/GTK is unsupported. CI asserts absence. Linux use requires remediation and fresh validation. |

These warnings remain visible; a zero vulnerability count is not a claim that the dependency
graph is warning-free or secure against unknown issues. See [RustSec](https://rustsec.org/),
[glib advisory](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) and
[proc-macro-error advisory](https://rustsec.org/advisories/RUSTSEC-2024-0370.html).
Known synthetic Gitleaks matches are reviewed rather than suppressed; scanners do not establish
real-device privacy or transport safety.

## License, assets and privacy

Both package manifests declare `Apache-2.0`; LICENSE and NOTICE remain in the archive. This adds
metadata only: direct dependencies and lockfiles are unchanged. Fresh Node metadata and Rust
target/all-platform license reviews run because manifests changed. Installed Node metadata covers
the runner platform, not every optional platform package. License declarations and notice review
are not an artifact-specific binary redistribution audit. THIRD_PARTY_NOTICES and TRADEMARK retain
existing asset provenance/brand constraints; all 111 project asset bytes remain unchanged.

Core and Labs remain offline; no account, telemetry, cloud executor or remote model is added.
Local Link stays separate and default off, starting Bonjour/TCP only after explicit enable.
PRIVACY, data-flow and module-boundary documents still describe the unchanged runtime boundary.
GitHub CI downloads development dependencies/advisory data; it handles synthetic tests and source,
not user clipboard history. Security reporting belongs to the new repository's private advisory
channel or the maintainer contact in SECURITY.md.

## Reproduce

Download `ClipRiva-source.tar.gz`, `SOURCE_RECORD.json` and `SHA256SUMS` from the same Release.
Run `shasum -a 256 -c SHA256SUMS` after downloading all listed attachments, then extract source.
On a clean macOS host with the pinned Node/pnpm/Rust toolchains and Xcode command-line tools:

```sh
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --all-features --format-version 1
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets --all-features -- -D warnings
pnpm test:rust
cargo test --manifest-path src-tauri/Cargo.toml --locked --release -- --ignored --nocapture
```

For advisory/license evidence, reproduce the steps in `.github/workflows/ci.yml`; raw Cargo
metadata contains host paths and stays outside published evidence. `pnpm release:verify` and
native bundling/installation are binary gates and are not run for this source-only release.

## Deferred gates

Real native installation/UI, Intel and other macOS versions, two physical Macs, Local Link
50-pairing/200-transfer and packet/privacy matrix, signing, notarization, Gatekeeper, real-user
acceptance and artifact-specific bundled notices remain unverified. No installer, stable app or
Local Link Beta is released. This source preview does not satisfy those binary gates.
