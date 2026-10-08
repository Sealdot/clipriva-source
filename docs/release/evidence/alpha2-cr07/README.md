# CR-07 first use and privacy controls evidence

**Code commits:** `1ed6882dd91e0f11ceae57e720a51e0f6b010b09` (first use/privacy wording), `5fd53ac` (workspace copy failure recovery), `a22dc33` (first-value/Cover theme), local and unpushed. **Status:** wording, theme and synthetic DOM checks passed; CR-07 native acceptance remains open.

## Delivery and locations

- `src/app/App.tsx`: first-use guide now describes selecting the earlier Item and choosing Copy, then pasting with ⌘V. Onboarding uses non-sensitive examples and says Copy needs no Accessibility permission. Privacy Cover explicitly states that capture continues unless paused, History remains stored, and Cover is not disk encryption or a system-wide recording block.
- `src/features/clipboard/CaptureSettingsDialog.tsx`: the first-use instructions match Quick Paste's Copy behavior. The excluded-app field states that adding an exclusion does not remove existing History.
- `src/app/App.test.tsx` and `src/features/clipboard/CaptureSettingsDialog.test.tsx`: synthetic checks cover the visible explanations and omission of known Item/source strings from the covered DOM.
- `src/app/App.tsx` and `src/styles/global.css`: a failed workspace Copy keeps the Item visible and shows a content-free alert with Retry Copy and Dismiss. The success notice appears only after a successful write. Stack continues to hold its position on failure instead of consuming the Item. `src/app/App.test.tsx` injects one clipboard write failure, verifies no false success or second write, then verifies explicit retry succeeds.
- `src/styles/global.css`: the first-value guide and Privacy Cover now use the system theme's surface, text, border and accent tokens. Cover still replaces the app tree while active; this CSS change adds no content rendering, capture rule or permission.

## Executed checks

`$NODE` is the workspace-bundled Node.js executable returned by `load_workspace_dependencies`. These checks ran after code and the first evidence commit (`a26d4233fddaaded75782cd6b2ead66a665876c2`) with no source changes.

| Command | Result |
| --- | --- |
| `$NODE node_modules/@biomejs/biome/bin/biome check .` | **passed**, exit 0; 94 files and four pre-existing broken `logs/soak` symlink warnings. |
| `$NODE node_modules/typescript/bin/tsc -b --pretty false` | **passed**, exit 0. |
| `$NODE node_modules/vitest/vitest.mjs run` | **passed**, exit 0; 26 files / 227 tests. |
| `$NODE node_modules/vite/bin/vite.js build` | **passed**, exit 0; 519.87 kB JS chunk warning. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | **passed**, exit 0. |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | **passed**, exit 0. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked -q` | **passed**, exit 0; 239 passed / 1 pre-existing ignored benchmark. |

For the later copy failure change, the following checks ran on source commit `5fd53ac` with the screenshot and this evidence document still untracked/uncommitted. The clean-tree replay after the evidence commit is reported in the task handoff.

| Command | Result |
| --- | --- |
| `pnpm lint` | **passed**, exit 0; 96 files and four pre-existing broken `logs/soak` symlink warnings. |
| `pnpm typecheck` | **passed**, exit 0. |
| `pnpm test` | **passed**, exit 0; 26 files / 235 tests. |
| `pnpm build` | **passed**, exit 0; 528.17 kB JS chunk warning. |

For theme code commit `a22dc33`, `pnpm lint` **passed**, exit 0 with the four existing broken `logs/soak` symlink warnings; `pnpm typecheck` **passed**; `pnpm test` **passed**, 26 files / 237 tests; `pnpm build` **passed** with a 528.16 kB JS chunk warning; and `git diff --check` **passed**. The existing covered-DOM test remains in the full run.

## Screenshots and limitations

- [First-use screen](screenshots/first-use-browser-mock.png) and [Privacy Cover](screenshots/privacy-cover-browser-mock.png) are 920×620 synthetic React captures at 1× from local Vite in headless Chrome. The first screenshot shows the changed recovery explanation; the second shows the cover without fixture Item content. These are **browser-mock** evidence, not native acceptance. They use this repository's browser adapter and existing dependency icons, with no external asset or live clipboard data.
- [Workspace Copy failed](screenshots/copy-failed-browser-mock.png) is a 920×620 light-theme synthetic React capture at 1× from local Vite in headless Chrome. The fixture Code Item remains in details and `navigator.clipboard.writeText` was replaced with a rejection to show the Retry Copy alert. This is **browser-mock** evidence, not a native clipboard permission or paste test. It uses this repository's browser adapter and existing dependency icons; no external asset or live clipboard data was added.
- [First-value guide light](screenshots/first-value-guide-light-browser-mock.png), [first-value guide dark](screenshots/first-value-guide-dark-browser-mock.png), [Privacy Cover light](screenshots/privacy-cover-light-browser-mock.png) and [Privacy Cover dark](screenshots/privacy-cover-dark-browser-mock.png) are 920×620, 1× local Vite/headless Chrome captures with the in-memory synthetic repository. The guide computed white/dark surfaces `rgb(255, 255, 255)` / `rgb(29, 35, 41)`; Cover computed light/dark backgrounds `rgb(245, 246, 247)` / `rgb(21, 25, 29)` and matching text. In the browser covered state, a known synthetic Code Item string was absent from `document.body.innerText`. These are **browser-mock** visual/DOM observations, not proof of native AX or screenshot protection. No real clipboard content or third-party asset was added.
- Native first-use return/focus, Accessibility denial/revocation, actual content protection, VoiceOver/AX omission, screenshot behavior and content-free diagnostic marker scan are **blocked/not_run**. The DOM test cannot prove a native AX or screen-capture property. Native copy denial/recovery and the rest of the CR-07 error-state matrix remain open.

## Impact and rollback

No new permission, network, stored data, dependency, encryption, migration, or release choice. Privacy and security boundaries are unchanged; the visible wording now states its actual scope, and a clipboard write error cannot be mistaken for a successful copy. No third-party material was added; screenshot provenance is above. Rollback: `git revert a22dc33` for theme, `git revert 5fd53ac` for copy recovery or `git revert 1ed6882dd91e0f11ceae57e720a51e0f6b010b09` for first-use wording, after reviewing later CR dependencies.
