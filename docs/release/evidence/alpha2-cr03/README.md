# CR-03 Quick Paste search evidence

**Code commits:** `e112a07e7fa2726c800e954bf5d9483c9a038679` (search implementation), `227ace3` (late-response regression), local and unpushed. **Status:** search correctness implemented on synthetic data; CR-03 acceptance remains open for native and release-performance validation.

## Delivery and locations

- `src-tauri/src/db/mod.rs`: a nonempty Quick Paste query prefilters the full retained History using parameterized SQLite substring search, then applies the existing deterministic Rust rank. Zero-input suggestions still use a bounded 10,000-Item window sorted by recent activity. Manual OCR and Note matching provide provenance without serializing their text in search results. A scalar retention check avoids hydrating all unexpired Items on each query. No schema migration was added.
- `src/features/clipboard/browserRepository.ts`, `types.ts`, and `QuickPasteOverlay.tsx`: synthetic browser parity for continuous Chinese, literal punctuation and manual OCR, and a content-free Note/Image text match label. Tests are in `browserRepository.test.ts`, `App.test.tsx`, and the Rust database module.
- `src/features/clipboard/QuickPasteOverlay.test.tsx`: a controlled `tauri` request remains pending while a newer `ClipRiva` request completes. Releasing the older result does not replace the newer rows or change the Item activated by Enter. The existing query-key isolation was sufficient; no runtime search or ranking code changed.
- [Search decision](../../../adr/quick-paste-search.md): options, index tradeoffs and the remaining performance risk.
- `PRIVACY.md`, `docs/privacy/data-flow.md`, and `docs/open-source/module-boundary.md`: clarify that a Quick Paste match on manually extracted image text exposes only provenance; the text body remains in its existing local boundary.

## Executed checks

`$NODE` is the workspace-bundled Node.js executable returned by `load_workspace_dependencies`. These checks ran after code and the first evidence commit (`e1d78d9fd515f49c700d3909ca94dce5af26a464`) with no source changes.

| Command | Result |
| --- | --- |
| `$NODE node_modules/@biomejs/biome/bin/biome check .` | **passed**, exit 0; 94 files and four pre-existing broken `logs/soak` symlink warnings. |
| `$NODE node_modules/typescript/bin/tsc -b --pretty false` | **passed**, exit 0. |
| `$NODE node_modules/vitest/vitest.mjs run` | **passed**, exit 0; 26 files / 227 tests. |
| `$NODE node_modules/vite/bin/vite.js build` | **passed**, exit 0; 519.37 kB JS chunk warning. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | **passed**, exit 0. |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | **passed**, exit 0. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked -q` | **passed**, exit 0; 239 passed / 1 pre-existing ignored benchmark. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked quick_paste_reaches_an_older_match_beyond_50k_newer_items -- --nocapture` | **passed**, exit 0; one rare query 61.573 ms and broad query 585.252 ms in a debug build, not P95. |

Later regression verification on source commit `227ace3`:

| Command | Result |
| --- | --- |
| `pnpm exec vitest run src/features/clipboard/QuickPasteOverlay.test.tsx -t 'does not show or activate a late result'` | **passed**, exit 0; one deterministic delayed-response case. |
| `pnpm lint` | **passed**, exit 0; 96 files and four pre-existing broken `logs/soak` symlink warnings. |
| `pnpm typecheck` | **passed**, exit 0. |
| `pnpm test` | **passed**, exit 0; 26 files / 236 tests after isolating the activation spy from the fixture's mutable ranking. An earlier full run failed in a neighboring rank test because the new test had copied a fixture Item; that test isolation issue was corrected before commit. |
| `pnpm build` | **passed**, exit 0; 528.17 kB JS chunk warning. |

## Screenshots and validation limits

- [Continuous Chinese query](screenshots/chinese-browser-mock.png) and [literal symbol query](screenshots/symbol-browser-mock.png) show the 600×454 React Quick Paste interface with synthetic in-repository fixture data. Captured at 1× from local Vite in headless Chrome. They are **browser-mock** evidence, not native product screenshots. They contain no real clipboard content or external asset; the UI uses repository code and dependency icons.
- The 50,001-Item Rust fixture verifies older retained-item recall; the Note/OCR tests verify matching, deletion lifecycle and text-body omission. The delayed-response source test verifies React query-key isolation and Enter selection, but it is not native IPC timing evidence. Native IME composition, stale-response race, 1×/2× screen behavior, full History parity and target-app receipt are **blocked/not_run** while safe native desktop control is unavailable.
- Release-build search P95 over at least 100 hot samples, raw latency list, index size and write-cost comparison are **not_run**. The post-commit single debug sample of the broad 50k query took 585.252 ms, above the planned ≤350 ms P95 threshold. It cannot be used as release P95. CR-03 remains open.

## Impact and rollback

No permission, network path, dependency, new stored data type, migration, encryption or Local Link behavior changed. Privacy documentation now describes the already local manual-OCR match provenance crossing IPC. Security: search terms are bound parameters; the body of Note/OCR text stays out of Quick Paste result serialization. License: no new assets or dependencies; the screenshots' provenance is recorded above. The substring prefilter scans retained rows and has incomplete Unicode case folding through SQLite `lower()`, so broad-query latency and some cased non-ASCII scripts remain risks. Rollback: `git revert 227ace3` for the regression test or `git revert e112a07e7fa2726c800e954bf5d9483c9a038679` for search implementation, after checking later CR dependencies.
