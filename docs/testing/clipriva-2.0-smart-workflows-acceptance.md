# ClipRiva 2.0 alpha smart-workflows acceptance

- **Status:** implementation-complete user-story matrix; exact candidate evidence is recorded by the
  final clean-commit verification and task handoff
- **Version target:** `2.0.0-alpha.1`
- **Date:** 2026-08-23
- **Evidence rule:** every result is Unverified until recorded against an exact clean commit

Fixtures must be synthetic. Reports must not contain real clipboard content, Notes, Collection
names, local paths, automation queries, socket paths or credentials.

## A. Version and migration

| ID | User story / acceptance | Required evidence | Status |
| --- | --- | --- | --- |
| V-01 | As a tester, I see one truthful alpha version everywhere rather than 1.5/2.0 conflict. | Version script asserts package/Cargo/Tauri `2.0.0-alpha.1`; README, changelog and versioning copy review. | Unverified |
| V-02 | As an existing user, my 1.5/2.0 working-candidate data upgrades without losing Items, Saved state, Collections, OCR, recycle or Local Link metadata. | Rust migration fixture through migrations 22–26 and schema assertions. | Unverified |
| V-03 | As a release reviewer, I can distinguish this alpha from a signed/public release. | Release-copy/static test; no tag/sign/notarize/publication action. | Unverified |

## B. Smart Collections and Notes

| ID | User story / acceptance | Required evidence | Status |
| --- | --- | --- | --- |
| SC-01 | I add or edit a Note without changing Item body, clipboard, copy count, Saved state or retention. | Rust repository + Inspector component test. | Unverified |
| SC-02 | I find a clip by Note in History and Quick Paste without Note text appearing in ordinary list DTOs. | Rust FTS/ranking, API shape and Quick Paste tests. | Unverified |
| SC-03 | Note edit/delete updates FTS transactionally; failure keeps the prior Note/index. | Rust transaction test. | Unverified |
| SC-04 | Recycled Notes are not searchable, restore makes them searchable, and permanent deletion cascades Note/FTS rows. | Rust lifecycle test. | Unverified |
| SC-05 | Sensitive, blank and >16 KiB Notes are handled safely: blank deletes, invalid input does not replace the prior value or pause Capture. | Policy/repository/component tests and marker scan. | Unverified |
| SC-06 | I save current finite filters as a Smart Collection; text search/manual Collection are explicitly excluded. | Smart dialog and App journey test. | Unverified |
| SC-07 | New captures or Saved/use changes dynamically change Smart results, including matches beyond the first page. | >100 Item repository and polling/component test. | Unverified |
| SC-08 | Empty rules, unknown kinds, nesting, >20 definitions and manual/Smart name conflicts fail closed. | Validation/migration/browser parity tests. | Unverified |
| SC-09 | Rename/delete changes only the Smart definition and never deletes, recycles, unsaves or mutates Items. | Repository + confirmation-copy test. | Unverified |
| SC-10 | Search and temporary filters inside a Smart scope use AND semantics and show accurate heading, chips and empty state. | App keyboard/user-journey test. | Unverified |
| SC-11 | Privacy Cover prevents Note queries and omits Note/rule/source/content values from DOM and accessible names. | Bootstrap/DOM privacy-boundary tests. | Unverified |
| SC-12 | Dialog and Note editor support keyboard focus, Escape, status/error announcements and focus return. | Component accessibility tests. | Unverified |

## C. Filter Composer

| ID | User story / acceptance | Required evidence | Status |
| --- | --- | --- | --- |
| FC-01 | Existing built-in/custom single Actions migrate to one-step pipelines with identical names/results. | Migration + evaluator compatibility tests. | Unverified |
| FC-02 | A 3-step Filter executes in displayed order; reordering changes output consistently in Rust and browser adapter. | Table-driven parity tests. | Unverified |
| FC-03 | Live preview uses the Rust evaluator, writes no clipboard/audit/database state and ignores stale responses. | Command, database-count and component race tests. | Unverified |
| FC-04 | Invalid JSON identifies the failing step, disables Apply and leaves the previous clipboard untouched without echoing input in errors. | Evaluator/command/component test. | Unverified |
| FC-05 | Apply writes once and only then records one immutable content-free audit with finite step IDs/count/outcome. | Native command/effect-order and audit tests. | Unverified |
| FC-06 | One-to-eight steps, 256 KiB output, unique name/shortcut and immutable built-in boundaries are enforced. | Repository validation tests. | Unverified |
| FC-07 | A Filter shortcut acts only on an explicitly selected text Item and never in inputs, dialogs, IME composition or non-text content. | Inspector/Quick Paste keyboard tests. | Unverified |
| FC-08 | Labs off hides UI and every native Filter command fails closed without creating derived state. | Command/component/privacy-boundary tests. | Unverified |

## D. Stack

| ID | User story / acceptance | Required evidence | Status |
| --- | --- | --- | --- |
| ST-01 | I add selected Items in visible order; duplicates and the 20-Item limit produce accurate feedback. | Native state + App bulk journey test. | Unverified |
| ST-02 | Collect mode adds only newly persisted, policy-approved external copies; pause, deny, sensitive, unsupported and self-write events do not enter Stack. | Monitor/native integration tests. | Unverified |
| ST-03 | Mouse and keyboard reorder/remove only waiting Items; completed/busy entries stay fixed and focus is preserved. | State-machine + component accessibility tests. | Unverified |
| ST-04 | Current/next previews resolve by ID; deleted/recycled Items show Unavailable and are never substituted. | Repository/Stack integration test. | Unverified |
| ST-05 | Successful activation advances exactly once; repeated keys and stale completion tokens do not advance twice. | Native concurrency/state tests. | Unverified |
| ST-06 | Permission/focus/compatibility failure keeps the cursor and exposes truthful Retry, Copy and advance, and Skip recovery. | Quick Paste/global-shortcut outcome tests. | Unverified |
| ST-07 | Main and Quick Paste show one state; restart clears it; synthetic Stack IDs/bodies are absent from SQLite/WAL/preferences/logs/diagnostics/exports/Local Link. | Cross-window, relaunch and marker scans. | Unverified |
| ST-08 | Stack and Quick Paste shortcuts cannot conflict; replacement failure rolls back; empty/busy/Cover states are explained. | Registration unit/manual macOS tests. | Unverified |
| ST-09 | VoiceOver receives ordered Current/Next/Completed/Unavailable and progress updates without hidden clipboard bodies. | Component/manual accessibility evidence. | Unverified |

## E. CLI and Apple Shortcuts bridge

| ID | User story / acceptance | Required evidence | Status |
| --- | --- | --- | --- |
| AU-01 | Fresh install and upgrade leave automation and every capability off with no listener socket. | Migration, startup and filesystem tests. | Unverified |
| AU-02 | Master enable without a capability still denies metadata/content/copy/library/filter requests independently. | Executor/protocol matrix. | Unverified |
| AU-03 | Metadata search is bounded and contains no body, preview, OCR, Note, Collection, filename/path or internal error. | Protocol shape + synthetic marker scan. | Unverified |
| AU-04 | Content authorization returns only the explicitly requested text; disabling it rejects the next request immediately. | Server integration test. | Unverified |
| AU-05 | Clipboard write succeeds once or fails without changing copy count/audit and never returns body in status/error output. | Native effect-order test. | Unverified |
| AU-06 | Filter automation accepts only saved allow-listed pipelines; unknown/Shell/oversized/malformed requests fail closed. | Protocol fuzz and evaluator tests. | Unverified |
| AU-07 | stdin preserves Unicode, newlines, quotes and JSON; argv, logs and finite errors contain no supplied marker. | CLI subprocess and static marker tests. | Unverified |
| AU-08 | App-not-running, disabled, stale socket, timeout, oversized frame, partial/malformed JSON and concurrent requests return stable finite codes. | CLI/server protocol tests. | Unverified |
| AU-09 | Settings provides understandable enable, capability confirmation and immediate-disable keyboard flows; Cover shows no automation output. | Settings/App accessibility tests. | Unverified |
| AU-10 | An Apple Shortcut using Run Shell Script passes stdin and receives stdout; missing permission gives actionable bounded failure and disable takes immediate effect. | Manual macOS 13/14/15/26 matrix on one frozen artifact. | Unverified |
| AU-11 | Static boundaries contain no arbitrary SQL/path/shell/MCP and the CLI never opens the database directly. | Architecture/static test. | Unverified |

## F. Completion and synchronization

1. Focused component, browser, Rust repository, migration, state-machine, CLI/protocol and privacy
   tests pass.
2. `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build` pass.
3. `cargo fmt --check`, Clippy with warnings denied and `cargo test --locked` pass.
4. `pnpm release:verify`, privacy marker scans, large-history/Quick Paste benchmark and relevant
   release scripts pass.
5. Node/Rust license metadata is regenerated and the audit records no unreviewed direct dependency,
   missing metadata or restricted-license finding.
6. Candidate commit is created; the same checks are rerun with no tracked or untracked source
   changes.
7. Only that exact commit is pushed. `scripts/verify-git-sync.sh` must prove local/remote parity.
8. No tag, release, signing, notarization or application publication occurs in this task.
