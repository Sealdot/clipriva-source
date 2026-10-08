# ClipRiva v0.8 implementation and acceptance plan

## Outcome and release posture

v0.8 turns the Local Link v0.7 control-plane preview into a reviewable two-Mac text-transfer
candidate. The only supported payload is one explicitly selected UTF-8 text item between two
trusted Macs on the same LAN. There is no account, cloud relay, offline queue, automatic clipboard
sync, image transfer, file transfer, broadcast, or cross-platform client.

The implementation remains labelled **Local Link Preview** until every automated gate and the
physical two-Mac matrix passes. A browser fixture, in-process transport, loopback TCP test, build,
or screenshot is useful engineering evidence but is never evidence that a real Mac received an
authenticated encrypted transfer.

## Findings carried forward from the v0.7 audit

The v0.7 implementation already has four boundaries worth preserving:

1. Local Link is isolated from History, Quick Paste, capture policy, Recycle Bin, and Labs.
2. Pending inbound plaintext is process-memory only and never crosses the WebView command boundary.
3. Copy writes only the system clipboard, Save writes only History, and Reject writes neither.
4. Transfers are content-free metadata, bounded to 20 records and 24 hours.

The v0.8 work must close these P0 gaps:

- production transport is currently unavailable;
- pairing can be confirmed locally without a remote identity or remote confirmation;
- there is no Keychain identity, Bonjour discovery, authenticated encryption, protocol version,
  replay defense, nonce, expiry, acknowledgement, cancel, receiver limit, or rate limit;
- current receive and revoke paths can race with a side effect;
- Devices, Send, Incoming, Details, and Transfers can compete for focus;
- send and transfer states cannot express Connecting, Encrypted, Awaiting, Cancelled, or Expired.

## Product and technical decisions

| Area | v0.8 decision | Non-goal / fail-closed rule |
| --- | --- | --- |
| Consent | New installs and upgrades keep real transport off. A v0.7 preview toggle is not migrated into listener consent. | No background listener before a new explicit enable action. |
| Discovery | Bonjour/mDNS publishes an anonymous rotating instance during enabled operation. The display name, pairing nonce, and comparison material are exposed only in a user-started 60-second pairing window. Endpoints remain memory-only. | No directory service, persistent IP/port, Internet discovery, or cloud relay. |
| Identity | Each Mac owns a long-lived static identity secret in macOS Keychain. SQLite stores only peer public identity/fingerprint and trust metadata. | Missing, locked, corrupt, or denied Keychain access fails closed. No temporary unpinned identity fallback. |
| Pairing | A bounded authenticated handshake binds both identities and the handshake transcript. Both screens show the same six-digit SAS and short fingerprints. Trust is persisted only after both sides confirm. | A display name, IP address, six-digit value, or one-sided click never creates trust. |
| Transport | A reviewed Noise pattern provides mutual authentication and encryption over bounded TCP frames. Trusted transfers pin the peer identity; there is no plaintext or unverified fallback. | No custom cipher construction, 0-RTT payload, old protocol downgrade, or arbitrary WebView socket API. |
| Offer | The sender first transmits content-free protocol/version/size/TTL metadata. The receiver checks lock state, trust, version, rate, and capacity before accepting the encrypted body. | Busy, locked, incompatible, rate-limited, or untrusted receivers do not accept the text body. |
| Payload | One non-empty UTF-8 value, 1 byte through 256 KiB. Text, code, URL, and color Items are normalized to `text`. | Rich text, images, file references, paths, batches, and history sync are blocked. |
| Pending body | The receiver holds plaintext only in process memory for at most 60 seconds. | No serialization, crash recovery, offline queue, preview, log field, diagnostic field, or transfer-summary payload. |
| Idempotency | Session ID, transfer ID, nonce, timestamp, TTL, bounded replay tombstone, and atomic decision claim make each transfer single-use. | Duplicate or late frames cannot Copy or Save twice. |
| Receiver pressure | One pending request per peer, three globally, and five offers per peer per minute. | Excess offers are rejected before the text body is accepted. |
| Retry | A failed request is terminal. The user returns to the still-selected local Item and starts a new transfer with new IDs and nonce. | Transfers never replays or retries an old payload. |
| Revocation / disable | Revocation, disable, timeout, sender cancel, lock, disconnect, and exit share one atomic cancellation path that clears memory and closes sessions. | A cloned body cannot survive revocation and later write clipboard or History. |

## Delivery plan

### M1 — Protocol and trust boundary

- Publish the versioned frame schema, handshake pattern, SAS derivation, identity storage,
  acknowledgement semantics, replay behavior, failure classes, and threat model.
- Add migration behavior that turns every legacy preview trust record without a valid pinned key
  into `needsRePairing` and leaves real transport disabled after upgrade.
- Add an injectable identity store for unit tests and a macOS Keychain implementation for runtime.

Exit: protocol and threat-model documents match the code; migration, identity, and state-machine
unit tests pass; UI still makes no LAN-delivery claim.

### M2 — Two-process alpha

- Add listener lifecycle, Bonjour browse/publish, in-memory endpoint cache, bounded TCP framing,
  authenticated encrypted sessions, peer pin verification, offer/body/receipt exchange, cancel,
  timeout, dedupe, rate limit, and capacity checks.
- Keep pending plaintext exclusively in the receiver runtime state.
- Emit UI-safe metadata updates through the existing named Tauri boundary.

Exit: two independent processes with isolated databases and identities complete pairing and
Copy/Save/Reject/Cancel/Expire over real TCP loopback. This is protocol integration evidence, not a
physical two-Mac Go result.

### M3 — Product flow and accessibility

- Devices shows feature state, this Mac's fingerprint and protocol, pairing window, same SAS,
  local/remote fingerprint, both confirmations, Online/Offline, last seen, trusted since, and a
  fingerprint-aware revoke confirmation.
- Send selects one online trusted peer and presents Ready → Connecting → Encrypted → Awaiting →
  Final with countdown and Cancel. The body remains hidden and a secondary action returns to the
  selected Item.
- Incoming shows device, text type, byte size, and expiry only. Copy, Save, Reject, Escape, sender
  cancel, and timeout are mutually exclusive.
- Transfers supports All, Incoming, Outgoing, and Failed filters and contains no body, preview,
  source app, path, endpoint, or replay button.
- One coordinator enforces modal priority: a single system/pair/send surface, incoming priority,
  non-blocking Transfers behavior, details auto-close, focus restoration, and stable keyboard order.

Exit: component and application tests pass; the four reference states pass same-viewport visual
comparison with no P0/P1/P2 differences.

### M4 — Beta candidate

- Run static, build, privacy-boundary, migration, protocol, fuzz/bounds, clean-install, upgrade,
  packaging, and license/SBOM checks.
- Run the physical two-Mac matrix and record timing, packet-capture, Keychain, SQLite, WAL, log,
  clipboard, History, lock-screen, sleep/wake, AP isolation, Wi-Fi switch, and revocation evidence.
- Only after every gate passes may product copy change from Preview to Beta.

Exit: Go/No-Go report records reproducible evidence and zero absolute No-Go events.

## Execution result — 2026-07-28

| Milestone | Result | Evidence / remaining gate |
| --- | --- | --- |
| M1 — Protocol and trust boundary | Completed for Preview candidate | Migration 15, macOS Keychain identity, Noise XX/framing, replay/rate/capacity state machine, protocol and threat-model tests are present. |
| M2 — Two-process alpha | Not activated | Discovery metadata and encrypted framing compile and pass unit/same-process loopback tests, but production starts no listener, Bonjour runtime, native pairing, peer session or remote delivery. |
| M3 — Product flow and accessibility | Completed for Preview UI | Devices, pairing comparison, Send, Incoming and Transfers states are implemented; browser fixtures are visibly labelled and cannot claim delivery. Automated UI, keyboard and surface-coordination checks pass. |
| M4 — Beta candidate | No-Go | Build, static boundary and license/SBOM checks pass. Two independent processes, signed/packaged behavior and the two-real-Mac 50-pairing/200-transfer matrix were not available in this workspace. |

The accepted engineering result is therefore **v0.8 Preview/Alpha groundwork**, not Local Link Text
Beta. Git may record this reviewable checkpoint; release copy must remain Preview until M2 and M4
are completed with physical evidence.

## Automated test cases

| ID | Layer | Scenario | Required assertion |
| --- | --- | --- | --- |
| V08-01 | Migration | v14 preview toggle/trusted rows upgrade | Real transport remains off; empty/unverifiable peer identity becomes `needsRePairing`; migration is atomic. |
| V08-02 | Identity | first run, restart, concurrent access, missing/corrupt/denied Keychain entry, explicit reset | Identity is stable after restart; secret never reaches SQLite/IPC/logs; every unavailable state fails closed. |
| V08-03 | Pairing | normal, one-sided confirm, code mismatch, fingerprint mismatch, cancel, duplicate, timeout | Trust exists only after both sides confirm the same transcript-bound SAS and identities. |
| V08-04 | Discovery | off, enabled, pairing window, timeout, exit, sleep/wake | Off binds/publishes nothing; names/comparison material are visible only for 60 seconds; endpoint is never persisted. |
| V08-05 | Framing | truncated/oversized frame, invalid UTF-8, length lie, unknown field/version, stale/future timestamp | Parser rejects before allocation or state change; no input content is logged. |
| V08-06 | Session | correct peer, unknown peer, replaced key, tampered handshake, downgrade attempt | Correct pinned peer succeeds; every other case returns content-free auth/version failure with no plaintext fallback. |
| V08-07 | Eligibility | disabled, 0 B, 1 B, 256 KiB, 256 KiB+1, non-text, privacy block, offline, incompatible | Only supported in-range text to an online compatible trusted peer constructs a payload. |
| V08-08 | Offer pressure | peer already pending, three global pending, sixth offer/minute, locked receiver | Rejected before body acceptance with receiverBusy/rateLimited/receiverLocked; memory limits remain bounded. |
| V08-09 | State machine | Connecting → Encrypted → Awaiting → final, illegal/late/duplicate messages | Only legal transitions succeed and one final state is immutable. |
| V08-10 | Receiver choice | 100 concurrent Copy/Save/Reject calls | Exactly one caller claims the body and at most one side effect occurs. |
| V08-11 | Lifecycle race | accept races revoke/disable/timeout/cancel/exit | One atomic winner; any terminal cancellation prevents all later clipboard/History writes and clears plaintext. |
| V08-12 | Copy | explicit Copy | System clipboard receives exact text; History is unchanged; self-capture suppression is consumed once. |
| V08-13 | Save | explicit Save | History receives exact text with a Local Link peer source; system clipboard is unchanged. |
| V08-14 | Reject | explicit Reject/Escape | Clipboard and History are unchanged; pending memory is cleared; sender sees Rejected. |
| V08-15 | Cancel/expire | sender cancel, 60-second timeout, late acknowledgement | Both runtimes converge on Cancelled/Expired; pending body is cleared; late frames cannot revive it. |
| V08-16 | Replay | same transfer/nonce 20 times, reconnect, restart | No second body acceptance or Copy/Save; replay metadata remains bounded and content-free. |
| V08-17 | Retention | no Transfers UI opened for 25 hours / more than 20 summaries | Background/startup cleanup enforces both 20-row and 24-hour bounds. |
| V08-18 | Metadata | sentinel body, path, endpoint, secret, session material | SQLite/WAL/IPC/Transfers/log/diagnostic export contain none of the prohibited values. |
| V08-19 | Devices UI | default off, pairing progress, online/offline, revoke confirm | Status is textual and accessible; fingerprint is visible before trust/revoke; off still permits revocation. |
| V08-20 | Send UI | arrows/Enter/Escape, return to Item, every phase/failure | Keyboard contract is stable; only selected eligible device sends; failures state whether text was accepted. |
| V08-21 | Incoming UI | countdown, Tab order, device-specific labels, cancel/expire | Copy → Save → Reject → Close order is stable; Escape rejects; no body appears in DOM. |
| V08-22 | Transfers UI | All/Incoming/Outgoing/Failed, 20 records | Filtering works; status uses text plus icon; no old-payload retry or sensitive field appears. |
| V08-23 | Surface coordinator | Settings/Pairing/Send/Transfers/Details/Incoming combinations | At most one main modal; Details closes for Send/Transfers; Incoming never hides a security/system blocker; focus returns. |
| V08-24 | Core regression | Link off, discovery failure, auth failure, revoke | History, capture policy, Quick Paste, Recycle Bin, Direct Paste, and Labs behavior remain unchanged. |
| V08-25 | Packaging | final macOS bundle metadata and capabilities | Only required local-network/Bonjour declarations exist; WebView gains no generic network, Keychain, SQL, shell, or filesystem capability. |

## Physical two-Mac acceptance

| Gate | Sample | Pass condition |
| --- | ---: | --- |
| First pairing | 50 normal plus cancel/timeout/mismatch cases | 50/50 normal success; zero incorrect trust; abnormal cases create no trust. |
| Text transfer | 200 values including Chinese, English, code, URL, emoji, multiline, 1 B, and 256 KiB | 200/200 exact content; zero loss, corruption, duplicate, or wrong recipient. |
| Receiver actions | Copy, Save, Reject at least 50 each | Exactly one destination/decision per transfer. |
| Latency | same AP, distance, and Wi-Fi jitter | Send click to Incoming card p50 ≤ 2 s and p95 ≤ 5 s. |
| Cancellation | sender cancel, timeout, either app exit | Both devices clear pending body and show compatible terminal status. |
| Revocation | before send, connecting, awaiting, and after completion | In-flight session is terminated; new auth with old identity fails within 1 second. |
| Network | disconnect, AP isolation, sleep/wake, Wi-Fi switch | Accurate failure; no queued delivery; listener recovers within 10 seconds after wake. |
| Lock/user switch | receiver locked and fast user switching | Body is not accepted; sender receives receiverLocked. |
| Privacy | Copy/Save/Reject/Cancel/failure with unique sentinels | Only the permitted destination contains body; no prohibited SQLite/WAL/log/Transfers/diagnostic/endpoint exposure. |
| Packet capture | pair and transfer sessions | No clipboard plaintext, no plaintext fallback, and no unencrypted receipt content that reveals the body. |

## Go / No-Go

Go requires all automated checks, design QA, dependency/license review, protocol/threat-model
review, clean upgrade/install packaging, and the full physical matrix above.

Absolute No-Go events are: plaintext or secret leakage, wrong peer trust, unauthenticated or
downgraded transport, duplicate Copy/Save, any clipboard/History write after revoke/cancel/timeout,
payload persistence outside the allowed explicit Save destination, a failed transfer reported as
delivered, or Beta copy before the physical evidence exists.

## Rollback

- Keep one default-off real-transport switch independent from Core settings.
- Disabling Local Link immediately stops browse/publish/listen, closes sessions, clears pending
  bodies, and leaves local History/capture settings untouched.
- Protocol mismatch never falls back; it asks the user to update the peer.
- Missing or migrated trust never silently rebuilds; it becomes `needsRePairing`.
- A security or privacy regression keeps the next build's transport off while preserving Core.

## Required handoff

Every implementation handoff must state automated checks run or skipped, remaining physical-Mac
evidence, rollback behavior, and privacy/security/license impact. A local Git update may record a
reviewable Preview/Alpha implementation, but it must not be described as a Beta release or public
Go until the physical matrix is attached.
