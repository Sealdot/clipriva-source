# ClipRiva 2.0 acceptance specification

- **Status:** working-candidate acceptance specification; execution evidence is not attached here
- **Date:** 2026-08-12
- **Scope:** approved P0-P2 product upgrade on `codex/clipriva-p0-p2-product-upgrade`
- **Release effect:** none; this document does not authorize push, tag, signing, notarization,
  distribution or Local Link promotion

Every case below starts **Unverified**. A code diff, passing unit test, screenshot, loopback harness
or this written plan is not evidence that a manual/physical gate passed. Record results against an
exact commit and, where relevant, the same frozen signed application artifact. Fixtures must be
clearly synthetic and must not contain real clipboard values, Collection names, source paths,
device identities, Keychain material or endpoints.

## Evidence classes

| Class | Acceptable evidence | Does not prove |
| --- | --- | --- |
| Automated | Named test, command, exact commit, exit status and retained non-sensitive report | Real macOS permission UI, screen sharing, two-device networking, signing or installation |
| Browser/component | DOM/accessibility assertions and screenshots from synthetic fixtures | Native persistence, Keychain, Vision, window protection or real network behavior |
| Native integration | Rust/native tests, migration/schema inspection and synthetic local disk/log scans | Signed clean-machine behavior or two physical Macs |
| Manual macOS | Exact OS/hardware/build, steps, expected/actual result and redacted artifact | Other OS/hardware combinations |
| External release | Frozen signed artifact, two-real-Mac matrix, packet/disk/log scans and reviewer decision | Nothing beyond the recorded artifact and matrix |

## P0 — trust and core flow

| ID | Acceptance | Required checks | Evidence | Status |
| --- | --- | --- | --- | --- |
| P0-ONB-01 | One first-value task | After privacy confirmation, copy two synthetic values; exactly one recovery guide is visible and no Incoming/Local Link coaching competes with it. | Component plus browser screenshot | Unverified |
| P0-ONB-02 | Guide completion | Recover the first value through Quick Paste; relaunch and confirm the guide does not return. | Component plus native/manual relaunch | Unverified |
| P0-SURF-01 | One main surface | Open Details, Settings, Incoming, Transfers, Send, cleanup and Recycle Bin in sequence; each atomically closes the previous surface. | Reducer/component tests | Unverified |
| P0-SURF-02 | Escape and focus | Escape closes only the active surface and focus returns to the invoking control. | Keyboard/component test | Unverified |
| P0-LL-01 | Disabled status projection | With Local Link disabled and stale trusted/online fixture state, no device is presented Online or Sendable. | Component/adapter test | Unverified |
| P0-LL-02 | Enabled readiness projection | A device becomes Sendable only after enabled preferences and readiness/authenticated presence agree. | Component/adapter test | Unverified |
| P0-NAV-01 | Product hierarchy | Primary navigation is History, Saved and Collections; Local Link remains secondary and visibly labelled Preview. | DOM/accessibility assertion | Unverified |
| P0-SEARCH-01 | Search scope | Visible label, accessible name and placeholder identify History, Saved or current Collection scope. | Component test | Unverified |
| P0-A11Y-01 | Readability | Informational text is at least 11 px and ordinary copy meets 4.5:1 contrast at normal/minimum supported sizes. | Computed-style audit and screenshots | Unverified |

## P1 — Saved, Collections and daily reuse

| ID | Acceptance | Required checks | Evidence | Status |
| --- | --- | --- | --- | --- |
| P1-FILTER-01 | Server-side filtering before limit | Seed more than 100 synthetic Items; a matching later Item remains discoverable by combined query/kind/source/Collection/time/Saved/recent-use filters. | Native repository test | Unverified |
| P1-FILTER-02 | Safe combinations | Combine every supported structured filter with quotes, wildcard characters and empty/null values; results are correct and SQL is parameterized. | Native repository test | Unverified |
| P1-COL-01 | Legacy productization | Existing pinned Items render immediately as Saved and existing tags as Collections without content duplication or migration loss. | Migration plus UI test | Unverified |
| P1-COL-02 | Atomic Save | Save with one or more Collections commits atomically. Removing Saved preserves Collection membership and returns the Item to normal retention. | Transaction/repository test | Unverified |
| P1-COL-03 | Rename/merge | Renaming into an existing Collection merges membership case-insensitively without duplication and respects eight names/24 visible characters. | Repository test | Unverified |
| P1-COL-04 | Delete membership only | Deleting a Collection removes matching memberships but does not delete, recycle, unsave or expose clipboard content. | Repository plus UI confirmation test | Unverified |
| P1-COL-05 | Sensitive metadata exclusion | Marker-like Collection names do not appear in Local Link payload/summary DTOs, local diagnostics, support bundle, automatic export or application logs. | Schema tests plus synthetic marker scans | Unverified |
| P1-ACT-01 | Unified Action Panel | Context exposes only valid actions; Arrow/Home/End/Escape work and trigger focus is restored. | Keyboard/component test | Unverified |
| P1-ACT-02 | Destructive protection | Removing a Saved Item through cleanup uses Recycle Bin preview and the explicit include-Saved/pinned compatibility contract. | Component plus native test | Unverified |
| P1-QP-01 | Quick Paste clarity | Default footer has only Navigate, Restore and More help groups; secondary controls remain keyboard reachable. | DOM/accessibility test | Unverified |
| P1-SET-01 | Settings structure | Exactly three primary tabs exist: General, Privacy and Shortcuts. Advanced local tools and Local diagnostics & data are collapsed secondary sections; save, draft, focus trap and Arrow/Home/End behavior remain intact. | Component test | Unverified |

Collection names are user data even though their database column is still named `label`. Test
reports must use synthetic markers and must not paste real Collection names into public artifacts.

## P2 — Privacy Activity

| ID | Acceptance | Required checks | Evidence | Status |
| --- | --- | --- | --- | --- |
| P2-PA-01 | Empty/list/clear | Empty state is truthful; finite reason/source/time rows render newest first; Clear removes Activity only and leaves History, preferences, diagnostics and Local Link unchanged. | Repository/IPC/component tests | Unverified |
| P2-PA-02 | Bounds | Insert more than 20 synthetic events across more than 7 days; retrieval and cleanup expose no more than 20 rows and no expired row. | Migration/repository test | Unverified |
| P2-PA-03 | Content-free schema | Table/DTO/log/diagnostic/export scans show no clipboard body, preview, hash, Item ID, filename/path, Collection, peer or endpoint. | Static schema assertion plus marker scan | Unverified |
| P2-PA-04 | Diagnostics independence | Diagnostics off/on does not gate Activity collection or viewing; Activity Clear does not clear diagnostics and diagnostics Clear does not clear Activity. | Native/component test | Unverified |

## P2 — sequential Queue

| ID | Acceptance | Required checks | Evidence | Status |
| --- | --- | --- | --- | --- |
| P2-SQ-01 | Bounded synchronized order | Enqueue follows visible order, deduplicates IDs, caps at 20 and projects identical order/cursor to main window and Quick Paste through named events. | State-machine/component integration | Unverified |
| P2-SQ-02 | Exactly-once advance | Successful Restore/Paste advances once. Failed paste and duplicate/concurrent completion do not advance; Retry, Copy and advance, and Skip are explicit distinct actions. | State-machine/native activation test | Unverified |
| P2-SQ-03 | Unavailable Item | Delete/recycle the current Item; Queue reports Unavailable and never substitutes another Item or cached content. | Integration test | Unverified |
| P2-SQ-04 | Process-memory only | Populate Queue, restart application and confirm it is empty. Synthetic Queue ID/body markers are absent from SQLite/WAL, blobs, preferences, logs, diagnostics and exports. | Native relaunch plus marker scan | Unverified |
| P2-SQ-05 | No Local Link queue | Queue cannot enqueue an incoming body, drive Send, retain/retry a network payload or alter Local Link transfer metadata. | Boundary/schema test | Unverified |

“Process memory only” is a ClipRiva serialization rule, not a guarantee about macOS swap,
hibernation or crash dumps.

## P2 — Privacy Cover

| ID | Acceptance | Required checks | Evidence | Status |
| --- | --- | --- | --- | --- |
| P2-PC-01 | DOM/accessibility omission | With Cover active, synthetic clipboard/OCR text, Collection/source/filename values and image descriptions/bytes are absent from DOM text, attributes, accessible names and hidden preview nodes on every surface. | Component/e2e DOM scan | Unverified |
| P2-PC-02 | Immediate closure | Enabling Cover closes inspector, preview and any native Local Link reveal; disabling it does not silently reopen them. | Component plus native/manual test | Unverified |
| P2-PC-03 | No launch flash | Persist Cover enabled, relaunch and capture first paint; no protected value renders before preference/bootstrap resolution. | Native browser trace/video with synthetic marker | Unverified |
| P2-PC-04 | Best-effort native hardening | Supported Tauri/macOS window content-protection state is applied when Cover is on and reverted when off. Any observed platform limitation is recorded, not hidden. | Native adapter test plus manual macOS check | Unverified |
| P2-PC-05 | No overclaim or permission | Bundle entitlements and runtime show no Screen Recording, Camera or Photos request; UI/help calls protection best-effort and never claims to block all screenshots/sharing/recording. | Bundle/static copy inspection | Unverified |

External-camera and OS/third-party capture behavior is explicitly outside the guaranteed boundary.
A successful native flag call does not prove global screen-capture prevention.

## P2 — local image OCR

OCR is included as an explicit manual macOS Vision action. Automatic extraction is not implemented;
tests must not imply an automatic capture path exists.

| ID | Acceptance | Required checks | Evidence | Status |
| --- | --- | --- | --- | --- |
| P2-OCR-01 | Manual-only local engine | Fresh install/upgrade has no automatic OCR path. Manual extraction uses macOS Vision on an existing local image and makes no network request or new OS-permission request. | Migration/native adapter and network test | Unverified |
| P2-OCR-02 | Searchable bounded output | Synthetic Chinese/English images produce deterministic locally searchable “Text in image” evidence; output over 256 KiB fails or truncates according to a documented bounded rule without partial orphan rows. | Native Vision test on macOS plus repository test | Unverified |
| P2-OCR-03 | Sensitive fail-before-store | Synthetic secret-like OCR output is not persisted or indexed. No automatic new-image extraction path exists. | Native policy/database marker scan | Unverified |
| P2-OCR-04 | Lifecycle | OCR text/FTS follows owning image through recycle and restore; permanent Item deletion cascades. Clear all extracted text preserves images and other History. | Migration/repository test | Unverified |
| P2-OCR-05 | Cancellation/resource bound | One cancellable worker processes jobs; cancel/restart/error leaves no partial OCR/FTS row and bounded fixtures cannot create unbounded CPU/memory/storage work. | Native worker tests and benchmark | Unverified |
| P2-OCR-06 | Boundary exclusion | OCR markers are absent from Local Link DTO/frames/summaries, Privacy Activity, diagnostics/support exports, application logs and network traces. | Schema assertions plus disk/log/packet marker scans | Unverified |

## Cross-boundary completion checks

Run on the final merged candidate and record exact outputs; this list is a requirement, not a claim
that the commands have passed in this documentation task.

1. `pnpm lint`, `pnpm typecheck`, `pnpm test` and `pnpm build`.
2. `cargo fmt --check`, Clippy with warnings denied and `cargo test --locked`.
3. Privacy-boundary, migration, large-history, Queue restart and synthetic marker-scan tests.
4. Normal/minimum-size screenshots, keyboard-only and VoiceOver review using synthetic content.
5. Dependency/license/SBOM review if a manifest or lock changed. macOS Vision framework use must
   not be replaced with an undeclared remote provider.
6. Re-review `PRIVACY.md`, data flow, module boundary and threat model after the final feature merge.

Local Link is evaluated separately by
[`docs/release/local-link-ga-evidence-gate.md`](../release/local-link-ga-evidence-gate.md). Until
that gate is fully evidenced and approved, its acceptance status is Preview / No-Go regardless of
P0-P2 repository test results.
