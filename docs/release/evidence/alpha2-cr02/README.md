# CR-02 Quick Paste interface evidence

**Code commit:** `59eef5445faf3c1e32c1545ba201a3ea170b3679` (local, unpushed). **Status:** key interaction and 600×454 layout implemented; native acceptance remains blocked.

## Delivery and locations

- `src-tauri/tauri.conf.json`: Quick Paste defaults to 600×454 logical points with a 520×380 minimum; the old 680×440 fixed maximum is removed.
- `src-tauri/src/lib.rs`: each opening clamps the size and centers the window in the current monitor's reported work area, with primary monitor fallback. Pure tests cover 1×, 2×, undersized and unavailable areas.
- `src/features/clipboard/QuickPasteOverlay.tsx`: a row click selects without writing; hover leaves selection unchanged. One visible footer button copies to the clipboard. `⌘I` toggles temporary Preview, Space stays with the input, and search/Preview text selection keeps native `⌘C`. `Escape` closes Preview before the window. The More menu remains a secondary action entry at the footer.
- `src/features/clipboard/ClipboardActionPanel.tsx`: the Quick Paste menu names Direct Paste explicitly and does not advertise Enter as a Direct Paste shortcut while Enter defaults to Copy.
- `src/styles/global.css` and component tests: 58–60 px row rhythm, visible action footer, keyboard and selection coverage.

## Executed checks

`$NODE` is the workspace-bundled Node.js executable returned by `load_workspace_dependencies`.

| Command | Result |
| --- | --- |
| `$NODE node_modules/@biomejs/biome/bin/biome check .` | **passed**, exit 0; 94 files with four pre-existing broken `logs/soak` symlink warnings. |
| `$NODE node_modules/typescript/bin/tsc -b --pretty false` | **passed**, exit 0. |
| `$NODE node_modules/vitest/vitest.mjs run` | **passed**, exit 0; 26 files / 225 tests. Focused App/Quick Paste run: 42 passed. |
| `$NODE node_modules/vite/bin/vite.js build` | **passed**, exit 0; 519.00 kB JS chunk warning. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | **passed**, exit 0. |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | **passed**, exit 0. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked -q` | **passed**, exit 0; 237 passed / 1 pre-existing ignored benchmark. |

## Screenshots and unresolved validation

- [Light 600×454](screenshots/quick-light-browser-mock.png) and [dark 600×454](screenshots/quick-dark-browser-mock.png) show the current React interface with in-memory synthetic data and the footer action. Captured from a local Vite preview in headless Chrome at 1×. These screenshots are **browser-mock** evidence only. They were generated from this repository's UI and existing dependency icons; no external asset was added.
- Real Tauri light/dark at 1× and 2×, actual usable-screen positioning, Accessibility denial, VoiceOver, focus return, target-app receipt and the full shortcut conflict matrix are **blocked/not_run** because native desktop control remains unavailable. The WorkArea tests are source behavior, not a physical multi-display observation.

## Impact and risk

No permission, network, stored-data type, dependency, migration or Local Link gate changed. Privacy/security/license boundaries are unchanged. On a multi-display Mac the hidden window's current monitor may lag the previous app's monitor; native observation is required before claiming correct placement. Existing English copy is retained pending the CR-07 wording pass. The interface remains on `2.0.0-alpha.1` until the complete alpha.2 gate is ready. Rollback: `git revert 59eef5445faf3c1e32c1545ba201a3ea170b3679` after checking later CR dependencies.
