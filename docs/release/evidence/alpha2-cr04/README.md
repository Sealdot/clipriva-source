# CR-04 system-theme foundation evidence

**Code commits:** `d4867cfea81b1cc1a528d56fbcc55f583e4e7e38` (foundation), `99408b8` (recovery dialogs), `fa94261` (shared action menu), `af4dac4` (Capture Settings), `968045f` (Filter Composer), `4a7a635` (Stack panel), `a22dc33` (first-value/Cover), local and unpushed. **Status:** foundation and six shared-surface passes implemented; CR-04 remains in progress pending remaining component coverage and native accessibility checks.

## Delivery

- `src/styles/theme.css`: light/dark semantic tokens for backgrounds, surfaces, text, controls, focus and feedback, plus a reduced-motion override for the main animated controls. The light palette activates with `prefers-color-scheme: light`; dark is the default.
- `src/main.tsx`: loads the token layer after the existing stylesheet, so existing React components can be updated incrementally without replacing the 3,951-line stylesheet at once.
- `src-tauri/tauri.conf.json`: removes Quick Paste's forced Dark window appearance so the webview can follow the system setting. No window size, permissions, network capability, or identifier changed.
- `src/features/clipboard/CleanupPreviewDialog.css`, `RecycleBinDialog.css`, and `src/styles/global.css`: cleanup preview, Recycle Bin, destructive confirmation and shared text-button hover/danger use the same semantic light/dark tokens. The Recycle Bin list now has a viewport-bound scroll area; the first synthetic capture exposed items escaping below the modal at 920×620, which the final captures verify was corrected. Cleanup and deletion logic did not change.
- `src/features/clipboard/ClipboardActionPanel.css`: the shared Quick Paste/workspace More menu, active/focus row, danger action and inline recycle confirmation now use the semantic palette in both system themes. Action behavior and permission boundary did not change.
- `src/features/clipboard/CaptureSettingsDialog.css` and `src/styles/theme.css`: Capture Settings tabs, status cards, privacy controls, disclosures, diagnostics, shortcut panels and input fields use semantic text, surface, border, focus, selected and warning tokens. The settings fields previously inherited a dark input background in light mode; the shared theme rule corrects it. Capture policy, permissions and stored settings did not change.
- `src/features/clipboard/FilterComposerDialog.css`: the Labs Filter Composer dialog, overlay, secondary text, form fields, Before/After panels and error border/text now use the same theme tokens. Its old variable names were undefined and fell back to fixed dark values. Filter rules and execution behavior did not change.
- `src/features/clipboard/ClipboardQueuePanel.css`: Stack panel, current Item, controls, reset and Quick Paste drawer use the shared theme tokens. Stack remains temporary process memory; order, copy, skip and reset behavior did not change.
- `src/styles/global.css`: the first-value guide and Privacy Cover use semantic light/dark colors in `a22dc33`; [CR-07 evidence](../alpha2-cr07/README.md) contains the paired captures and covered-DOM observation.
- Browser-fixture screenshots: [Quick Paste light](screenshots/quick-light-browser-mock.png), [Quick Paste dark](screenshots/quick-dark-browser-mock.png), [first use light](screenshots/first-use-light-browser-mock.png). Captured at 1× with local Vite and headless Chrome, using the in-memory synthetic repository. These are **browser-mock** evidence, not native acceptance captures. The screenshots are generated from this repository's UI and existing dependency icons; no external image/font asset was introduced.
- Later 920×620 1× local Vite/headless Chrome captures: [cleanup preview light](screenshots/cleanup-preview-light-browser-mock.png), [cleanup preview dark](screenshots/cleanup-preview-dark-browser-mock.png), [Recycle Bin light](screenshots/recycle-bin-light-browser-mock.png), [Recycle Bin dark](screenshots/recycle-bin-dark-browser-mock.png), [permanent confirmation light](screenshots/recycle-bin-confirm-light-browser-mock.png), [permanent confirmation dark](screenshots/recycle-bin-confirm-dark-browser-mock.png). A disposable browser fixture moved seven unpinned sample Items through the real UI to populate the Recycle Bin. No user clipboard or database was touched. These are **browser-mock**, not native acceptance. Computed final light dialog background was `rgb(255, 255, 255)`; the list client height was 333 px and scroll height 657 px, contained within the dialog. All visual assets are repository UI or existing dependency icons.
- Later 600×454 1× captures of [Quick Paste More light](screenshots/quick-actions-light-browser-mock.png), [Quick Paste More dark](screenshots/quick-actions-dark-browser-mock.png), [inline recycle confirmation light](screenshots/quick-actions-confirm-light-browser-mock.png) and [inline recycle confirmation dark](screenshots/quick-actions-confirm-dark-browser-mock.png) used local Vite/headless Chrome and the synthetic browser repository. The final light menu computed white `rgb(255, 255, 255)` background and dark `rgb(24, 34, 44)` foreground. These are **browser-mock** visual checks; no native window, external asset or real clipboard data was involved.
- Later 920×620 1× captures of Capture Settings [General light](screenshots/capture-settings-general-light-browser-mock.png), [General dark](screenshots/capture-settings-general-dark-browser-mock.png), [Privacy light](screenshots/capture-settings-privacy-light-browser-mock.png), [Privacy dark](screenshots/capture-settings-privacy-dark-browser-mock.png), [Privacy scrolled light](screenshots/capture-settings-privacy-bottom-light-browser-mock.png), [Privacy scrolled dark](screenshots/capture-settings-privacy-bottom-dark-browser-mock.png), [Shortcuts light](screenshots/capture-settings-shortcuts-light-browser-mock.png) and [Shortcuts dark](screenshots/capture-settings-shortcuts-dark-browser-mock.png) used local Vite/headless Chrome and an in-memory synthetic fixture. Visual inspection confirmed readable tabs, status blocks and numeric fields in both themes. These are **browser-mock** checks, not native acceptance. Screenshots contain only repository UI and existing dependency icons; no third-party asset, real clipboard data or settings were introduced.
- Later 1280×800 1× [Filter Composer light](screenshots/filter-composer-light-browser-mock.png) and [Filter Composer dark](screenshots/filter-composer-dark-browser-mock.png) captures used local Vite/headless Chrome, with Labs enabled only in the in-memory synthetic browser fixture and the built-in code Item selected. Computed light dialog/text/panel/input colors were `rgb(255, 255, 255)` / `rgb(24, 34, 44)` / `rgb(240, 242, 244)` / `rgb(240, 242, 244)`; dark values were `rgb(29, 35, 41)` / `rgb(242, 245, 246)` / `rgb(36, 44, 51)` / `rgb(36, 44, 51)`. These are **browser-mock**, not native acceptance. No real clipboard or stored setting was touched. The browser fixture logged a duplicate `demo-code` React key warning during interaction; it did not affect this visual check, but its fixture cause remains to be checked.
- Later 1280×800 1× [Stack panel light](screenshots/stack-panel-light-browser-mock.png) and [Stack panel dark](screenshots/stack-panel-dark-browser-mock.png) captures used local Vite/headless Chrome. The synthetic UI added its nine visible demo Items to the process-memory Stack. Computed panel/text/border colors were light `rgb(255, 255, 255)` / `rgb(24, 34, 44)` / `rgb(220, 225, 229)` and dark `rgb(29, 35, 41)` / `rgb(242, 245, 246)` / `rgb(58, 70, 80)`. The fixture's first-use reminder remains visible below the Stack. These are **browser-mock**, not native or persistent-data evidence; no real clipboard was touched.

## Executed checks

`$NODE` is the workspace-bundled Node.js executable returned by `load_workspace_dependencies`.

| Command | Result |
| --- | --- |
| `$NODE node_modules/@biomejs/biome/bin/biome check .` | **passed**, exit 0; 94 files, four pre-existing broken `logs/soak` symlink warnings. |
| `$NODE node_modules/typescript/bin/tsc -b --pretty false` | **passed**, exit 0. |
| `$NODE node_modules/vitest/vitest.mjs run` | **passed**, exit 0; 26 files / 224 tests. |
| `$NODE node_modules/vite/bin/vite.js build` | **passed**, exit 0; 518.51 kB JS chunk warning. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | **passed**, exit 0. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked -q` | **passed**, exit 0; 236 passed / 1 pre-existing ignored benchmark. |
| Local contrast calculation on token pairs | Light text 16.10:1, muted 5.85:1, accent 5.04:1 against white; dark text 14.47:1 against surface, muted 6.55:1 against panel, accent 9.07:1 against surface. This checks only token pairs, not every component state. |

Later checks for code commit `99408b8`:

| Command | Result |
| --- | --- |
| `pnpm lint` | **passed**, exit 0; 96 files and four pre-existing broken `logs/soak` symlink warnings. |
| `pnpm typecheck` | **passed**, exit 0. |
| `pnpm test` | **passed**, exit 0; 26 files / 236 tests. |
| `pnpm build` | **passed**, exit 0; 528.17 kB JS chunk warning. |
| `git diff --check` | **passed**, exit 0. |

Checks for shared action-menu code commit `fa94261`:

| Command | Result |
| --- | --- |
| `pnpm lint` | **passed**, exit 0; 96 files and four pre-existing broken `logs/soak` symlink warnings. |
| `pnpm typecheck` | **passed**, exit 0. |
| `pnpm test` | **passed**, exit 0; 26 files / 236 tests. |
| `pnpm build` | **passed**, exit 0; 528.17 kB JS chunk warning. |
| `git diff --check` | **passed**, exit 0. |

Checks for Capture Settings code commit `af4dac4`:

| Command | Result |
| --- | --- |
| `pnpm lint` | **passed**, exit 0; 96 files and four pre-existing broken `logs/soak` symlink warnings. |
| `pnpm typecheck` | **passed**, exit 0. |
| `pnpm test` | **passed**, exit 0; 26 files / 236 tests. |
| `pnpm build` | **passed**, exit 0; 528.17 kB JS chunk warning. |
| `git diff --check` | **passed**, exit 0. |

Checks for Filter Composer code commit `968045f`:

| Command | Result |
| --- | --- |
| `pnpm lint` | **passed**, exit 0; 96 files and four pre-existing broken `logs/soak` symlink warnings. |
| `pnpm typecheck` | **passed**, exit 0. |
| `pnpm test` | **passed**, exit 0; 26 files / 236 tests. |
| `pnpm build` | **passed**, exit 0; 528.17 kB JS chunk warning. |
| `git diff --check` | **passed**, exit 0. |

Checks for Stack panel code commit `4a7a635`:

| Command | Result |
| --- | --- |
| `pnpm lint` | **passed**, exit 0; 96 files and four pre-existing broken `logs/soak` symlink warnings. |
| `pnpm typecheck` | **passed**, exit 0. |
| `pnpm test` | **passed**, exit 0; 26 files / 236 tests. |
| `pnpm build` | **passed**, exit 0; 528.17 kB JS chunk warning. |
| `git diff --check` | **passed**, exit 0. |

## Unverified and remaining risk

Real Tauri light/dark/system screenshots, VoiceOver, keyboard focus on the native webview, 2× scaling, minimum window size, and all modal/hover/disabled combinations are **blocked/not_run** by the same desktop-control issue recorded in M0. The browser captures now include recovery dialogs, the shared More menu, Capture Settings, Filter Composer, Stack panel, first-value guide and Privacy Cover, but other secondary screens and literal dark colors remain; full CR-04 and alpha.2 visual acceptance are still open. There is no in-app manual appearance selector yet; system preference is the only implemented switch. No privacy, security, license, permission, network or stored-data boundary changed. Rollback: `git revert a22dc33` for first-value/Cover, `git revert 4a7a635` for Stack panel, `git revert 968045f` for Filter Composer, `git revert af4dac4` for Capture Settings, `git revert fa94261` for the action menu, `git revert 99408b8` for recovery dialogs or `git revert d4867cfea81b1cc1a528d56fbcc55f583e4e7e38` for the foundation, after reviewing later CSS dependencies.
