# CR-05 workspace evidence

**Source commits:** `8f75ec0` (on-demand workspace), `ec3346d` (Inspector child-key correction), local and unpushed. **Status:** on-demand workspace source implementation complete; native layout/accessibility and installed-data acceptance remain open.

## Delivery and locations

- `src/app/App.tsx`: default workspace is source rail plus result list. Selecting a card opens its Item details; at widths below 1000 px details replace the list, while wider windows show a third column. Back restores focus to the originating card when it still exists, otherwise the list. The detail Item remains attached to its original ID while a search or filter changes. Sending from details retains that Item snapshot rather than following a later list selection.
- `src/features/clipboard/ClipboardNoteEditor.tsx` and `ClipboardInspector.tsx`: an edited Note stays a draft until Save Note succeeds. Navigation from details asks the user to keep editing or discard; a canceled move retains the draft. A successful save permits navigation and preserves the existing Item-owned Note. Saved and Collection data and labels continue using their existing repository facts.
- `src/styles/global.css` and `src/styles/theme.css`: responsive list/detail layout, shared light/dark colors for the workspace and Inspector, Note editor layout, visible focus, and an accessible draft decision surface. No icon, font, illustration, or external content was introduced.
- `src/app/App.test.tsx`: synthetic tests exercise default absence of details, open/return and focus, Labs actions behind details, unsaved draft retention/discard, saved Note persistence, Settings/Local Link surface replacement, and existing History/Collection/Saved behavior.
- `src/features/clipboard/ClipboardInspector.tsx` and `.test.tsx`: when Labs was enabled, the Note Editor and Actions Panel were sibling children with the same Item-ID React key. The Inspector itself already has an Item-ID key in `App.tsx`, so the two redundant child keys were removed. A focused test renders Notes and Labs together and rejects a duplicate-key console warning. No Note, Filter, Saved or Collection data path changed.

## Executed checks

These commands ran on the source tree before `8f75ec0`; final clean-tree checks after this evidence commit are reported in the handoff.

| Command | Result |
| --- | --- |
| `pnpm lint` | **passed**, exit 0; 96 files, four pre-existing broken `logs/soak` symlink warnings. |
| `pnpm typecheck` | **passed**, exit 0. |
| `pnpm test` | **passed**, exit 0; 26 files / 234 tests. |
| `pnpm build` | **passed**, exit 0; 527.63 kB JS chunk warning. |
| `pnpm exec vitest run src/app/App.test.tsx` | **passed**, exit 0; 35 tests before the last draft/focus test additions; included in the 234-test full run. |
| `git diff --check` | **passed**, exit 0. |

For source commit `ec3346d`, `pnpm exec vitest run src/features/clipboard/ClipboardInspector.test.tsx` **passed**, exit 0 (5 tests); `pnpm lint` **passed**, exit 0 with the four existing broken `logs/soak` symlink warnings; `pnpm typecheck` **passed**; `pnpm test` **passed**, 26 files / 237 tests; `pnpm build` **passed** with the 528.16 kB JS chunk warning; and `git diff --check` **passed**. The first focused test run failed because the new assertion used the wrong accessible label (`Local note`); the existing label is `Searchable Note`. The first lint run then found formatting in that assertion. Both test-only mistakes were corrected and the named commands passed before commit.

## Captures and unverified layers

- [Default list, light 920×620](screenshots/workspace-list-browser-mock.png), [detail replacing list, light 920×620](screenshots/workspace-detail-narrow-browser-mock.png), [wide list plus detail, light 1280×800](screenshots/workspace-detail-wide-browser-mock.png), [wide dark 1280×800](screenshots/workspace-detail-dark-browser-mock.png), and [unsaved Note decision, light 920×620](screenshots/workspace-unsaved-note-browser-mock.png) were captured at 1× from local Vite in headless Chrome. All are **browser-mock** with the repository's synthetic Items and existing dependency icons. The unsaved text is synthetic; no live clipboard content or third-party asset was added. CDP computed layout showed one 735 px result column at 920 px, and 694.859 px list plus 374.141 px details at 1280 px.
- The first visual pass exposed dark workspace backgrounds under light cards and nearly invisible Inspector text. Token overrides and Note layout were corrected before the listed captures. This browser review does not establish native macOS rendering, 2× scaling, minimum-size behavior, VoiceOver order, or actual clipboard/target-app outcomes: those are **blocked/not_run**.
- [Labs with Note](screenshots/labs-note-browser-mock.png) and [Labs with Actions](screenshots/labs-actions-browser-mock.png) were captured at 1280×800, 1×, from local Vite/headless Chrome using the repository's synthetic code Item. Labs was enabled only in the browser fixture. The duplicate `demo-code` React-key warning was reproduced before `ec3346d` and stopped after a fresh browser load with both sections present; the focused test checks the same condition. These are **browser-mock** captures and console observations, not native acceptance. No real clipboard content, user Note or third-party asset was involved.
- A real existing-data upgrade with Saved/Collections/Notes/setting retention and the full secondary-surface light/dark matrix are **not_run**. The browser fixtures exercise representative facts, not a migrated user database. An unsaved Note is intentionally process memory only; quitting or invoking Privacy Cover discards it, while Privacy Cover still hides content immediately. The local incoming-request surface can claim focus when it becomes visible; the focused return test suppresses that separate pending request.

## Impact and rollback

No permission, network destination, stored-data type, dependency, migration, encryption, Local Link gate or release behavior changed. The draft and send Item snapshot live only in the current webview. Privacy/security impact is limited to the visible local workspace; license impact is none. Rollback: revert `ec3346d` for the child-key correction or `8f75ec0` for the workspace after checking later dependencies; neither changes stored schema or user data.
