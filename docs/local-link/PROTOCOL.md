# Local Link v0.9 protocol contract

> **Historical v0.9 baseline.** The statements below that describe a non-started production adapter
> are retained as the v0.9 review record. In v1.0, Local Link remains default-off Preview / Alpha,
> but an explicit user enable starts the native TCP listener and Bonjour/mDNS runtime. Pairing uses
> Noise XX, transcript-bound six-digit SAS and both users’ confirmation; a body travels only to an
> explicitly paired local-LAN Mac. See
> [`../clipriva-1.0-implementation-and-acceptance.md`](../clipriva-1.0-implementation-and-acceptance.md)
> and [`PROTOCOL-v1.5.md`](PROTOCOL-v1.5.md) for the current contract. Unit/loopback evidence remains insufficient
> for Beta; the 50-pairing/200-transfer real-Mac gate still applies.

- **Status:** Preview / two-Mac Alpha contract; not Beta evidence
- **Protocol generation:** 1
- **Scope:** one explicitly selected UTF-8 text Item, macOS to macOS, on a reachable local
  network
- **Release gate:** Local Link remains default off and must still be described as Preview until the
  50-pairing / 200-transfer real-Mac matrix passes

The current production adapter is intentionally fail-closed. It starts no listener or Bonjour
runtime, establishes no peer trust/session and delivers no clipboard body. Pairing, confirmation
and Send return content-free failures. This document records candidate components and the contract
that a future end-to-end adapter would have to satisfy; it is not a statement that transport works.

## Candidate implementation mapping

The current v0.9 candidate uses `mdns-sd` for Bonjour/mDNS, `snow` for Noise, Apple's Security
framework through the Rust `security-framework` crate for Keychain, and `zeroize` for the in-process
copy of the long-lived private key. The production service does not start the discovery runtime or
a TCP listener. Noise/framing and strict Receipt/status schemas are exercised in unit and
same-host TCP pre-gate tests only. This mapping is source-level evidence, not remote-Mac delivery or
encryption evidence.

This document defines the security and data-boundary contract that the v0.9 implementation must
meet. It does not by itself prove that two Macs exchanged data securely. Browser fixtures,
loopback transports, unit tests and a successful single-Mac build are development evidence only.

## Non-goals

Local Link is not account identity, cloud sync, full-history sync, an Internet relay, offline
delivery, a background clipboard mirror, file transfer or a general remote-control channel. It has
no plaintext or legacy-protocol fallback. An unreachable peer produces a terminal, content-free
failure; ClipRiva does not queue the selected text for later delivery.

## Components and ownership

| Component | Responsibility | Durable data |
| --- | --- | --- |
| Native Local Link service | Current fail-closed command/state boundary; future owner of listener, discovery, pairing and sessions | Content-free settings and summaries only |
| macOS Keychain | Protect the local long-lived Noise identity through `security-framework` | One versioned private/public identity blob; never exposed to the WebView |
| SQLite | Store explicit opt-in, trusted-peer public identity binding and bounded transfer summaries | No plaintext, ciphertext, nonce, session key, network endpoint or pending payload |
| Process memory | Hold handshake state, session keys, replay/rate-limit state and undecided plaintext | Current synthetic/pre-gate pending state expires under an autonomous worker and clears on implemented terminal/disable/revoke/exit paths; disconnect/lock/user-switch remain production gates |
| React WebView | Present UI-safe device and transfer state and request named actions, including opaque reveal intent | No socket, raw packet, private key, trust anchor, endpoint or plaintext receive API/response |
| Native AppKit presentation | Draw a short-lived zeroizing copy for an explicit reveal request | No durable data; complete forced-close behavior still requires real-macOS lifecycle evidence |

Core capture, History, Quick Paste, Recycle Bin and Labs do not depend on the Local Link service.
Disabling or failing Local Link must leave those paths usable.

## Consent and listener lifecycle

1. New installations start with Local Link off.
2. An upgrade from the v0.6/v0.7 control-plane preview also starts the real transport off. A stored
   preview-era `enabled` value is not consent to bind a port or advertise a service.
3. The v0.9 production adapter does not start the native listener or Bonjour registration after
   enablement. A future adapter may do so only after separate review and explicit current-version
   consent.
4. The current disable path clears in-process candidate state. Any future enabled adapter must also
   stop advertisement/listening, close sessions and clear pending plaintext/session material.
5. Starting ClipRiva, opening History, Quick Paste or Settings, or enabling Labs never starts Local
   Link networking.
6. Keychain or listener setup failure is fail-closed: the UI reports an unavailable or identity
   failure and does not substitute an unprotected identity or plaintext transport.

The current adapter should not trigger a Local Network/firewall prompt because it starts no network
runtime. Prompt behavior becomes a real-device gate if a listener is enabled in a later candidate.

## Discovery

The discovery component is configured for Bonjour/mDNS service type `_clipriva._tcp.local.`. Its
metadata builder creates a fresh random `ll-<UUID>` instance and `.local.` host label. The TXT
allow-list contains only `v=<protocol generation>` and `pair=0|1`. Production does not start this
runtime or publish/browse any record. A future runtime would receive a new random instance on each
start; no periodic within-runtime rotation is claimed.

- If a future runtime is enabled, outside an explicit 60-second **Add device** window it may expose
  only the anonymous instance generated for that runtime, protocol generation and listener port.
- A device display name, pairing code and comparison fingerprint are available only during the
  pairing window or to an already authenticated trusted peer.
- Discovery never advertises clipboard content, History metadata, account data, a stable device
  name, a private key or a persistent network identifier.
- IP addresses and ports are transient network facts. They are not written to SQLite, Transfers,
  diagnostics or logs.
- Discovery success is not trust. An unknown peer cannot send a transfer request or learn trusted
  device data without completing the pairing ceremony.

mDNS is local-link discovery, not a proof that the TCP endpoint can never be routed. Mutual
authentication remains mandatory for every connection, including connections that reach the
listener without discovery.

## Identity and Keychain lifecycle

The local Noise static private key is generated by `snow` and stored with its matching public key in
a 65-byte versioned macOS generic-password item: one version byte, 32 private-key bytes and 32
public-key bytes. The Keychain service is `com.clipriva.desktop.local-link.identity`, the account is
`noise-static-v1`, the item label is `ClipRiva Local Link identity`, and Keychain synchronization is
disabled. The item uses `AccessibleWhenUnlockedThisDeviceOnly`. The public key and its SHA-256
fingerprint are not secret. Signed/reinstall/upgrade access behavior remains an unresolved Beta
review item and must be verified on real macOS before promotion.

Rules:

- The secret is never stored in SQLite, preferences, source, logs, diagnostics, crash reports or
  IPC responses.
- A normal application restart reuses the same identity. An inaccessible, malformed or missing
  item does not silently replace an identity that has trusted peers; Local Link enters
  `needsRePairing` or an identity-error state.
- Pairing binds a peer's static Noise public key and fingerprint, not its display name, IP address,
  port, pairing code or Bonjour instance.
- Revoking a peer deletes or invalidates that peer binding and closes its sessions; it does not
  rotate the local identity or delete local History.
- Resetting the Local Link identity is a separate destructive action. The implemented source/unit
  path keeps Local Link off, clears pending/receipt state, deletes the Keychain item, marks every
  local peer binding `needsRePairing`, and requires every device to pair again. Its UI requires an
  explicit confirmation and states that History is not deleted. Signed Keychain integration and
  reinstall behavior remain Preview/Alpha gates rather than verified recovery claims.
- Removing only `~/Library/Application Support/com.clipriva.desktop/` does not necessarily remove
  the Keychain item. Product reset and uninstall guidance must say so explicitly.
- Identity rotation or migration is never silent. Failure falls back to re-pairing, not trust based
  on an old display name or endpoint.

## Gated pairing ceremony

Production pairing is not connected in v0.9. Strict `PairingProposal`, `PairingConfirm` and
`PairingCommit` message schemas reject unknown fields, invalid identifiers/fingerprints, incompatible
versions and expired proposals, but they do not create trust by themselves. A future pairing
implementation is permitted only
during the user-started 60-second window and must satisfy this ceremony.

1. The two native peers establish a fresh Noise XX handshake using fresh ephemeral keys.
2. Both UIs derive and display the same six-digit short authentication string (SAS) and a short
   fingerprint bound to the handshake transcript and both static public identities.
3. Each side shows the peer display name only as a label; users compare the SAS/fingerprint on both
   Macs.
4. Trust is committed only after both sides explicitly confirm and the transcript/identity binding
   still matches.
5. Mismatch, timeout, cancellation, duplicate request, protocol failure or either side declining
   clears the provisional state and creates no trusted peer.

The six-digit value is not a password, long-term key or future-session credential. It and its hash
must not survive a completed, cancelled or expired ceremony. A retry creates new ephemeral keys,
session identifier, transcript, SAS and nonce material.

## Authenticated transport

The candidate uses `snow` 0.10.0 with
`Noise_XX_25519_ChaChaPoly_BLAKE2s`. There is no plaintext codec or fallback. Current tests connect
two independent identities through in-process and same-host `TcpListener` byte streams; production
opens no TCP session. These harnesses test the real frame I/O path but do not prove Bonjour,
permissions, a remote Mac or a signed bundle. A future adapter must authenticate the peer static key
against an explicitly confirmed trusted-device binding before accepting any clipboard plaintext.

The `snow` channel uses prologue `ClipRiva Local Link v1`. Handshake messages use a four-byte
big-endian length prefix and a 1,024-byte maximum. Transport ciphertext frames use the same prefix,
a 65,535-byte maximum and a 60 KiB plaintext maximum. JSON control frames and binary payload frames
have separate one-byte type markers. A payload is split into ordered 48 KiB chunks, each bound to
the transfer ID and carrying its index/count; the reassembled payload must match the offer's size,
SHA-256 digest and UTF-8 requirement.

The decrypted application message is a versioned, finite-schema envelope:

| Field | Rule |
| --- | --- |
| `version` | Supported protocol generation; mismatch fails before accepting content |
| `session_id` | Fresh opaque identifier for this connection; never a device identity |
| `transfer_id` | Fresh opaque identifier for this user attempt; idempotency key for the receiver |
| `nonce` | Fresh per attempt/message material; reuse is rejected |
| `timestamp` / `ttl` | Bounded clock-skew check and a maximum 60-second receive decision window |
| `content_type` | `text/plain; charset=utf-8` only in v0.9 |
| `size` | 1 through 262,144 UTF-8 bytes and equal to the decoded payload size |
| `digest` | Integrity/correlation value inside the authenticated encrypted envelope; not persisted in Transfers |
| `payload` | The selected UTF-8 text; never included in a receipt, log, summary or IPC response |

Invalid UTF-8, empty text, oversized frames, unknown fields that change semantics, an unsupported
version, expired TTL, duplicate identifiers or an identity mismatch fail closed. The sender must
rerun the capture privacy policy immediately before creating the plaintext envelope. Pause,
excluded-App and sensitive-content results prevent any payload from reaching the transport.

## Gated receive decision and receipts

There is no production inbound peer path in v0.9. Synthetic/pre-gate offers may exercise the native
receiver seam: pending plaintext stays in native process memory and an autonomous deadline worker
removes it at the offer deadline even if the WebView never polls Transfers. The public request
contains only the trusted device label, type, size and expiry. `reveal_local_link_transfer` accepts
an opaque transfer ID and returns `void`; AppKit receives only a short-lived zeroizing native copy.
The WebView never receives the body. Closing an already visible native surface on lock/user-switch/
disconnect remains a real-macOS integration gate.

The receiver permits exactly one terminal action:

| Action | Local effect | Receipt |
| --- | --- | --- |
| Copy | Write the system clipboard with self-capture suppression; do not create History | `copied` |
| Save | Create/update local History under the normal local policy; do not change the clipboard | `saved` |
| Reject / Escape | Clear pending memory; write neither clipboard nor History | `rejected` |
| Sender cancel | Clear pending memory | `cancelled` |
| 60-second expiry | Clear pending memory | `expired` |
| Disable, revoke, disconnect, lock, user switch or exit | Clear pending memory and close the session | Content-free terminal failure or cancellation |

The finite terminal schema is:

- `Receipt { transfer_id, receipt }` records the receiver result.
- `ReceiptAck { transfer_id, receipt }` acknowledges that exact terminal result.
- `StatusQuery { transfer_id }` asks for the already-recorded result without resending a body.
- `StatusResponse { transfer_id, receipt? }` returns that result or a content-free unknown state.

`receipt.outcome` is exactly `copied`, `saved`, `rejected`, `cancelled`, `expired` or
`notDelivered`. Only `notDelivered` may carry exactly one allow-listed `failure`; all other outcomes
must carry none. Unknown fields are rejected. The first valid terminal receipt wins; identical
duplicates are idempotent and conflicting/late outcomes do not replace it. Receipts contain no
body, preview, digest, source application, path, endpoint or key. A lost receipt does not authorize
a second local side effect. The current state machine tests convergence under loss, duplication and
reordering; the production sender/receiver retry loop remains an end-to-end gate.

## Replay, duplicate and abuse controls

- A `transfer_id` is processed once. A duplicate returns a content-free duplicate result without a
  second Copy or Save.
- A retry is a new user attempt from a currently selected Item. It creates a new connection/session
  as needed and a new transfer ID and nonce; ClipRiva never replays an old payload from Transfers.
- Expired timestamps/TTLs and nonce/session misuse are rejected.
- Each trusted peer may have at most one pending request and may submit at most five requests per
  rolling minute. The application accepts at most three pending requests globally.
- Rate and concurrency checks occur before retaining plaintext. Screen lock, user switching and app
  shutdown reject before retaining plaintext.
- Revocation closes current sessions, clears undecided plaintext, invalidates the peer binding and
  causes new authentication attempts by the old identity to fail.

The v0.9 SQLite replay tombstone stores only the peer-device reference, opaque transfer ID, opaque
session ID, nonce and expiry. It is retained for at most 24 hours, independently of the transfer
offer's 60-second TTL, and expired rows are pruned. It contains no body, digest, endpoint,
private/session key or reusable transport state. Rate-limit counters remain process-memory state.

## Durable metadata

Transfer summaries retain at most 20 records and no longer than 24 hours. Allowed fields are the
opaque transfer ID, direction, peer-device reference, fixed text kind, byte size,
timestamps/expiry, finite status, receiver-action label and finite failure class. The schema must
not retain the selected Item as a retry queue, pending/cipher text, digest, nonce, session key,
source App, path, account, IP address or port.

Trusted-device metadata may retain an opaque device ID, local display label, peer static public
identity/fingerprint, trust state/timestamps, compatible protocol range and last-seen time. Online
state and current endpoints are ephemeral.

The named `clear_local_link_transfers` action deletes only terminal content-free summaries. It does
not delete active/pending requests, replay controls, trusted devices, the Keychain identity or local
History, and it cannot recover an old payload.

## Failure and logging contract

User-visible failure classes are finite and content-free: unavailable, authentication, version,
locked, busy, rejected, cancelled, expired, revoked, rate-limited, policy-blocked or internal
failure. “No text was delivered” is shown only when content was not accepted into receiver pending
memory. “Pending text was cleared” is used when content may have reached that memory and a terminal
cleanup occurred.

Logs and diagnostics use explicit allow-lists. They may include protocol generation, broad phase,
finite failure class, monotonic duration and aggregate counts. They never include plaintext,
ciphertext, previews, digests, transfer/session IDs, fingerprints, device names, IP/port, Keychain
values or raw library errors that may embed those fields.

## Go / No-Go evidence

The implementation remains Preview/Alpha and default off until all applicable automated checks and
the real-device matrix pass. The minimum real-device gate is:

- 50 successful/negative pairing ceremonies with zero incorrect trust decisions;
- 200 UTF-8 transfers with zero corruption, loss or duplicate side effects;
- Copy, Save and Reject at least 50 times each;
- cancellation, expiry, revocation, replay, sleep/wake, lock/user-switch, Wi-Fi changes and AP
  isolation;
- p50 at most 2 seconds and p95 at most 5 seconds from Send to receive-card presentation;
- packet evidence showing encrypted application payloads and no application cloud relay;
- SQLite/WAL, file, Transfer and structured-log scans showing no forbidden plaintext for Copy,
  Reject, Cancel and failure paths.

Any plaintext fallback, incorrect trust, body disclosure, duplicate Copy/Save, post-revocation
access or failed transfer reported as delivered is an absolute No-Go.
