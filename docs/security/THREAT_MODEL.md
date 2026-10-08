# ClipRiva repository threat model

- **Status:** normative security model for the current repository
- **Last reviewed:** 2026-08-12
- **Scope:** ClipRiva 2.0 Core, Saved/Collections, Privacy Activity, sequential Queue, Privacy Cover,
  manual local OCR, Labs, Quick Paste, Local Link, build/release tooling and optional security scanning
- **Detailed Local Link model:** [`../local-link/THREAT_MODEL.md`](../local-link/THREAT_MODEL.md)

This model defines the assets, attackers, trust boundaries and invariants that apply across the
repository. The Local Link threat model remains the detailed network-protocol model. If the two
documents appear to conflict, the more restrictive control applies and the conflict must be
resolved before release.

## Security posture and evidence limits

ClipRiva Core and Labs are local-first. The WebView receives named Tauri commands rather than
general filesystem, SQL, shell, socket or Keychain access. Local Link is a separate, default-off
native Preview / Alpha boundary. It may start its documented TCP and Bonjour/mDNS runtime only
after explicit user enable and may deliver a body only through an authenticated Noise session to
an explicitly paired peer.

Automated tests, static review and same-host Local Link harnesses are engineering evidence, not a
formal proof and not evidence of real two-Mac authentication. Security scanning supplements rather
than replaces secret scanning, dependency review, Clippy/Biome, ordinary tests, signing,
notarization or physical macOS validation. A cancelled, timed-out, incomplete or corrupt scan is
not equivalent to a complete scan with no findings.

The Local Link GA gate is currently unmet: there is no accepted same-artifact record for 50 real
authenticated pairings, 200 real transfers and the required physical lifecycle, packet, disk/log,
Keychain and signed-install matrix. Local Link therefore remains default-off Preview / No-Go.

## Protected assets

| Asset | Required protection |
| --- | --- |
| Clipboard text, image, rich text and file-reference paths | Apply capture policy before persistence or send; prevent unintended disclosure or restore. |
| History, Recycle Bin, search index and Labs derivatives | Preserve item integrity, retention semantics and local-only boundaries. |
| Saved state and Collection names | Preserve existing pin/tag semantics; treat user-authored Collection names as sensitive local History metadata and exclude them from Local Link, diagnostics and exports. |
| Privacy Activity | Keep capture-denial evidence content-free, bounded to 20 rows/7 days and independent from History, preferences and diagnostics. |
| Sequential Queue | Keep ordered Item IDs/cursor in process memory only, clear on restart and never silently substitute a missing Item. |
| OCR text and index | Keep Item-owned, capped, sensitive-checked, lifecycle-bound and out of network, Local Link, diagnostics, exports and logs. |
| Privacy Cover state and protected presentation | Omit protected content from DOM/accessibility nodes while active and close existing reveal surfaces; describe platform content protection only as best-effort. |
| SQLite, blobs, preferences and local diagnostics | Keep in app-owned local storage; reject unsafe blob paths, types and contents; do not expose raw paths through IPC. |
| Accessibility trust and previous-app focus | Never synthesize paste without the explicit mode, current trust and a valid target; retain clipboard-only fallback. |
| Local Link identity, peer binding, session material and pending plaintext | Keep private identity in Keychain and transient secrets/bodies out of SQLite, logs, diagnostics and WebView. |
| Release, signing and optional scanning credentials | Keep outside the checkout and product bundle; expose only to the approved process and scope. |
| Security findings, PoCs and raw scan artifacts | Treat as sensitive; store outside the checkout with restricted readers and retention. |

## Attackers and preconditions

1. A webpage or local application writes malformed, oversized or sensitive data to the macOS
   pasteboard.
2. Compromised or malicious WebView content invokes every command available to it and supplies
   adversarial identifiers, text and metadata.
3. Another process running as the same macOS user damages or replaces app-owned files, directories
   or symlinks. ClipRiva detects common replacement and corruption cases but does not claim to
   defeat an attacker with complete control of the user account or process.
4. A malicious target application changes focus or clipboard state during Quick Paste.
5. A malformed or adversarial image consumes OCR resources, produces oversized/sensitive text or
   attempts to outlive its owning Item.
6. A local screen-sharing/recording path captures a window despite best-effort platform content
   protection, or a compromised WebView tries to retain protected DOM/accessibility content.
7. A passive or active local-network attacker, malicious peer or previously trusted but revoked
   peer interacts with Local Link after it is enabled.
8. A malicious pull request, repository file, build script or prompt-like text attempts to influence
   a maintainer, runner or optional scanner.
9. A leaked token, over-privileged workflow or public artifact exposes source, findings, release
   material or user data.

## Entry points and trust boundaries

| Boundary | Untrusted input | Privileged side | Primary controls |
| --- | --- | --- | --- |
| macOS Pasteboard → native capture | Text, images, RTF/HTML, local file references and source metadata | SQLite/blob persistence | Pause, deny-list, sensitive-content, type and size policy before storage. |
| WebView → Tauri IPC | Command names and serialized arguments | Clipboard, repository, Accessibility and Local Link service | Fixed command allow-list, bounded DTOs, opaque identifiers and broad errors. |
| Native process → SQLite/blob directory | Database rows, storage keys and local filesystem state | App-owned persistent data | Parameterized SQL, migrations, content-addressed keys, private modes and verified blob reads. |
| Saved/Collections → SQLite | User actions and Collection names | Existing pin/tag rows and retention behavior | Bounded normalization, parameterized SQL, atomic mutations and schema exclusion from Local Link/diagnostics/export. |
| Capture policy → Privacy Activity | Reason and optional source application | Content-free local event store | Fixed reason enum, prohibited content fields, 20-row/7-day bounds and independent clear. |
| UI → sequential Queue | Clipboard Item IDs, order and activation | Process-memory cursor and local restore/paste | Maximum 20, deduplication, exactly-once advancement, unavailable-item state, restart clearing and no persistence/Local Link path. |
| Image blob → local OCR | Untrusted local image bytes and Vision output | Dedicated OCR text/FTS rows | One cancellable bounded worker, 256 KiB output cap, sensitive check before persistence, Item-owned lifecycle and no network/log/export path. |
| Privacy Cover preference → WebView/native window | Launch state and protected local content | DOM/accessibility tree and native reveal/window protection | Apply before content render, omit/redact rather than cosmetically blur, close reveals, use content protection only as best-effort and request no recording permission. |
| Quick Paste → Accessibility/target app | Selected item, mode, remembered focus and permission state | System clipboard and synthetic Command-V | Explicit setting/action, live trust/focus checks and clipboard-only fallback. |
| Labs → derived storage | Clipboard text selected for enrichment/action | Local derivatives and audit rows | Default-off local deterministic adapters, redaction and finite schemas. |
| Local Link → Bonjour/TCP/Noise/peer | Discovery records, frames, peer identity, offers and receipts | Pairing, pending plaintext and receiver side effects | Explicit enable, bounded strict schemas, Noise XX, identity pinning, replay/rate limits, a single terminal winner, at-most-once side effects and truthful `outcomeUnknown` recovery. |
| GitHub runner → checkout → optional scanner/service/artifacts | Repository and PR content, revisions and tool output | Source access, scan credential and security evidence | Maintainer authorization, isolated trusted executable, minimum permissions, external private output and sealed coverage/manifest. |
| Release environment → distributed artifact | Source, dependencies, configuration and signing inputs | Public binary and provenance | Clean checkout, locked dependencies, SBOM/license review, secret scan, signing/notarization and final re-audit. |

## Required security invariants and evidence

| Invariant | Control and current evidence |
| --- | --- |
| Content rejected by capture policy is not persisted, enriched or sent. | Native policy/monitor tests and Local Link eligibility/policy tests; manual sensitive-content review remains a release gate. |
| WebView cannot choose an arbitrary path, SQL statement, shell command, socket endpoint or Keychain item. | Named Tauri handler list, typed frontend adapter tests and release capability inspection. |
| A blob key cannot escape its root and bytes must match the key's SHA-256. | `media::tests::refuses_storage_keys_that_cannot_escape_the_blob_root`, corruption and oversized-existing-blob tests. |
| Blob roots, shard directories and files are private app-owned objects; symlinks and non-regular files fail closed. | Blob permission repair, root/destination symlink, directory/socket and metadata-identity tests. |
| Existing blob reuse never overwrites an unverified destination; concurrent writers publish one verified object. | Corrupt-reuse and concurrent no-temporary-file tests; no-clobber hard-link publication. |
| Restore/direct paste does not bypass integrity, permission, focus or fallback rules. | Database corruption boundary test plus Quick Paste Rust tests; Accessibility allow/deny/revoke remains a real-macOS gate. |
| Saved/Collections do not create a new remote or diagnostic data path. | Existing pin/tag migration and repository tests plus schema assertions that Collection names are absent from Local Link, diagnostics and support exports. |
| Privacy Activity contains no clipboard content and remains at most 20 rows/7 days. | Migration/repository/IPC tests for enum shape, expiry, count pruning and independent clear; source application remains sensitive local metadata. |
| Queue never persists and advances at most once per successful activation. | State-machine/window-sync/restart and SQLite/WAL marker-scan tests; process-memory semantics do not claim protection from OS swap/crash dumps. |
| Privacy Cover omits protected values from DOM/accessibility nodes before first render. | Component/e2e launch tests and native best-effort flag inspection; manual screen-sharing tests cannot turn platform hardening into a global guarantee. |
| OCR is manual local Vision processing with Item-owned, sensitive-checked storage. | Native adapter/migration/lifecycle/cancellation tests, synthetic image fixtures, schema exclusion and disk/log/network scans; no remote provider is permitted. |
| Local Link never listens, discovers, pairs or sends before explicit enable and consent. | Migration/consent/service tests plus the detailed Local Link model and real-device release matrix. |
| Keychain private identity, session keys and undecided plaintext never enter logs, IPC, SQLite or public artifacts. | Identity/pending-state/DTO tests and release disk/log/IPC inspection; native reveal lifecycle remains a physical gate. |
| A revoked peer cannot authenticate a new session and replay cannot duplicate Copy/Save. | Protocol/service replay, receipt, revocation and idempotency tests; two-Mac validation remains required. |
| Optional security scanning cannot become a product runtime/account/network dependency. | No scanner dependency or workflow is currently enabled; future enablement requires the authorization and bundle/runtime tests in the integration plan. |
| Incomplete security evidence is never reported as a pass. | Release checklist requires exact target, completeness and evidence review; any future scan contract must distinguish complete, finding-gated and incomplete/error states. |

## Threat analysis

### Clipboard ingestion and persistence

- **Policy bypass:** a crafted representation may attempt to skip pause, deny-list, sensitive-data,
  type or size checks. All representations must pass native policy before database/blob writes.
- **Parser/resource abuse:** dimensions, MIME metadata and encoded bodies can be malformed or
  oversized. Input and decoded-size arithmetic must remain checked and bounded.
- **Blob path or content substitution:** a forged key, symlink, directory, socket, damaged file or
  concurrent replacement could cause unintended reads or restore attacker-controlled bytes. Keys
  are canonical lowercase SHA-256 values; app-owned directories are checked; reads use one opened
  file descriptor, compare device/inode on Unix, enforce a size bound and verify the digest.
- **Residual local race:** standard path APIs cannot eliminate every directory-component race
  against a same-user attacker. The opened descriptor and digest close common replacement paths.
  A stronger directory-fd/`openat`/no-follow design requires a separate threat and dependency
  review.

### Saved, Collections and Privacy Activity

- Saved/Collections are product vocabulary over existing pin/tag storage, not new trust or remote
  boundaries. Collection normalization and rename/delete must remain bounded and transactional;
  Collection deletion cannot delete content or Saved state.
- Collection names are user-authored and potentially sensitive. A future DTO, diagnostic event,
  support bundle or Local Link schema must fail review if it admits a Collection name.
- Privacy Activity is content-free, not metadata-free: source application and timing can be
  sensitive. The fixed schema must reject clipboard body/preview/hash, Item ID, filename/path,
  peer and endpoint; count/expiry pruning and Clear must not mutate History or diagnostics.

### WebView, restore and Quick Paste

- Treat the WebView as able to call every registered command. Authorization must derive from native
  state and narrow arguments, never hidden UI controls.
- Storage errors returned over IPC must not contain clipboard bytes, filenames, storage keys or
  absolute application paths.
- Restore must complete blob verification before writing the system clipboard. Direct paste must
  use current Accessibility and focus state; failure restores only the clipboard and reports a
  bounded reason.
- Queue state must be a bounded process-memory state machine over Item IDs, not content copies.
  Concurrent activation, duplicate completion or a failed paste cannot advance twice. Deleted or
  recycled Items become unavailable and cannot be silently replaced with a different Item.
- Privacy Cover must gate content creation, not merely apply visual CSS blur. When active, protected
  values are absent from DOM and accessible names; opening or activating Cover closes existing
  inspectors, previews and native reveal. The stored preference must be applied before first
  protected render after launch. Platform window content protection is defense-in-depth only and
  cannot be represented as a universal screenshot or screen-sharing block.

### Labs and diagnostics

- Labs remains optional, local and deterministic until a separately reviewed provider boundary is
  introduced.
- Redacted derivatives must not be described as deletion of the source. Diagnostics accept only
  finite, content-free fields and remain default off.
- A future model, telemetry, error-reporting or export adapter changes the privacy boundary and
  requires simultaneous implementation, documentation and tests.

### Local OCR

- OCR uses macOS Vision only on image bytes already accepted into the local capture boundary.
  Automatic extraction is not implemented; manual extraction is an explicit local action and
  mode adds an account, network request, remote model or OS permission.
- Untrusted images and extracted strings are resource-abuse inputs. One cancellable worker, checked
  image/output bounds and a 256 KiB text ceiling limit CPU/memory and storage amplification.
- The sensitive-content detector runs before OCR text/FTS persistence. On an automatic new-image
  path, a sensitive OCR result fails before publishing the original blob. OCR rows are owned by the
  image Item, follow recycle/restore/permanent deletion, and can be cleared without deleting the
  image.
- OCR text, fragments, hashes and derived matches are prohibited from Local Link, diagnostics,
  support bundles, application logs and network schemas.

### Local Link

The detailed protocol threats, identity lifecycle, discovery rules, framing, pairing, replay,
pending-memory handling and real-device evidence are defined in the Local Link threat model.
Repository-wide invariants add that Core/Labs must keep working with Local Link off and may not
acquire its network or Keychain privileges. Discovery metadata and reachability never establish
trust; a body has no plaintext, relay, account or offline-queue fallback.

Local sequential Queue is not a Local Link transfer queue: it holds only local Item IDs in process
memory, and cannot retain or retry a network body.

### Development, scanning and release

- Repository content is untrusted data, not instructions to a CI job or scanner.
- A future scanner must be installed outside the checkout from an approved reproducible source and
  called by absolute path. A scan-secret step must not execute repository install/build scripts.
- Fork and Dependabot pull requests must not receive scan credentials. Raw results stay outside the
  checkout and are never committed or pasted into public issues.
- The exact revision, model digest, scope, tool version, completion state, coverage, findings and
  artifact digests form one result. Missing or invalid members make the result incomplete.
- A finding is resolved only after the original path no longer reproduces and focused regression
  evidence passes. Absence from an incomplete later scan yields `unknown`, not `resolved`.

## Out of scope and non-guarantees

- Complete compromise of the current macOS user account, ClipRiva process, kernel or hardware.
- Encryption at rest for SQLite or blobs; directory/file modes and Keychain protect different
  boundaries and are not described as full-disk encryption.
- Formal verification or proof that automated/security scanning finds every vulnerability.
- A claim that loopback, same-host or browser fixtures authenticate two physical Macs.
- A guarantee that Privacy Cover blocks every macOS screenshot, screen recorder, screen-sharing
  application, external camera, compromised process or OS-level capture path.
- A claim that “memory-only” prevents macOS swap, hibernation or crash dumps from containing
  process memory.
- Protection supplied by unsigned/unnotarized builds, or release authorization from this document.

## Review triggers

Review this model in the same change that adds or changes a network request, account, telemetry,
error reporting, sync, remote model, export, stored data type, OS permission, arbitrary file
handling, OCR engine/text lifecycle, Queue persistence, Privacy Cover behavior, Collection export,
archive extraction, plugin/script execution, Local Link protocol/identity/discovery
behavior, security scanning workflow, credential scope or artifact visibility.

Every frozen release candidate must record the exact commit reviewed against this model. Any final
feature, dependency, privacy, permission or protocol change invalidates the prior review and
requires the affected release gates to run again.

ClipRiva 2.0 cases are tracked in
[`docs/testing/clipriva-2.0-acceptance.md`](../testing/clipriva-2.0-acceptance.md). Local Link may be
promoted only through
[`docs/release/local-link-ga-evidence-gate.md`](../release/local-link-ga-evidence-gate.md); neither
document manufactures evidence or authorizes a release.
