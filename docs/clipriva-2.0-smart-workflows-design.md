# ClipRiva 2.0 alpha smart-workflows design

- **Status:** implementation specification
- **Version target:** `2.0.0-alpha.1`
- **Branch:** `codex/clipriva-2.0-smart-workflows`
- **Date:** 2026-08-23
- **Release effect:** none; this work does not authorize a tag, release, signing, notarization or
  promotion of Local Link from Preview

## 1. Outcome and version decision

This iteration turns the implemented 2.0 working-candidate experience into one coherent executable
alpha and adds four local productivity workflows:

1. rule-based Smart Collections and item-owned searchable Notes;
2. a bounded, allow-listed Filter Composer with live preview;
3. a native process-memory Stack for collecting, ordering and sequentially activating clips;
4. default-off, capability-scoped local automation through the ClipRiva executable, with an Apple
   Shortcuts “Run Shell Script” bridge.

The repository currently executes as `1.5.0` while its active product and privacy documents describe
a 2.0 working candidate. The next executable version is therefore `2.0.0-alpha.1`, not stable
`2.0.0` and not `2.1.0`. The alpha identifier fixes the source-of-truth mismatch without claiming
that physical-mac, signing, notarization or release gates passed. `package.json`, Cargo, Tauri,
README, changelog and versioning guidance must agree. Historical 1.5 evidence remains unchanged.

## 2. Product principles

- Core remains useful with no account, Internet service, model provider or automation permission.
- Notes, Smart rules and Filter definitions are local sensitive metadata.
- No user-authored SQL, shell script, regular expression, filesystem path, remote URL, MCP or Agent
  interface is introduced.
- Stack contains item IDs and transition state only, never clipboard bodies, and clears on process
  restart.
- Automation is off after fresh install and upgrade. Every request is checked by native code against
  explicit, finite capabilities; enabling the service is not equivalent to granting every
  capability.
- User-facing success text describes only effects ClipRiva can prove.

## 3. Smart Collections and searchable Notes

### 3.1 Stored model

Migration 22 adds two independent data types:

- `clipboard_item_notes`: one UTF-8 Note per Item, at most 16 KiB, owned by and cascade-deleted with
  the Item. Blank input deletes the Note. Notes remain attached while an Item is in Recycle Bin but
  are not searchable there; restore makes them searchable again. A Note does not implicitly Save
  its Item or extend retention.
- `clipboard_smart_collections`: at most 20 named rule definitions. A rule has schema version 1 and
  combines only the existing finite filters: kinds, source application, Saved state, time window,
  recent-use state and Local Link source. It cannot reference another Collection or contain SQL,
  regex or a search string.

Manual and Smart Collection names share a case-insensitive namespace. Smart Collections may be
empty and survive History cleanup until explicitly deleted. Deleting or renaming one changes only
its rule definition and never modifies clipboard content, Saved state or manual membership.

Notes receive a dedicated FTS5 index. History ranks body matches before OCR, Notes and metadata.
Quick Paste may return `matchField = note`, but Note text is loaded only by the explicit Inspector
query and is not copied into every list DTO.

### 3.2 Experience

- The filter panel offers **Save filters as Smart Collection** only when at least one supported
  structured filter is active. Current free-text search and manual Collection scope are explicitly
  excluded from the saved rule.
- Smart Collections appear beside manual Collections with a distinct icon and rule summary. Their
  locked rule chips combine with temporary search and filters using AND semantics.
- The Inspector contains a separate Note editor with explicit Save/Delete, byte-limit feedback and
  the reminder that Notes follow Item retention.
- Sensitive or oversized Notes fail closed and do not replace an existing Note or pause Capture.
- Privacy Cover prevents Note queries and omits rule source labels and Note-match content from the
  DOM and accessibility tree.

## 4. Filter Composer

### 4.1 Execution boundary

Existing Labs Actions migrate from one transform to a pipeline of one to eight ordered steps. The
initial step allow-list is:

- uppercase, lowercase, trim whitespace, normalize whitespace and format JSON;
- remove blank lines, deduplicate lines, sort lines ascending and sort lines descending.

Steps have no arbitrary executable or path parameter. Every intermediate value is capped at 256
KiB. Unknown steps, invalid JSON, over-limit output and malformed stored pipelines fail closed with
finite errors that do not repeat the input.

Legacy built-ins and custom Actions migrate to one-step pipelines without changing results.
Built-ins remain immutable. Custom filters can be created, edited and deleted. An optional unique
shortcut slot `1` through `9` maps to `Command/Control + Option + digit` while Quick Paste or the
Inspector has an explicitly selected text Item; shortcuts never fire in an input, dialog or IME
composition and are not advertised as system-global shortcuts.

Live preview executes the same Rust pipeline used for Apply. Preview writes neither clipboard nor
audit state. Apply computes, writes the system clipboard once, then records one immutable redacted
audit containing the full finite pipeline snapshot. A clipboard-write failure must not produce a
completed audit.

### 4.2 Experience

- New/Edit opens a focused composer with ordered steps, Add, Remove, Move Up and Move Down controls.
- Before/After preview is debounced; stale responses cannot replace a newer preview.
- Invalid steps keep the draft, mark the failing position and disable Apply & Copy.
- Applying from the Inspector leaves the Item selected and shows a local success status. Applying
  from Quick Paste closes only after a proven clipboard write.
- Labs remains the explicit gate in this alpha.

## 5. Queue becomes Stack

### 5.1 Native process state

The Stack becomes native process-memory state so main-window, hidden Quick Paste and the global
shortcut share one source of truth. It contains at most 20 ordered Item IDs, availability, cursor,
collecting state and an in-flight token. It is forbidden from SQLite/WAL, preferences, files, logs,
diagnostics, exports and Local Link and clears at process restart.

Supported transitions are enqueue visible/selected, begin/end collecting, move/remove a waiting
item, reserve activation, activation succeeded/failed, copy-and-advance, skip, mark unavailable and
reset. Completed entries and a busy current entry cannot be reordered. Duplicate and limit-skipped
counts are returned truthfully.

Collect mode observes only newly persisted, policy-approved Items. Paused, excluded, sensitive,
unsupported and ClipRiva self-written clipboard events are never collected. Collect mode never
enables Capture by itself.

### 5.2 Experience and shortcut

- The prior Queue button becomes **Stack** and only opens the panel; it no longer silently adds all
  visible results.
- Bulk selection adds selected Items in visible order. The panel supports keyboard reorder/remove,
  current/next previews, progress and clear feedback for duplicates, limits and unavailable Items.
- A configurable Stack shortcut is registered separately from Quick Paste and cannot conflict with
  it. Triggering it reserves the current Item and uses the existing safe activation path. A proven
  `pasteSent`/restore success advances exactly once; permission, focus or compatibility failure
  keeps the cursor in place and offers Retry, Copy and advance, or Skip.
- Empty, busy, unavailable and Privacy Cover states never substitute another Item silently.

## 6. Controlled CLI and Shortcuts bridge

### 6.1 Runtime model

`ClipRiva automation ...` is handled by the same bundled executable. CLI mode connects to a
versioned Unix-domain socket owned by the running ClipRiva process; it never opens SQLite or blob
storage directly. The listener exists only while automation is enabled, lives in the private app
data directory and is removed immediately on disable/exit. Requests are bounded JSON with unknown
fields denied. Socket permissions restrict access to the current macOS user; documentation must
not claim caller identity isolation among that user’s processes.

Automation has a master toggle plus finite capabilities:

- `history.metadata`: bounded search returning IDs, kinds and timestamps only;
- `history.content`: explicit text-content retrieval;
- `clipboard.write`: copy a selected text Item or filter result to the system clipboard;
- `library.write`: Save/unsave and manual Collection membership;
- `filters.run`: list and apply saved allow-listed Filters.

The alpha omits delete/clear, images/blob paths, OCR bulk export, Direct Paste, Local Link, Stack
control, diagnostics export, arbitrary commands and MCP. Queries and supplied text use stdin rather
than argv. Errors use finite codes and do not include content, query text, paths or raw database
errors.

### 6.2 Commands and Shortcuts

The bounded interface is:

```text
ClipRiva automation status --json
ClipRiva automation search --stdin --json
ClipRiva automation get --id <opaque-id> --json
ClipRiva automation copy --id <opaque-id> --json
ClipRiva automation filter list --json
ClipRiva automation filter apply --id <filter-id> --item <opaque-id> --json
ClipRiva automation saved add|remove --id <opaque-id> --stdin --json
```

Apple Shortcuts can call these commands with **Run Shell Script**, passing dynamic values through
stdin. This is a Shortcuts-callable bridge, not a native App Intents extension. A native Shortcuts
extension, signing/entitlements and MCP remain separate future decisions.

Settings explains the risk, exposes the master and per-capability toggles, shows the current bundle
command path and supports immediate disable. Enabling content read or clipboard write requires an
explicit confirmation within the dialog.

## 7. Delivery slices

1. Commit this design and the user-story acceptance matrix before product code.
2. Implement Smart Collections/Notes and migration 22 with repository/browser/UI tests.
3. Implement Filter pipelines in migration 23 and contextual shortcut slots in migration 26 with
   evaluator, preview, audit and UI tests.
4. Move Stack to native memory, add collect/reorder/bulk/global-shortcut flows and persist only its
   configurable shortcut preference in migration 25.
5. Implement capability-scoped automation and migration 24 with protocol/CLI/Settings tests.
6. Align version sources and current documentation to `2.0.0-alpha.1`; retain historical evidence.
7. Update privacy, security, data-flow, module-boundary and test inventory documents.
8. Run dependency/license review because version manifests and Cargo.lock change even if the direct
   dependency set does not.
9. Create the candidate commit, rerun final checks with a clean tree, push that exact commit and run
   `scripts/verify-git-sync.sh`. Do not tag, sign, notarize or publish.
