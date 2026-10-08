# M0 baseline evidence for 91afcd8

**Date:** 2026-09-26. **Scope:** source identity, executable checks on one Mac, native first-run capture, synthetic browser UI captures, and alpha.2 blockers. This is not a signed build or release manifest. [Execution ledger](../../../planning/clipriva-2.0-execution.md) and [test protocol](../../../testing/clipriva-2.0-task-protocol.md) define subsequent work.

## Exact source and environment

- Start `HEAD`: `91afcd884e18eda20d73178a8a1e8bc0cd917758`; requested baseline commit is the same. `git diff --name-status 91afcd8 HEAD` returned no entries; `git status --short` was empty before M0 files were created.
- Version at start: `2.0.0-alpha.1` in `package.json`, `src-tauri/Cargo.toml`, the Cargo lock package entry and `src-tauri/tauri.conf.json`. No version bump, migration, dependency change, permission or network change was made in M0.
- Test machine: macOS 14.2.1 (23C71), Apple M1 `arm64`, Node 24.19.0, Rust 1.97.1. Active developer directory was Command Line Tools; `xcodebuild -version` failed because full Xcode is absent. Approximately 7.4 GiB was free before native compilation and 3.5 GiB after, so a second target directory/clean release bundle is blocked by local capacity unless space is safely supplied. No user files were deleted to make space.
- Native dev launch used a temporary Tauri configuration overriding only the bundle identifier to an isolated M0 review identity and disabling duplicate dev-server startup. It is not the production identifier or a frozen package. The disposable review profile and uncropped screenshot were moved to local Trash after the capture because native startup observed the existing system pasteboard; neither is part of this repository. No user clipboard body is present in committed evidence.

## Executed checks

`$BUNDLED_NODE` below denotes the workspace-provided Node 24.19.0. These are the actual script arguments used after a frozen-lockfile `pnpm install` (exit 0). `pnpm check` was also attempted but failed in the package-manager preflight while retrying registry metadata; its scripts did not run through that invocation. The same underlying lint/typecheck/test/build commands were executed directly and are reported separately, without converting the failed aggregate command into a pass.

| Command | Exit / result | Evidence limit |
| --- | --- | --- |
| `pnpm install --frozen-lockfile --registry=https://registry.npmjs.org` | 0; lockfile policy check 193 entries | Dependency installation only. Initial configured private-mirror attempt was interrupted after request errors; public-source retry completed. |
| `$BUNDLED_NODE node_modules/@biomejs/biome/bin/biome check .` | 0; 92 files checked, 4 broken-symlink warnings under pre-existing `logs/soak` | Static frontend checks; warnings need cleanup/ignore review. |
| `$BUNDLED_NODE node_modules/typescript/bin/tsc -b --pretty false` | 0 | TypeScript only. |
| `$BUNDLED_NODE node_modules/vitest/vitest.mjs run` | 0; 26 files, 222 tests passed | Browser/component and browser-repository fixture, not native clipboard behavior. |
| `$BUNDLED_NODE node_modules/vite/bin/vite.js build` | 0; 1,886 modules; JS chunk 517.31 kB with a >500 kB warning | Frontend bundle, not Tauri installer. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | 0 | Formatting. |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | 0 | Rust static checks. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked` | 0; 234 passed, 0 failed, 1 ignored release benchmark | Native library/repository tests; no physical target-app confirmation. |
| Tauri dev launch with isolated identifier | 0 build and launched; native first-run dialog captured | Development build on this Mac only; not installer, signed or notarized. |
| `pnpm check` through bundled pnpm entry | 1; registry retry raised `TypeError: Cannot set property message ... which has only a getter` before package scripts | Package-manager preflight/environment issue remains open, although equivalent commands above passed. |

The ignored Rust case is the repository's explicit release-mode 10k search benchmark; no M0 P95 performance result is claimed. `pnpm release:verify`, unsigned release bundle, clean-machine install, migration from a real older profile, packet scan, idle-resource trace, VoiceOver, target-app Direct Paste and dual-Mac Local Link were **not_run**.

## Captured visuals and provenance

All screenshots in `screenshots/` were created in this M0 task from the current source or the original CSS/HTML prototype. No third-party icon, font, stock image or copied artwork was added. The standalone prototype uses only CSS and synthetic text; the browser adapter uses repository-owned synthetic fixtures. This paragraph records the provenance required for screenshots/other visual material under `AGENTS.md`.

| File | Evidence layer | What it shows / limit |
| --- | --- | --- |
| `native-first-run-dialog.png` | `native-dev` | Actual Tauri first-run dialog on this Mac, cropped before commit to exclude unrelated clipboard content. Does not show full native workspace or Quick Paste. |
| `browser-mock-main-workspace.png` | `browser-mock` | Current three-column shell, selected list row and first-value guide using synthetic in-memory Items. |
| `browser-mock-quick-paste.png` | `browser-mock` | Current 680×440 Quick Paste visual with eight synthetic entries. |
| `browser-mock-quick-empty.png` | `browser-mock` | Current no-result state for a synthetic unmatched query. |
| `browser-mock-settings.png` | `browser-mock` | Settings shell while the first-value guide is still active; inner area appears dim. It does not validate native permission state. |
| `prototype-quick-light.png`, `prototype-quick-dark.png`, `prototype-quick-degraded.png` | `design-prototype` | Proposed 600×454 information hierarchy and fallback copy. They are neither current implementation nor native evidence. |

Native Quick Paste, Inspector, permission revoked, capture paused/excluded and Local Link state screenshots are **not_run**: the computer-use control service failed to start, and further native interaction would risk recording unrelated system pasteboard content. Browser fixtures and the prototype are retained as UI references only. A future disposable-profile native run must seed synthetic content without altering user data and capture those states.

## Findings and blockers

1. **CR-01/02:** `use_clipboard_item` already has typed `clipboardRestored/pasteSent/pasteNotSent` and native fallback, but write failure/invalid Item remain command errors rather than the proposed complete UI contract. Quick Paste has no explicit in-flight activation guard in the inspected component. `pasteSent` correctly names dispatch, not receipt.
2. **CR-02/04:** Native Quick Paste is fixed 680×440 with a forced dark theme; current React row click activates Copy, hover changes selection, Space opens Preview on empty search, and a global `⌘C` handler intercepts a selected text field. The prototype proposes a visible primary Copy action and 600×454. These are observed code differences, not claims of a reproduced native defect.
3. **CR-03:** Quick Paste checks only the newest 10,000 candidate Items before matching; a 50,000-item later match can be absent. Its own case-folded substring matcher can support short Chinese strings within that window, while main FTS uses `unicode61`. Recall, punctuation safety and P95 need the specified synthetic matrix before an indexing decision.
4. **CR-05/07:** Current main workspace has a permanent inspector at 860×560. First-run and Privacy Cover already exist, so work should converge wording and state rather than replace them. Native first-run on this Mac observed existing clipboard content in the isolated profile; a future first-run/privacy review must explicitly test startup capture timing and consent semantics with synthetic data.
5. **CR-10/11:** Full Xcode, Developer ID/notarization owner, additional macOS/Intel devices, a second real Mac, and human task testers are unavailable in this M0 run. The exact frozen signed package, 50 pairings, 200 transfers, and complete safety/lifecycle matrix remain **blocked/not_run**. Local Link remains Preview/No-Go. A Core stable package needs demonstrated native isolation if that gate stays open; a hidden UI toggle alone is insufficient.
6. **Resource / tooling:** Low free disk space and the configured private npm mirror prevent a clean second-target release run here. No `pnpm check` success is claimed. The source-level equivalents and Rust checks have current passing results.

## Support matrix and next alpha.2 list

The repository claims macOS 13+ and its existing beta matrix lists Ventura 13 through Tahoe 26 on Apple Silicon/Intel combinations. M0 directly exercised only macOS 14.2.1 on Apple M1 in dev mode. Every other OS/architecture row, full-screen/multi-display, target-app format matrix, signed install and upgrade is **not_run**; exact feasible Intel/high-OS combinations and device owners must be resolved before a release support claim. Rosetta or one-Mac source build will not fill a native Intel row.

The implementation order is [CR-01/04 → CR-02/03/07 → CR-05/06](../../../planning/clipriva-2.0-execution.md), with CR-08 migration protection and CR-09/10 decisions prepared separately. The immediate gate is review of the Quick Paste light/dark/degraded prototype. No alpha.2 feature code or version bump was made in M0.

**Privacy impact:** no product data flow changed; the isolated test profile was moved to Trash after it observed existing pasteboard content. **Security impact:** no permission/network/cryptography change; the at-rest decision remains proposed. **License impact:** no dependencies or third-party assets were added; screenshots and prototype are task-created from existing source and synthetic fixture text.
