# CR-01 recovery contract evidence

**Code commit:** `4939498e049c97c1e8217dbb950cc1862c1b0fe7` (local, unpushed). **Machine:** macOS 14.2.1, Apple M1. **Evidence type:** source tests and browser fixture only. This record does not establish native target-app delivery.

## Changed contract and locations

- `src-tauri/src/commands.rs`: `use_clipboard_item` returns bounded `invalidItem`, `writeFailed`, `historyRecordFailed`, or `busy` results before any paste dispatch. It retains `clipboardRestored`, `pasteSent`, and `pasteNotSent` for successful write phases. A failed Direct Paste reasserts the selected Item in the same format, including plain text mode, without a second history increment. Database infrastructure errors remain IPC errors.
- `src-tauri/src/quick_paste.rs`: one activation guard prevents overlapping `use_clipboard_item` calls and releases on every return path. The prior app, Accessibility, focus, and self-write checks remain in their existing native path.
- `src/features/clipboard/QuickPasteOverlay.tsx` and `types.ts`: one in-flight UI action, truthful stopped-state messages, no Stack advance on a failed write or missing Item, and content-free failure diagnostics.
- `src/features/clipboard/browserRepository.ts` and focused tests: synthetic browser results reflect invalid Item and clipboard-write failure. They are fixture tests, not OS evidence.

## Executed checks

`$NODE` below is the workspace-bundled Node.js executable returned by `load_workspace_dependencies`. Commands ran in the repository with no package or lockfile change.

| Command | Result | Meaning |
| --- | --- | --- |
| `$NODE node_modules/@biomejs/biome/bin/biome check .` | **passed**, exit 0, 93 files; four pre-existing broken symlink warnings in `logs/soak` | Source style only. |
| `$NODE node_modules/typescript/bin/tsc -b --pretty false` | **passed**, exit 0 | Type contract. |
| `$NODE node_modules/vitest/vitest.mjs run` | **passed**, exit 0, 26 files / 224 tests | Includes repeated Enter and synthetic invalid/write-failure cases. |
| `$NODE node_modules/vite/bin/vite.js build` | **passed**, exit 0 | Frontend bundle; 518.51 kB chunk warning. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | **passed**, exit 0 | Rust formatting. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked` | **passed**, exit 0, 236 passed / 1 ignored | Guard and bounded serialization tests; ignored case is the existing release benchmark. |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | First run **failed** on a collapsible-if lint; fixed and rerun **passed**, exit 0 | Final code passes. |
| `git diff --check` | **passed**, exit 0 | Whitespace. |

## Screens and unverified layers

The approved [light](../m0-91afcd8/screenshots/prototype-quick-light.png), [dark](../m0-91afcd8/screenshots/prototype-quick-dark.png), and [degraded](../m0-91afcd8/screenshots/prototype-quick-degraded.png) captures show the intended UI state with synthetic content. They predate this code and are **browser-mock design evidence** only. A same-scenario CR-01 native screenshot or recording is **blocked**: desktop control timed out again on 2026-09-26, and the native dev profile exposed live clipboard content, so a safe raw screen was not retained. Native permission allow/deny/revoke, target focus change, actual Clipboard write failure, and target receipt are **not_run**. Frozen signed package and dual-Mac checks are **not_run**.

## Impact and remaining risk

No new permission, network request, stored data type, dependency, asset, or migration. Privacy, security, and license boundary is unchanged; returned failure categories contain no Item content. `pasteSent` proves only that ClipRiva dispatched Command-V to a rechecked target, not that the target accepted it. The older tray `copy_clipboard_item` route has no shared activation guard; the new guard covers `use_clipboard_item`. A write can fail after the OS partially changes the clipboard; `writeFailed` deliberately does not claim the old clipboard remains intact. Rollback is `git revert 4939498e049c97c1e8217dbb950cc1862c1b0fe7` after reviewing subsequent CR dependencies.
