# Private runner source verification — 2026-10-08

> Historical evidence. Current source-publication status and scope are in
> [the public preview record](source-public-preview-2026-10-08.md).


The frozen `2.0.0-alpha.1` source candidate is verified on the private repository's
`codex/source-candidate-ci-2026-10-08` branch through manual dispatch of the existing
[CI workflow](../../.github/workflows/ci.yml). This authorization covers the verification
branch and source checks only. The original workspace, private history, `main`, repository
visibility and previous local source delivery are preserved. No installer, tag, release,
signing/notarization, two-Mac or human acceptance work is authorized here.

## Scope and exact-commit evidence

The independent history starts at `009f7e3632aca8454340438989d97a50df838e4d`; subsequent
commits contain CI evidence tooling and audit documentation only. The private repository's
old history is not an ancestor of this branch or the delivered Git bundle. Application
implementation, assertions, manifests, lockfiles and all 111 existing asset files remain
byte-identical to that independent root.

The source handoff supplies `candidate-record.json`, `ci-summary.json`, portable dependency
metadata, logs, `SHA256SUMS` and reproduction instructions. Their final commit, tree,
workflow run, artifact identity and lockfile hashes must match. A preceding run's success
cannot substitute for the final source commit's result. Run `scripts/verify-git-sync.sh`
after pushing the frozen commit; a clean source tree and exact remote branch match are
required. No parity with `main` is claimed.

## Observed results and actual correction

The original, unmodified workflow passed on the independent root in
[run 37721163688](https://github.com/Sealdot/clipriva/actions/runs/37721163688). The runner
completed locked pnpm installation, frontend lint/types/default-concurrency tests/build,
Rust formatting, Clippy and default Rust tests. The default Rust suite reported 241 passed
and one explicitly ignored release-mode benchmark. This is source compilation/test evidence,
not a clean app installation or native UI acceptance result.

The first added metadata step in
[run 37721736310](https://github.com/Sealdot/clipriva/actions/runs/37721736310) failed in
the new audit parser: 23 Rust packages declare legacy slash-separated license alternatives.
All original declarations are retained. Pure legacy identifier lists such as
`MIT/Apache-2.0` are normalized to `MIT OR Apache-2.0` for evaluation, following the
[Cargo manifest documentation](https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields).
Unknown syntax, missing licenses and a restricted declaration without an unrestricted
alternative still fail the step. Restrictive AND obligations are not discarded. No dependency
or application code was changed to make the review pass.

The target metadata covers 127 installed Node package/version entries and 317 reachable
registry packages for `aarch64-apple-darwin`, with all root features selected for Cargo.
The corrected review has zero missing declarations, restricted declarations without an
unrestricted alternative, or unsupported expressions in that scope. The 23 legacy
normalizations are explicitly recorded; original and normalized expressions remain inspectable.
The exact final runner evidence must confirm these counts before handoff.

## Check contract

- Install with pinned pnpm 11.9.0 and `--frozen-lockfile`; do not copy a local dependency
  installation or private registry configuration into the runner.
- Run frontend lint, TypeScript, the full default-concurrency test suite and production build.
  The previous local timeout was not reproduced by the baseline runner. No two-worker
  setting, timeout extension, retry-on-failure or assertion deletion is introduced.
- Run `pnpm licenses list --json`; collect Cargo metadata with `--locked --all-features`
  and the runner's actual host target. Traverse only the reachable target graph for license
  counts. Raw metadata containing paths stays in the runner temporary directory.
- Review portable name/version/license records and retain both lockfile SHA-256 values.
  Run a production-only Node advisory audit and record tool/OS/Xcode/target versions.
- Run Rust format, locked Clippy on all targets/features with warnings denied, and locked
  default Rust tests.
- Separately run `cargo test --locked --release -- --ignored --nocapture` so the existing
  10,000-item Quick Paste benchmark executes its unchanged assertions. Its one-run result
  is not a p50/p95 performance or real-device acceptance study.
- Verify the tracked/untracked source tree is clean. Upload only portable audit evidence,
  never raw Cargo metadata, source databases, clipboard content, or an application bundle.

The artifact upload runs even after a failure to retain available evidence; it does not turn
a failed, cancelled or missing check into success. The final run must complete all required
steps successfully. Development CI uses GitHub and public package/crate registries under the
maintainer's authorization; no product network or privacy boundary changes.

## Remaining gaps and impact

This target review is not a fresh all-platform review of all 227 Node lock entries or all
518 Rust registry lock entries. Binary-specific dependency NOTICE obligations, MPL/Unicode
and other license terms still require review for the exact future distributed artifact.
The root LICENSE remains Apache-2.0; project manifests still lack machine-readable license
fields. The review is metadata evidence, not a legal distribution opinion.

Rust advisory scanning, universal/Intel and other macOS build matrices, a clean isolated-user
application launch/install/upgrade, GUI/permission/network observation, Local Link two-Mac
and Keychain/lifecycle/packet evidence, signing/notarization/Gatekeeper and real-device
distribution remain open. A Rust release-mode test executable is not an installer or a
distributable application; no Tauri bundle is built or uploaded in this workflow.

Privacy impact: no app data, permission, account, telemetry or network change; CI output
contains dependency metadata and tool versions only. Security impact: old private history
stays isolated from the source candidate; final source/history scans and branch sync remain
required, and untested runtime gates stay explicit. License impact: no runtime dependency,
asset or project license change; the additional CI upload action is development tooling
and does not ship in ClipRiva. Feature work remains frozen.
