# ClipRiva privacy and data handling

- **Effective status:** local-first ClipRiva `2.0.0-alpha.1` engineering candidate, default-off Labs
  and local automation, manual-only OCR, and default-off Local Link Preview
- **Project owner and contact:** Sealdot404 — `sealdot404@gmail.com`
- **Local Link release status:** Preview / No-Go; required real-device and signed-artifact evidence
  has not been completed

ClipRiva Core keeps clipboard History on the Mac where it is captured. Core and Labs have no
account, telemetry service, analytics endpoint, sync service, cloud-model provider or
application-controlled Internet service.

Local Link is a separate, default-off device-to-device experiment. Only when the user explicitly
enables Local Link does its native service start a private TCP listener and Bonjour/mDNS discovery
runtime. The intended content-bearing destination is only a clearly paired Mac on the same local
network; mDNS is local multicast discovery and carries no clipboard body. Local Link has no account
directory, cloud relay, Internet fallback, offline queue, automatic History synchronization or
plaintext/legacy-protocol fallback. It remains Preview / No-Go until the real-device gate below
passes.

This document defines the permitted privacy boundary for the ClipRiva 2.0 working candidate. It
does not by itself prove that every planned 2.0 capability is implemented or release-ready. The detailed flow inventory is
in [docs/privacy/data-flow.md](docs/privacy/data-flow.md); protocol and threat-model constraints are
in [docs/local-link/PROTOCOL-v1.5.md](docs/local-link/PROTOCOL-v1.5.md) and
[docs/local-link/THREAT_MODEL.md](docs/local-link/THREAT_MODEL.md).

## Data ClipRiva stores or handles locally

| Data category | Examples | Why it is stored |
| --- | --- | --- |
| Clipboard content | Text, PNG images, RTF/HTML, and local file-reference paths | Search, display, restore, and History management. Referenced file contents are not imported. |
| Item, Occurrence and Variant metadata | Stable local identifiers, source application, capture/use time, representations, Saved state and retention/recycle state | Local History ordering, recovery and reliable local behaviour. “Saved” productizes the existing local pin/retention-protection fact; it does not create a cloud destination. |
| Saved and Collections | Existing `is_pinned` state plus up to eight user-authored Collection names of at most 24 visible characters per Item | Local organization, filtering, reuse and retention protection. Collections productize the existing Item tag rows rather than adding an account or sync model. For schema compatibility these remain bounded user tags; tags never enter Local Link or diagnostics. Collection names can contain sensitive wording: they stay in local SQLite, follow the Item through Recycle Bin restore and are deleted with the Item, and never enter support bundles or automatic export. |
| Smart Collections | At most 20 user-named, allow-listed local filter definitions | Reopen a dynamic view without copying or extending the lifetime of matching Items. Definitions contain finite kind/source/Saved/time/use/Local-Link-source fields only; they contain no clipboard body, free-text search, SQL, regex, script or nesting. Names and rules remain local and are deleted explicitly by the user. |
| Item notes | One optional UTF-8 note of at most 16 KiB per Item | Add searchable local context to a specific clip. Notes are sensitive History content in dedicated SQLite/FTS storage. Blank save deletes the note; recycle/restore follows the owner, and permanent Item deletion cascade-deletes it. Notes do not make an Item Saved or extend retention and never enter ordinary Item lists, Local Link, diagnostics, support export, logs or automation metadata-only results. |
| Privacy Activity | A finite reason, optional source-application label and occurrence/expiry time for a clipboard change that was not captured | Explain local privacy decisions without retaining the clipboard value. It is content-free, stores at most 20 recent rows for at most 7 days, and excludes body, preview, hash, Item ID, filename/path, peer and endpoint. Clearing it does not change History, preferences, diagnostics or Local Link. |
| Preferences and workspace layout | Pause, deny-list, retention, Quick Paste, Labs/diagnostics and Privacy Cover settings, plus the main-window size | Apply user-selected local behaviour and safe workspace geometry across launches. New installs default unpinned History to 30 days; an existing user-selected retention value is retained. OCR is manual-only and has no automatic preference. Window geometry contains no clipboard or account data. |
| Sequential Stack | At most 20 ordered local clipboard Item IDs, an in-memory cursor and collect-mode flag | Support explicit collection, reorder and sequential restore/paste actions. Stack is process memory only, contains no clipboard body, clears on application restart and is forbidden from SQLite/WAL, files, logs, diagnostics, exports and Local Link. An unavailable/deleted ID is reported rather than silently replaced. |
| Filter Composer definitions and audit | A named allow-listed pipeline of 1–8 finite text steps; content-free outcome metadata after confirmed copy | Preview and confirm deterministic local transformations. The result preview stays in process/webview memory, is capped at 256 KiB and writes neither clipboard nor audit. The user explicitly confirms the displayed Before/After result; confirmation rechecks the Item version, original content, rule and output before one clipboard write, then stores only finite action/outcome metadata. A canceled or stale preview makes no write. No input, output, preview or content digest is audited. Arbitrary shell, regex, file and network actions are prohibited. |
| Local automation preferences | Default-off enabled state and separately granted finite capabilities | Let the bundled CLI or an Apple Shortcuts “Run Shell Script” action request a narrow operation from the running app. Requests use an owner-only local Unix socket and bounded JSON; there is no listener on the network. Content access and clipboard/library writes require separate grants. No arbitrary SQL, filesystem, shell, delete/clear, Local Link, Stack, bulk image/blob, diagnostics export or remote-Agent capability is provided. |
| Local OCR text | Capped text extracted by macOS Vision from image bytes ClipRiva already stores | Explicit manual local extraction. OCR text is a dedicated Item-owned SQLite/FTS data type; it follows the image Item through recycle, restore and permanent deletion. Sensitive output is rejected before persistence. OCR text never enters Local Link, diagnostics, support bundles, application logs or a network request. Automatic extraction is not implemented. |
| Optional Labs derivatives and diagnostics | Deterministic local classification/search derivatives; allow-listed event/result pairs, UTC day and app version | Power optional local features. Diagnostics exclude clipboard body, hashes, IDs, source app, paths, document names, accounts, endpoint, device name and key material. |
| Local Link consent | Enabled state and local display name | Preserve the user’s explicit network choice. New installs and upgrades default off; opening the app or Settings is not consent. |
| Local Link identity | Versioned long-lived Noise private/public identity blob and derived public fingerprint | Authenticate the local Mac. The private key is in macOS Keychain under `com.clipriva.desktop.local-link.identity` / `noise-static-v1`, protected as `AccessibleWhenUnlockedThisDeviceOnly` without Keychain sync. It is never stored in SQLite or returned to the WebView. |
| Local Link trust metadata | Opaque device ID, local display label, peer public identity/fingerprint, selected trust duration, trust state/times, compatible protocol range and last-seen time | Bind a peer only after authenticated, dual-confirmed pairing. The default is 30 days; “always” is an explicit local choice. “This session” keeps its usable peer key only in native memory. The 30-day expiry and re-pairing reason are derived from metadata; network endpoints are not stored. |
| Local Link transfer metadata | Opaque transfer ID, peer reference, direction, fixed `text` kind, byte size, timestamps/expiry, finite state (including content-free `viewed` and `reconciling`), receiver action, finite failure class including `outcomeUnknown`, and derived recovery action | Explain a recent attempt without creating a second clipboard History. `viewed` records only that the receiver opened its Incoming Center. `reconciling` means the complete payload crossed the sender’s durable no-replay boundary but its terminal receipt was not yet observed; `outcomeUnknown` is used when metadata-only reconciliation cannot prove a result. User-facing lists return at most 20. Active/reconciling/prepared recovery evidence is retained until it resolves; terminal summaries have a 24-hour ceiling and recent terminal evidence is protected from count pruning for 60 seconds so a lost Receipt can still be reconciled. |
| Local Link Copy effect claim | Opaque transfer ID, fixed `copy` action, `prepared`/`completed`/`uncertain` state, random operation marker, pasteboard change count and timestamps | Prevent a crash or duplicate command from writing the system clipboard twice. The claim contains no body, preview, digest, peer, endpoint or History Item reference and is cascade-deleted with its bounded transfer summary. |
| Local Link replay/rate state | Peer reference, opaque transfer/session ID, nonce, expiry and bounded counters | Reject replay and request flooding before retaining a body. Replay tombstones last at most 24 hours; rate counters are process memory. |

Local Link also provides two read-only support projections. Opening Devices may read a finite
readiness snapshot (enabled runtime, discovery, trust and recently authenticated presence); this
read does not start networking or request permission when Local Link is off. An explicit **Preview
diagnostics** action builds an ephemeral JSON bundle with app/OS architecture, that readiness
snapshot, per-export salted peer references and at most 50 allow-listed peer/transfer metadata
records. It is returned only to the requesting WebView for review and optional user download. It is
not stored by ClipRiva and is never uploaded automatically.

Pause, excluded-app and sensitive-content policy are evaluated before any Item, Occurrence, Variant
or related local representation is persisted. The same policy is rechecked before a Local Link body
enters native transport.

For the small text-family model, ClipRiva classifies and validates the selected local text before
it is persisted: text/code/command are capped at 256 KiB, HTTP(S) links at 8 KiB, and colour values at 1
KiB. An unsupported or oversized value creates only a short-lived, content-free local status
reason; its body is not retained. Plain-text whitespace normalization and consecutive duplicate
collapse happen locally before History storage.

Transfers, diagnostics and application logs must not store Local Link body, preview, ciphertext,
payload digest, source-app name, file path/name, account identifier, IP address, port, Bonjour
instance, endpoint, nonce, session key or private key. Pairing code and ceremony state are temporary
and are cleared on completion, cancellation or the 60-second timeout.

The Local Link support bundle additionally excludes display names, stable device/transfer IDs,
fingerprints, clipboard Item IDs, byte counts and raw system errors. Its peer references are salted
anew for each export and are not reversible identifiers maintained by ClipRiva.

## Storage, retention and memory boundary

Most Core data is stored locally in the application-support directory as SQLite (`clipriva.sqlite3`)
and, where necessary, local content-addressed blobs. Retention, pinning and the local Recycle Bin
control its lifetime. SQLite/blobs are not documented as encrypted at rest.

New installations retain unpinned History for 30 days by default. Expired unpinned records move to
the local Recycle Bin when the database opens, captures, lists or searches; pinned records are
exempt until explicitly unpinned or removed. A changed retention preference is applied immediately
to unpinned active records. Existing user-selected retention values are not silently replaced.

The UI calls the existing pin fact **Saved** and the existing Item tags **Collections**. This is a
product-language change, not a migration to remote storage. Collection names have no independent
retention period: they remain attached to their Item while it is in History or the Recycle Bin and
are cascade-deleted when that Item is permanently deleted. A Collection name may contain
user-sensitive wording, so ClipRiva treats it as local History metadata and does not include it in
Local Link, local diagnostics, support bundles or any automatic export. Removing Saved protection
returns the Item to normal retention but does not silently remove its Collection memberships;
deleting a Collection removes memberships only and does not delete clipboard content.

Privacy Activity reuses the content-free `uncaptured_clipboard_events` boundary. Expired rows are
removed locally, and the visible/retrievable set is bounded to 20 rows and 7 days. It is separate
from the optional diagnostics store. Source-application metadata can itself be sensitive even
though the event has no clipboard content, so it remains local and is excluded from diagnostics,
Local Link and exports.

The sequential Stack intentionally has no durable lifetime. Only ordered Item IDs, a cursor,
collect-mode state and an in-flight token may exist in ClipRiva process memory; application restart
clears them. “Memory-only” is an intentional serialization boundary, not a guarantee about macOS
swap, hibernation or crash dumps. Stack never means a Local Link delivery or offline payload queue.

Notes are a new sensitive stored data type. They are independently opened through the Item
Inspector and matched by the dedicated local full-text index; ordinary History/list DTOs do not
carry the note body. A note never changes Saved/retention state. Smart Collection definitions are
local organization metadata, not snapshots of matching Items, and deleting a definition never
deletes History.

Extracted OCR text is a new stored data type. It lives in a dedicated Item-owned SQLite table and
local full-text index, is capped at 256 KiB, and follows the owning image
through recycle, restore and permanent deletion. Deleting all OCR derivatives leaves the original
images and other History intact. Extraction is an explicit manual Inspector action; no automatic
capture path is implemented. Sensitive-content policy is evaluated before OCR persistence. The
implementation uses only local macOS Vision for this boundary and may not send the image or
extracted text to a model/provider.
Quick Paste can now find an image by its manually extracted OCR text; the compact search result
contains only match provenance and the ordinary Item fields, never the OCR body. A Note match has
the same body-omission rule.

On Unix/macOS, ClipRiva creates or tightens its app-data and blob directories to owner-only `0700`;
new and reused blob files are `0600`. Existing app-owned blob permissions are repaired when their
directories/files are opened. Blob restore and reuse reject symlinks and non-regular files, enforce
the automatic size limit and require the bytes to match the SHA-256 storage key. A failed check
returns no blob bytes and does not expose the absolute app-data path through the application error.
These controls reduce accidental disclosure and detect common same-user file replacement or
corruption; they are not encryption and do not claim protection after full compromise of the user
account or ClipRiva process. SQLite file-mode migration is not part of this change; its enclosing
app-data directory supplies the owner-only directory boundary.

Local Link adds a separate native boundary:

- Its long-lived identity is kept only in macOS Keychain. Removing application-support data or the
  application bundle may not remove it. Explicit Local Link identity reset keeps Local Link off,
  clears active/pending state, deletes the Keychain item and marks peer bindings
  `needsRePairing`; it does not delete local History.
- Session keys and discovered endpoints are process/OS-networking memory only. They are cleared on
  close, disable, revoke, reset and service exit and are forbidden from SQLite, Transfers,
  diagnostics and the WebView.
- An authenticated but undecided received body remains in native process memory for at most 60
  seconds. An autonomous deadline worker clears it even if the Transfers UI is closed. It must not
  be serialized to SQLite/WAL, files, blobs, logs, diagnostics or the WebView.
- Secure reveal receives only an opaque transfer ID over IPC and returns no text. A short-lived
  zeroizing native copy may be rendered by the native macOS surface; reveal neither extends the
  deadline nor creates a receipt. Sleep, session deactivation, explicit disable and termination
  synchronously close the surface and clear its informative text; the normal 60-second deadline
  also closes it.
- A Copy decision first stores a content-free effect claim, then writes text plus its random private
  marker to the macOS pasteboard, and finally commits the claim and terminal summary together. On
  restart, only a matching marker can prove completion. A missing/mismatched marker becomes
  `outcomeUnknown`; ClipRiva never repeats the clipboard write. The marker is not a body digest and
  cannot be used to reconstruct clipboard content.
- A verified Local Link trust binding defaults to 30 days; after the same dual confirmation, a
  person may instead select “this session” or explicit “always trust.” A session-only usable peer
  key exists only in native memory and disappears on disable or app exit. At 30-day expiry, native
  code clears the usable peer key/fingerprint and returns only `needsRePairing` plus a
  content-free reason; discovery, send, receive and receiver acceptance all fail closed until a
  new dual-confirmed pairing succeeds.

“Memory-only” describes ClipRiva’s intentional serialization boundary. General OS swap,
hibernation and crash-dump behaviour are platform boundaries, not guarantees made by this document.
The v1.5 native lifecycle adapter observes sleep/wake, session active/inactive and termination and
clears pending bodies before stopping transport. Network.framework path changes feed a
generation-scoped supervisor with 500 ms debounce and bounded restart delays of 250 ms, 1 s, 2 s
and 5 s. These implementation paths and reducer tests do not replace real-macOS lock/user-switch,
sleep/wake, network-switch or already-visible reveal acceptance evidence.

## Privacy Cover boundary

Privacy Cover is an explicit local presentation mode. When active, protected clipboard text, OCR
text, Collection names, source labels, filenames and images must not be created in WebView DOM
nodes or accessible names, and open inspectors/previews plus the native Local Link reveal surface
must close. The preference may be stored locally so the native/boot path can apply it before any
content surface renders after launch.

The desktop may also request the platform's window content-protection flag as best-effort
hardening. This does not promise protection from every screenshot, camera, screen-sharing or
screen-recording path, and it is not a global screen-recording detector/blocker. ClipRiva does not
scan other processes, detect meeting applications, or request Screen Recording, Camera or Photos
permission for Privacy Cover.

## Runtime network boundary

Core, Saved/Collections, Smart Collections/notes, Privacy Activity, Stack, Filter Composer, local
automation, Privacy Cover, Labs, local diagnostics and any permitted macOS Vision OCR path do not
send clipboard content, metadata or preferences to a ClipRiva-controlled service and do not require
an account. Stack events, note text and OCR text are not Local Link payloads. Local Link continues
to accept only the explicit, currently selected,
policy-approved text Item through its existing send boundary.

Local automation is inter-process communication on the same Mac, not a network service. It stays
off until the user enables it and grants individual capabilities. The owner-only socket accepts
only bounded, versioned operations from the bundled command-line client while ClipRiva is running;
content-returning and mutating requests fail closed without their specific grant. Apple Shortcuts
support is the same CLI boundary invoked by a user-authored shell action, not a native App Intents
integration and not permission to expose History to an Agent.

When Local Link is disabled, there is no Local Link listener, mDNS publish/browse runtime, peer
session or remote delivery. When the user explicitly enables it, native code starts the listener and
Bonjour/mDNS runtime. Discovery advertises random `ll-<UUID>` instance/host metadata and only
allow-listed version/pairing fields; it does not advertise the user’s device name, an endpoint for
persistence, a key, or clipboard content. Discovered endpoint values remain native-memory routing
facts.

Only an authenticated, explicitly paired peer static identity may receive a selected, policy-approved
UTF-8 text Item. TCP frames use the reviewed
`Noise_XX_25519_ChaChaPoly_BLAKE2s` protocol. Before a body is written to that transport, ClipRiva
rechecks pause, excluded-app and sensitive-content policy plus type, UTF-8 and 256 KiB limits. There
is no plaintext codec or fallback, cloud/relay route, account lookup, automatic sync or offline
delivery.

Protocol generation 3 uses authenticated, content-free `StatusQuery`/`StatusResponse` frames after
a terminal receipt may have been lost. Only the already-known transfer ID and `unknown`, `pending`
or finite terminal metadata are exchanged. The selected text is never retained for this worker and
is never replayed. If bounded reconciliation cannot prove an outcome, the sender records
`outcomeUnknown` instead of claiming cancellation, failure-before-delivery or success.

## Local Link receiver actions

- **Copy:** writes only the macOS clipboard once, adds a private random effect marker for crash
  reconciliation, and suppresses ClipRiva self-capture; it does not create History.
- **Save:** writes only local History, with a Local Link source label, and does not modify the
  clipboard.
- **Incoming Center open / viewed:** records only that a pending request was opened. It neither
  reads or writes the system clipboard nor creates History and it never exposes the body to the
  WebView.
- **Reject / Escape:** clears pending application memory and creates neither clipboard nor History
  content.
- **Cancel / expiry / disable / revoke / disconnect / exit:** clear pending memory and retain only a
  finite, content-free terminal state when applicable. If the remote or native side-effect commit
  boundary was already crossed, ClipRiva records/reconciles uncertainty instead of asserting a
  false cancellation or failed delivery.

A transfer ID is processed at most once. Duplicate/replayed packets cannot create a second Copy or
Save. Retrying is a new explicit send from the currently selected Item with fresh
session/transfer/nonce material; Transfers is never a payload queue.

## Permissions and controls

- Clipboard capture and restore use macOS clipboard facilities.
- The native menu-bar panel is an additional local display/control surface for the latest five
  History entries and capture pause state. Selecting an entry uses the same local clipboard restore
  path as the main window; it does not create a network route, account, telemetry event or OS
  permission request.
- The first user-invoked direct-paste attempt is the Accessibility boundary. Browsing History or
  opening Settings does not request it; without trust or a confirmed target, ClipRiva restores the
  clipboard and does not synthesize a paste keystroke.
- Privacy Cover does not request Screen Recording, Camera or Photos access. Platform content
  protection is best-effort window hardening, not a global recording guarantee.
- Local OCR uses macOS Vision on already-local image bytes and introduces no new OS permission,
  account, remote model or network request. Extraction is manual-only.
- Local Link is disabled by default. Only its explicit enable control may start native networking;
  disabling it stops the native transport and clears active Local Link state without deleting local
  History or disabling normal clipboard capture. The macOS bundle declares the Local Network purpose
  and `_clipriva._tcp` Bonjour service; the system may ask for that access only when Local Link is
  explicitly enabled, never while browsing the app.
- Add device may expose a temporary 60-second pairing ceremony. Device name, mDNS record, IP/port
  or endpoint are not trust anchors: trust requires the authenticated peer identity and both users’
  confirmation.
- Users can disable Local Link, stop pairing, reject/cancel a request, revoke a trusted peer, clear
  terminal content-free summaries and reset the Local Link identity. Keychain failure remains
  fail-closed.
- Any future OS permission or capability—such as notification—must be disclosed, implemented and
  tested before it is described as available. No denied permission authorizes a cloud or plaintext
  fallback.

## Security reports and support

For a suspected vulnerability or unintended disclosure, do not include clipboard contents, Keychain
values, public/private identity material, fingerprints, device names, IP/port data or raw packet
captures in a public issue. Follow [SECURITY.md](SECURITY.md) and contact
`sealdot404@gmail.com` privately.

## Release gate

Local Link remains **Preview / No-Go**. As of this document update, the required external evidence
has not been supplied: at least 50 explicit authenticated pairing ceremonies and 200 text
transfers on two real Macs, Copy/Save/Reject samples, cancellation/expiry/revocation/replay,
lock/user-switch, AP isolation, Wi-Fi switching, sleep/wake, performance, packet inspection,
SQLite/WAL/blob/log scans, Keychain lifecycle and signed/notarized clean-machine installation.
These gates must be run against one frozen candidate artifact and recorded in
[docs/release/local-link-ga-evidence-gate.md](docs/release/local-link-ga-evidence-gate.md).

Automated tests, browser fixtures, same-process loopback and same-host two-process harnesses do not
prove two-Mac network delivery or encryption. Plaintext fallback, incorrect trust, pending-body
persistence, duplicate Copy/Save, a post-revocation session, or a failed attempt reported as
delivered is an absolute No-Go.

Passing repository tests or publishing a Git branch does not satisfy this gate and does not
authorize a tag, signing, notarization, distribution or promotion of Local Link to GA.

## Changes to this document

Review this document and the corresponding data-flow/module-boundary documents whenever a change
introduces or changes a network path, discovery field, account, telemetry, sync, remote model,
export, stored data type, key lifecycle, deletion/retention rule or OS permission. Implementation,
tests and physical evidence must support every “local”, “encrypted”, “memory-only” and “no cloud”
claim.
