# Local Link GA evidence gate

- **Decision status:** **No-Go**
- **Product label:** default-off **Preview**
- **Assessment date:** 2026-08-12
- **Reason:** required same-artifact two-real-Mac, lifecycle, privacy and signed-install evidence is
  not attached or accepted
- **Release authority:** this file records a gate; it does not authorize push, tag, signing,
  notarization, distribution or GA promotion

Local Link cannot be promoted from Preview on repository tests, browser fixtures, loopback,
same-host processes, screenshots, implementation claims or a Git branch alone. Every mandatory row
must pass against the same frozen signed/notarized candidate artifact, with an auditable evidence
locator and reviewer decision. Unknown, incomplete, stale, mixed-build or missing evidence is a
failure of the gate, not a pass.

## Frozen-candidate identity

Do not begin the acceptance run until these fields are recorded in a private evidence manifest.
They are intentionally blank here; no candidate has been approved by this document.

| Field | Required value | Current state |
| --- | --- | --- |
| Git commit | Full immutable commit SHA | Unmet — not recorded |
| Version/build | Bundle version and build number | Unmet — not recorded |
| Application artifact | SHA-256 of the exact signed `.app`/distribution artifact | Unmet — not recorded |
| Signing identity | Redacted identity reference and successful signature verification output | Unmet — not attached |
| Notarization | Successful notarization/stapling verification for the exact artifact | Unmet — not attached |
| SBOM/license review | Exact manifest/lock digests and completed review | Unmet — not attached |
| Test device inventory | Two physical Macs, model/architecture, macOS version and network role | Unmet — not recorded |
| Evidence owner/reviewer | Named maintainer and independent reviewer | Unmet — not recorded |

Changing code, dependencies, entitlements, protocol, privacy/storage behavior, Local Network copy,
signing inputs or the application artifact invalidates all affected evidence. Re-freeze and rerun;
do not mix results from multiple SHAs or builds.

## Mandatory evidence ledger

Counts are minimums, not sampling targets that permit known failures. Sensitive evidence stays in
the approved private location; repository records may contain only redacted summaries, hashes and
locators. Do not attach real clipboard bodies, Collection/OCR text, device names, fingerprints,
Keychain values, IP/port data, raw packet secrets or private filesystem paths.

| Gate | Requirement | Minimum / expected result | Evidence required | Current status |
| --- | --- | --- | --- | --- |
| LL-GA-01 | Authenticated pairing ceremonies | **50/50 successful positive runs** across two real Macs, plus negative SAS mismatch/cancel/timeout cases creating zero trust | Run ledger, both-side timestamps, redacted identity continuity and negative-case results | **Unmet** — 0 accepted external runs attached |
| LL-GA-02 | Real text transfers | **200/200 accepted outcomes** on two real Macs using the frozen artifact | Per-run sender/receiver terminal state, action and non-sensitive timing; no body | **Unmet** — 0 accepted external runs attached |
| LL-GA-03 | Receiver side effects | Within the 200 transfers, at least **50 Copy, 50 Save and 50 Reject**; duplicate/retry/restart injection creates exactly one terminal winner and no duplicate Copy/Save | Pasteboard/History count evidence and both-side reconciled terminal metadata | **Unmet** — not attached |
| LL-GA-04 | Cancel/expiry/disconnect | Before/after payload-boundary cancellation, receiver timeout, disconnect and lost Receipt report truthful finite state or `outcomeUnknown`; no false Delivered/Copied/Saved | Fault-injection ledger and both-side state timeline | **Unmet** — not attached |
| LL-GA-05 | Replay/revocation/trust expiry | Replayed frames and revoked/expired/session-only peers cannot create trust, session or second side effect; 30-day/default and always/session choices match policy | Packet/fault ledger and local trust-store inspection | **Unmet** — not attached |
| LL-GA-06 | Sleep/wake and session lifecycle | Pending body and native reveal clear on sleep, lock and user switch; wake/unlock recovery remains fail-closed and bounded | Physical macOS video/steps plus memory/IPC/disk checks | **Unmet** — not attached |
| LL-GA-07 | Network changes | Wi-Fi switching, network loss/restore, firewall denial, listener failure and AP/client isolation do not bypass trust, open fallback routes or misreport success | Two-Mac network matrix and content-free supervisor states | **Unmet** — not attached |
| LL-GA-08 | Packet boundary | Bonjour contains only random allow-listed metadata; every body path is authenticated Noise; no plaintext, legacy, Internet, relay or unexpected DNS/HTTP route | Packet capture review tied to artifact/run IDs | **Unmet** — not attached |
| LL-GA-09 | Disk/log/IPC boundary | Synthetic body/preview/Collection/OCR/path/device markers are absent from SQLite/WAL/blobs/logs/diagnostics/WebView IPC where prohibited; pending body is memory-only and expires/clears | Before/during/after scans with salted synthetic markers | **Unmet** — not attached |
| LL-GA-10 | Keychain lifecycle | Identity uses `AccessibleWhenUnlockedThisDeviceOnly`, does not sync, survives approved upgrade behavior, fails closed when missing and resets/re-pairs as documented | Signed-build Keychain tests on clean users/machines; no key export committed | **Unmet** — not attached |
| LL-GA-11 | Consent and permission | Fresh install and upgrade keep Local Link off; no listener/browse/publish/permission prompt occurs before explicit enable; disable stops runtime | Process/socket/mDNS observation and macOS Local Network permission steps | **Unmet** — not attached |
| LL-GA-12 | Install/upgrade/remove | Signed/notarized clean-machine install, upgrade from supported versions, app-data preservation and documented identity-removal behavior | Artifact verification and clean-machine runbooks | **Unmet** — not attached |
| LL-GA-13 | Compatibility matrix | Required macOS 13/14/15/26, Apple Silicon/Intel combinations are exercised where supported by release policy; unsupported cells are explicitly dispositioned, never silently omitted | Device/OS matrix with maintainer-approved exclusions | **Unmet** — not attached |
| LL-GA-14 | Accessibility/product truth | Keyboard and VoiceOver can enable/disable, pair, inspect truthful statuses and reject/cancel; Preview/default-off and recovery copy remain accurate | Manual accessibility report and screenshots with synthetic labels | **Unmet** — not attached |
| LL-GA-15 | Performance/capacity | Pair/send/receive remain within approved latency, size, rate and pending-capacity limits without unbounded memory/CPU growth | Measured benchmark on both physical Macs | **Unmet** — not attached |
| LL-GA-16 | Security/privacy review | Protocol v1.5, repository threat model, privacy/data-flow/module boundary, entitlements and final diff are reviewed after the final feature merge | Signed review record tied to frozen SHA/artifact | **Unmet** — not attached |

The numerical zeroes above mean “zero accepted external runs attached to this gate,” not that no
developer has ever exercised the feature. Unreferenced or unverifiable activity cannot be counted.

## Required run distribution

The private run ledger must make the 50/200 totals independently countable and avoid repeating one
happy path only.

1. Pairing runs cover fresh identity, remembered 30-day trust, explicit session-only, explicit
   always-trust, re-pair after expiry/reset and negative mismatch/cancel/timeout ceremonies.
2. The 200 transfers contain at least 50 each of Copy, Save and Reject. Remaining runs cover
   cancellation, expiry, receiver busy/locked, disconnect, dropped Receipt, restart/reconciliation,
   replay and revocation. Negative cases never count as successful Copy/Save/Reject samples.
3. Both Macs alternate sender/receiver roles and include supported OS/architecture/network cells.
4. Every run records exact artifact hash, local run ID, expected/actual finite state and reviewer
   disposition without clipboard body or stable device/network identity.
5. A retry is a new explicit transfer. It cannot erase or relabel a failed/unknown prior result.

## Absolute No-Go conditions

Any single occurrence blocks promotion regardless of aggregate counts:

- Local Link networking starts or the Local Network permission appears before explicit enable.
- Plaintext/legacy transport, cloud/relay/Internet fallback, automatic sync or an offline body queue
  exists.
- Trust is inferred from display name, mDNS, IP/port, reachability or one-sided confirmation.
- Private/session key, undecided body, body preview/digest, endpoint or prohibited user metadata is
  persisted, logged, exported or returned to the WebView.
- A replay, retry, crash or concurrent decision causes duplicate Copy/Save or conflicting terminal
  winners.
- A revoked/expired peer authenticates or receives a new body.
- Cancellation/failure/unknown is presented as Delivered, Copied or Saved.
- A signed artifact differs from the reviewed/tested hash, or signing/notarization/clean-install
  evidence is incomplete.
- Required evidence is missing, unreadable, mixed across builds, contains unredacted user secrets,
  or cannot be independently reconciled to its run ledger.

## Decision procedure

1. Confirm every frozen-candidate field is complete and internally consistent.
2. Reconcile the raw private run ledger to all minimum counts and required matrix cells.
3. Confirm every mandatory row is Pass and every accepted artifact locator is readable by the
   reviewer; `Unknown`, `Partial`, `Not run` and `Waived without owner approval` fail the gate.
4. Confirm zero absolute No-Go findings and close/retest each prior security/privacy finding on the
   same artifact.
5. The release owner records a dated Go/No-Go decision. A Go decision permits only the separately
   approved release action; this document still does not perform a tag, signing, notarization,
   publication or deployment.

## Current decision

**No-Go.** The frozen candidate identity is blank and every external evidence row is unmet. Keep
Local Link default off and labelled Preview. Repository automation may improve engineering
confidence, but it cannot change this decision without the required physical and signed-artifact
evidence.
