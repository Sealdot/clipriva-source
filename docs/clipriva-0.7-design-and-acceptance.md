# ClipRiva v0.7 Local Link Text Beta — design and acceptance plan

## Decision

v0.7 is deliberately a **single-text, same-LAN, trusted-Mac** release. It
does not add image or file transfer, account identity, cloud relay, offline
delivery, history sync, background queues, or non-macOS clients. A transfer is
successful only after the receiver explicitly chooses **Copy** or **Save**.

This document freezes the product contract before a real transport is enabled.
The current native implementation remains a default-off control-plane preview;
it must not be presented as LAN delivery or encrypted transport until the
protocol, threat-model, implementation, and two-Mac evidence below are all
complete.

## User contract

| Surface | v0.7 rule | Acceptance evidence |
| --- | --- | --- |
| Devices | One Local Link master switch. New installs are off; off means no listener, discovery, connection, or inbound delivery. `Add device` opens a 60-second pairing window only. | Default/disable lifecycle test; manual two-Mac observation. |
| Trust | Both Macs compare the same six-digit code, device name, and short identity fingerprint, and both confirm. Timeout, mismatch, cancellation, or identity change creates no trust. Revocation immediately blocks new sessions and clears undecided inbound content. | Pairing state-machine, identity mismatch, expiry, revoke, and restart tests. |
| Send | Quick Paste `Shift+Command+S` and History's selected item only. `Enter` preserves local restore/paste behavior. The sender rechecks pause, excluded-app, sensitive-content, text encoding, trust, reachability, and the 256 KiB UTF-8 limit immediately before the payload is built. | Eligibility matrix, keyboard, and transport-contract tests. |
| Receive | Until a decision, plaintext remains only in memory. Copy writes only the system clipboard; Save writes only local History; Reject immediately clears it. No automatic receipt, no automatic forwarding, and no payload/preview in Transfers. | SQLite/filesystem/clipboard boundary tests and crash/timeout/revoke cases. |
| Status | Connecting, Awaiting receiver, Completed, Rejected, Failed, Expired, and Cancelled describe real protocol state. A retry creates a new transfer ID, nonce, and session after a fresh policy/trust check. | Transition, replay, retry, duplicate/ordering, and retention tests. |

## Architecture and safety gates

1. The React WebView keeps the existing narrow, typed IPC surface. It never
   receives a socket, network endpoint, plaintext transfer payload, private
   key, arbitrary filesystem access, or SQL capability.
2. The native Local Link service owns discovery, pairing, trusted-peer
   identity, session lifecycle, and content-free transfer summaries. Core
   capture, History, Quick Paste, Recycle Bin, and Labs must remain functional
   with Local Link disabled or unavailable.
3. A production transport requires a separately reviewed protocol specifying
   authenticated encryption, peer-key storage, protocol versioning, replay and
   duplicate handling, address lifetime, size/timeout limits, shutdown,
   revocation, and error classification. Pairing code and display name alone
   are never an identity or encryption claim.
4. No content, preview, file path, IP/port, session key, private key, pairing
   code, or complete fingerprint may enter SQLite transfer summaries,
   diagnostics, logs, or export artifacts. Summaries retain at most 20 recent
   records and automatically clear after 24 hours.

## Delivery order

1. **Contract hardening:** make the UI truthful, remove the long-lived
   discovery affordance, restrict the feature surface to text, set the 256 KiB
   boundary, and make Copy/Save/Reject materially different native actions.
2. **Protocol gate:** publish and review protocol, threat-model, data-flow and
   key-storage decisions; then update the privacy and module-boundary records
   before enabling any socket or discovery code.
3. **Trusted transport:** implement discovery, dual confirmation, authenticated
   encrypted delivery, content-free status receipts, expiry, cancellation,
   replay defense, and revocation.
4. **Beta validation:** run automated coverage plus the two-Mac matrix below.
   Browser fixtures and injected loopback transports are UI/state tests only;
   they are never network evidence.

## Automated test matrix

| ID | Scenario | Required assertion |
| --- | --- | --- |
| LL-01 | Default off / disable | No networking lifecycle is started; Core regression paths remain available. |
| LL-02 | Pairing | Both confirmations succeed only with matching code and identity; timeout/cancel/mismatch persists no trust or plaintext code. |
| LL-03 | Send eligibility | Non-text, absent selection, no trusted-online device, disabled Link, policy block, invalid UTF-8, and `>256 KiB` do not construct a payload or leak content. |
| LL-04 | Receive actions | Copy changes only the system clipboard and suppresses self-capture; Save changes only History; Reject leaves neither location changed. |
| LL-05 | Cleanup | Expiry, restart, rejection, cancellation, revoke, disconnect, and capture failure clear pending in-memory plaintext. |
| LL-06 | Protocol state | Illegal/duplicate/out-of-order messages cannot complete twice; Completed follows a receiver action only. |
| LL-07 | Retry | Only a recoverable failure can retry; it rechecks current policy/trust and produces a new ID/session/nonce. |
| LL-08 | Metadata boundary | DB/API/diagnostic records contain only the allow-listed summary fields; old records roll to 20 entries and expire after 24 hours. |
| LL-09 | UI/accessibility | Tab order, labelled controls, Escape safety, primary Copy action, reduced motion, and no text in Transfers. |
| LL-10 | Core regression | Quick Paste, History, Recycle Bin, capture policy, and permissions pass unchanged with Link disabled and after Local Link errors. |

## Manual Beta Go / No-Go

Go requires a reviewable two-Mac script: at least 50 normal-LAN first-pair
attempts (>=95% within 60 seconds), at least 200 text transfers (>=98%; Chinese,
English, code, and URLs), p50 receipt-card latency <=2 seconds and p95 <=5
seconds, plus filesystem/SQLite/clipboard/log checks proving no undecided
payload lands on disk. Revocation must block new sessions and clear undecided
payloads in every tested scenario.

No-Go includes treating a browser fixture or loopback as network proof,
persisting undecided text, trusting a display name/IP alone, delayed revocation,
or reporting a failed/queued transfer as completed.
