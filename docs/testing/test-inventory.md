# Test inventory

- **Assessment date:** 2026-08-23
- **Scope:** ClipRiva `2.0.0-alpha.1` smart-workflows engineering checkpoint; counts are an
  inventory, not a coverage percentage, signed-artifact result or real-device evidence.
- **Pre-change baseline:** the Rust suite passed 95 tests with 1 ignored and the frontend passed
  91/91 across 14 files on 2026-07-28.
- **Final v1.0 engineering checkpoint:** the frontend passed 115/115 across 15 files; Rust passed
  112 with 1 existing release benchmark ignored; frontend lint/typecheck/build and Rust fmt/Clippy
  passed. Native Local Link includes a same-host two-service TCP/Noise-SAS/pairing/receipt test, but
  no single-Mac result is real two-Mac or Beta evidence. The v1 acceptance record is
  `docs/clipriva-1.0-implementation-and-acceptance.md`.
- **Security-hardening checkpoint:** Rust passed 122 tests with the same release benchmark ignored.
  The added media/database cases cover private modes, legacy permission repair, canonical keys,
  integrity/size/type checks, symlink/root rejection, opened-file identity, concurrent no-clobber
  publication and failure before clipboard restore. Full frontend and release verification results
  must still be recorded for the exact final commit.
- **v1.5 engineering checkpoint:** `pnpm release:verify` passed on 2026-08-01: release-version
  consistency, frontend lint/typecheck/build and 145/145 tests across 16 files passed; Rust fmt/Clippy and 193 tests
  passed with the one release benchmark normally ignored. The benchmark was then run explicitly in
  release mode and searched a synthetic 10,000-item database in 60.995416 ms. This is local
  engineering evidence only; the two-real-Mac, network/sleep recovery, packet/storage scan and
  signed-install gates remain No-Go.
- **v2.0 alpha pre-candidate checkpoint:** 222/222 frontend tests across 26 files and 234 Rust tests
  passed with the existing 10,000-Item release benchmark ignored. The final candidate must repeat
  lint/typecheck/build, Rust fmt/Clippy/tests and the full suite from a clean exact commit. Physical
  Stack/global-shortcut, Apple Shortcuts/CLI, Accessibility, VoiceOver and signed-app behavior
  remain manual gates.

| Module | Test type | Current coverage | Main risk | Priority |
| --- | --- | --- | --- | --- |
| App shell and main workflows | Vitest + Testing Library (`src/app/App.test.tsx`) | Main workspace rendering, navigation, Smart Collection create/open/rename/delete, core privacy copy and browser-adapter flows | Desktop-only lifecycle differences | High |
| Clipboard presentation | Vitest + Testing Library (`ClipboardCard`, `ClipboardInspector`, `CleanupPreviewDialog`, `RecycleBinDialog`) | Rendering, previews, item inspection, bounded local tag editing, cleanup, deletion, and recovery UI | Visual regressions and desktop integration gaps | High |
| Capture settings | Vitest + Testing Library (`CaptureSettingsDialog`) | Capture pause, privacy controls, distinct Quick Paste/Stack shortcut validation and default-off capability-specific automation controls | Permission and policy mismatch with native runtime | High |
| Browser repository | Vitest (`browserRepository.test.ts`) | In-memory CRUD, Smart Collections/Notes parity, tag normalization/search, command fixtures, local diagnostics, Filter pipelines, Quick Paste recency and recovery behavior | Browser preview diverging from SQLite behavior | High |
| Exact duplicate grouping | Vitest (`exactDuplicateGroups.test.ts`) | Presentation grouping rules for exact versions | Incorrect grouping can hide or misrepresent history | High |
| Native capture and policy | Rust unit tests (`clipboard/*`) | Text/image/rich/file capture, source attribution, sensitive-content policy, and monitor behavior | macOS clipboard/permission behavior | High |
| Local persistence and media | Rust unit tests (`db/mod.rs`, `media/mod.rs`) | Migrations through 26, SQLite repository, bounded tags and Smart rules, Item-owned Note/FTS lifecycle, retention, recycle bin, blob lifecycle, private mode repair, canonical-key/type/size/device-inode/SHA-256 validation, no-clobber publication, safe restore failure and local search | Data loss, migration compatibility, blob cleanup and residual same-user directory-component races | High |
| Quick Paste | Vitest plus Rust unit tests (`QuickPasteOverlay`, `db/mod.rs`, `quick_paste.rs`) | Zero-input recent-activity order, one-pin visibility, tag/command matches, keyboard paths, restore/direct-paste outcomes and permission/focus fallbacks | User-visible paste failure or unexpected input synthesis | High |
| Local Link UI/state | Vitest (`LocalLink*.test.tsx`, `LocalLink.browserRepository.test.ts`) and Rust unit tests | Default-off controls, native DTO mapping, four device projections, selected-text eligibility/256 KiB limit, truthful failure-step projection, send metadata polling, opaque reveal intent, keyboard/focus paths, diagnostic IDs, terminal-summary clear and fail-closed presentation | Browser fixture being misreported as native/network evidence | High |
| Local Link migration/consent | Rust migration and service tests | New installs and preview-era upgrades reset networking off; only explicit enable starts native transport, and pairing/Send fail closed on an unavailable or untrusted peer | Preview consent silently starting a listener or legacy metadata becoming trust | Critical |
| Local Link identity/Keychain | Rust identity-store/service unit tests; real-macOS lifecycle matrix remains untested | Generate/reuse/format/failure behavior, `AccessibleWhenUnlockedThisDeviceOnly`, missing-with-peers fail-closed and reset-to-`needsRePairing`; signed/reinstall/denied Keychain behavior remains a gate | Silent identity replacement, private-key disclosure or reinstall/upgrade loss | Critical |
| Local Link discovery/listener | Discovery metadata plus native listener unit tests | Explicit enable creates random instance/host and `v`/`pair` allow-list routing; no real mDNS permission, firewall or packet evidence | Native discovery being misreported as real-Mac evidence | Critical |
| Local Link Noise protocol | Rust unit/UnixStream plus same-host TCP and malformed-input tests | `Noise_XX_25519_ChaChaPoly_BLAKE2s`, bounded framing/chunks, bilateral SAS pairing, peer/transfer ownership, v3 `unknown`/`pending`/`terminal` schema, legacy StatusResponse rejection, metadata-only status socket roundtrip and no plaintext codec; same-host evidence is not cross-device evidence | Same-host TCP being misreported as two-process/cross-device authentication or delivery | Critical |
| Local Link reconciliation | Rust DB/service tests plus real same-host TCP fault injection | No-replay boundary, post-boundary provisional Cancel plus dropped Receipt, persistent 20-second active-time deadline, four-endpoint / 1.5-second query / 500 ms I/O limits, restart reuse, measured lifecycle pause extension, generation invalidation, stalled socket cutoff and continued retry after terminal DB failure | Physical sleep/path timing and cross-device failure behavior remain a real-Mac gate | Critical |
| Local Link pending-content boundary | Rust service/storage state tests plus static DTO/IPC tests; real inbound disk/log/IPC scan remains untested | Autonomous deadline cleanup, implemented terminal/disable/revoke/exit cleanup, public DTO without preview/body and opaque-ID/void reveal seam; already-visible native view system lifecycle is untested | Future body persistence or false claim that AppKit/lock lifecycle was tested end-to-end | Critical |
| Smart Collections and Notes | Rust repository/policy tests plus Vitest (`SmartCollectionDialog`, `ClipboardNoteEditor`, App and Quick Paste) | Bounded dynamic rules, note size/sensitivity/delete behavior, dedicated search, ordinary DTO exclusion, recycle/restore/cascade and accessible create/edit/delete flows | Sensitive metadata disclosure, stale FTS or rule/UI divergence | High |
| Labs and Filter Composer | Rust unit tests (`context/*`, `actions.rs`, `db/mod.rs`) plus Vitest (`FilterComposerDialog`, Inspector, Quick Paste) | Redaction, 1–8-step evaluator parity, live preview, edit/delete/reorder, content-free audit, unique contextual shortcuts and input/dialog/IME guards | Derived-data disclosure, evaluator divergence or accidental remote boundary | High |
| Sequential Stack | Rust state/monitor tests plus Vitest (`ClipboardQueuePanel`, App, Quick Paste, Settings) | Bounded collection, collect-mode admission, reorder/remove, unavailable Items, single-winner activation, retry/skip, shared main/overlay state and shortcut conflict copy | Native global shortcut/focus/Accessibility behavior and restart/disk evidence | High |
| Local automation | Rust protocol/server/CLI/executor tests plus Settings component tests | Default-off master/capabilities, owner-only socket, bounded frames/errors/stdin, immediate denial and finite metadata/content/copy/library/Filter allow-list | Real Apple Shortcuts shell environment, process lifecycle and permission behavior | Critical |
| Release and real-device behavior | Manual matrix | Documented in `docs/beta-validation-matrix.md` | Signing, installation, Accessibility, and macOS-version differences | High |

## Existing minimum smoke coverage

| Requirement | Current check | Status |
| --- | --- | --- |
| Install dependencies | `pnpm install --frozen-lockfile` | CI command |
| Start a development surface | `pnpm dev` / `pnpm desktop:dev` | Documented local command; manual desktop validation remains required |
| Build | `pnpm build` | Automated in CI |
| Main page can load | App and component Testing Library tests | Automated in CI |
| Local database can initialize | Rust repository/migration tests | Automated in CI |
| No application environment variables | `pnpm test` and `pnpm build` run with no `.env` file | Automated in CI |
| Core modules have a minimal test | Vitest and Rust unit suites | Automated in CI |

## Security invariant coverage

| High-risk invariant | Automated evidence | Remaining manual/proof gap |
| --- | --- | --- |
| Capture policy precedes persistence/send | Clipboard policy/monitor/database and Local Link send-policy tests | Real application/clipboard combinations and release disk scan |
| WebView has no arbitrary privileged primitive | Command-contract/frontend adapter tests and handler/capability review | Built-bundle capability inspection |
| Blob key/type/size/identity/digest fails closed | `media::tests` invalid-key, symlink/socket/directory, oversize, metadata-identity and corruption cases | Stronger directory-fd/no-follow defense is explicitly out of scope |
| Blob publication is private and no-clobber | Permission repair, concurrent writer and corrupt-reuse tests | Clean-install/upgrade/backup restore on the signed macOS candidate |
| Corrupt content does not reach clipboard restore | Database corruption-boundary test and media integrity test | Desktop clipboard observation with a synthetic damaged blob |
| Direct paste requires trust/focus and preserves fallback | `quick_paste::tests` | Accessibility allow/deny/revoke on supported macOS versions |
| Notes remain Item-owned and absent from ordinary/remote metadata | Repository lifecycle, DTO shape, Quick Paste and privacy-boundary component tests | Signed-candidate SQLite/WAL/log scan with synthetic markers |
| Stack state is bounded process memory and advances once | Native state/monitor and main/Quick Paste component tests | Relaunch/disk scan and physical global-shortcut/focus matrix |
| Automation is default-off and capability scoped | Protocol/server/executor/CLI/Settings tests; arbitrary path/SQL/Shell/MCP operations are absent | Real Apple Shortcuts, stale-socket and signed-app lifecycle matrix |
| Local Link requires consent, authenticated pairing, a single winner, at-most-once side effects and truthful `outcomeUnknown` recovery | Migration/service/protocol/replay/receipt/effect-claim tests | Same-SHA two-real-Mac, packet, lock/user-switch and lifecycle matrix; SQLite and the system pasteboard cannot form one ACID transaction |
| Key/body/diagnostic schemas exclude sensitive material | Identity, pending-state, DTO/schema and diagnostics tests | Signed Keychain lifecycle and disk/log/IPC scan |
| Security scan incomplete is not a pass | Threat model and release checklist policy | No external scan was authorized or run in this checkpoint |

## Next testing priorities

1. Run the manual macOS compatibility matrix for clipboard capture, RTF/HTML/files, Quick Paste,
   Stack/global shortcuts, Apple Shortcuts/CLI and Accessibility allow/deny/revoke outcomes.
2. Add regression cases whenever a real data-loss, privacy, or migration defect is fixed.
3. Repeat the 10,000-item Quick Paste benchmark on the frozen signed candidate and record multiple
   runs/p50/p95; the local release-mode single-run pre-gate was 60.995416 ms.
4. Keep Local Link labeled Preview/Alpha and default off until the protocol/Keychain review and the
   50-pairing / 200-transfer two-real-Mac, packet/data-boundary, replay, lock/user-switch,
   sleep/wake, AP-isolation and revocation matrix in `docs/beta-validation-matrix.md` passes.
   Browser fixtures, injected loopback, same-host TCP and a single-Mac build are not evidence for that gate.
5. Verify the final macOS application bundle contains only the required Bonjour/Local Network
   declarations and that Tauri capabilities do not grant WebView socket or Keychain access.
6. Keep Local Link Preview/Alpha and default off until the real-Mac listener/discovery/pairing/
   delivery evidence is completed and reviewed; local automation does not authorize Beta.
