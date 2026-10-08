# Local Link v1.5 protocol contract

- **Status:** default-off v1.5 engineering candidate, not public-release evidence.
- **Protocol generation:** 3.
- **Scope:** one explicitly selected, policy-approved UTF-8 text item between authenticated, explicitly paired Macs on the same reachable LAN.

This is the current protocol contract. [`PROTOCOL.md`](PROTOCOL.md) is retained as the historical
v0.9 record. Unit, browser-fixture, loopback and same-host evidence do not satisfy the physical
two-Mac release gate.

## Compatibility and ownership

Version 3 keeps the authenticated, content-free `Viewed` lifecycle frame and adds explicit
`StatusResponse` states (`unknown`, `pending`, `terminal`) for lost-receipt reconciliation. v1/v2
peers are incompatible: version validation fails before an offer or body bytes. There is no
downgrade, plaintext codec, legacy fallback, cloud relay, offline queue or automatic re-delivery.

Only the native Local Link service owns Bonjour/mDNS, TCP, Noise, peer binding, replay state and
decrypted pending text. The WebView uses typed Tauri commands with opaque transfer IDs and never
receives a pending body, key, packet, endpoint, nonce or digest.

## Lifecycle

```text
connecting → encrypted → awaitingReceiver → viewed → copied | saved | rejected | expired | failed | cancelled
                                      ↘ reconciling → terminal | outcomeUnknown
```

- `connecting`/`encrypted` are pre-commit sender phases and are not delivery.
- `awaitingReceiver` means the receiver holds the body in zeroizing native memory, with a 60-second
  deadline.
- `viewed` means the receiver opened Incoming Center. It contains only the transfer ID and no
  body, preview or receiver decision.
- Copy, Save and Reject are receiver actions. Copy writes only the system clipboard; Save writes
  only local History; Reject writes neither.
- Terminal outcomes clear pending body immediately. The content-free summary may retain only the
  finite state/action/failure metadata for the normal bounded summary window.
- `reconciling` means the complete encrypted payload crossed the sender’s durable no-replay
  boundary and `PayloadComplete` may have been attempted, but no terminal receipt was observed. It
  authorizes metadata-only status queries, never payload replay.
- `outcomeUnknown` is terminal uncertainty. It is used when bounded reconciliation or native Copy
  recovery cannot prove an outcome and must never be presented as delivered or safely retryable.

## Authenticated frames

All frames run inside `Noise_XX_25519_ChaChaPoly_BLAKE2s` and the versioned, strict wire schema.
Control messages contain no plaintext.

| Frame | Meaning | Body permitted? |
| --- | --- | --- |
| `TransferOffer` / `OfferDecision` | Validate a new explicit request before payload transfer. | No |
| Payload chunks / `PayloadComplete` | Ordered chunks of the one text request. | Payload chunks only |
| `Viewed` | Receiver opened Incoming Center. | No |
| `Receipt` / `ReceiptAck` | Receiver’s terminal result and exact acknowledgement. | No |
| `StatusQuery` / `StatusResponse` | Reconcile known terminal state without resending text. | No |
| `Cancel` | Sender withdraws an unresolved request. | No |

`Viewed`, `Cancel`, receipts and status messages validate the opaque transfer ID. Unknown fields,
bad identifiers, invalid framing, wrong protocol version, bad authentication, replay and expired
offers fail closed.

An authenticated `StatusQuery` is answered only when the stored incoming transfer belongs to the
Noise-authenticated peer. `StatusResponse` is exactly `unknown`, `pending`, or a valid terminal
receipt. The terminal receipt outcomes are `copied`, `saved`, `rejected`, `cancelled`, `expired`,
`outcomeUnknown`, or `notDelivered`; only `notDelivered` carries a finite failure reason.

After the complete encrypted payload is written, the sender persists the no-replay boundary before
attempting `PayloadComplete`. A transport failure from that point moves the sender to
`reconciling`. The worker queries content-free status immediately and after 250 ms, 1 s, 3 s and
10 s, using only authenticated connections and current in-memory discovery. The persistent
active-time deadline is 20 seconds and is not reset by process restart. Each round checks at most
four de-duplicated endpoints; one endpoint query is bounded to 1.5 seconds and connect/Noise/Hello
socket I/O to 500 ms, all capped by the remaining total deadline. Known sleep, inactive-session and
unsatisfied-network periods pause active time and extend the persisted deadline by the measured
pause. Workers are lifecycle-generation scoped, so a stale worker cannot write `outcomeUnknown` or
replace a resumed worker. A terminal Receipt that cannot be committed to SQLite does not stop the
worker; it continues within the same deadline. A known terminal receipt converges the sender; an
exhausted/unknown result becomes `outcomeUnknown`. Reconciliation never persists or replays the old
body.

## Cancellation and terminal convergence

Cancellation is permitted only while the receiver has not committed Copy, Save or Reject. Before
the sender’s no-replay boundary (`connecting`/`encrypted`), it may record `cancelled` locally,
signal the live authenticated worker and stop payload work. After that boundary
(`awaitingReceiver`/`viewed`), Cancel is provisional: the sender remains `reconciling` until an
authenticated receiver Receipt or StatusResponse proves the terminal outcome. The receiver accepts
`Cancel` before payload arrival or while an unresolved payload is live, clears memory and returns
the content-free `cancelled` receipt. If that Receipt is lost, the sender continues StatusQuery.

If the receiver’s terminal decision had already won before it processed `Cancel`, that decision is
authoritative. The sender reconciles the authenticated receipt rather than claiming a false
`cancelled` result; Copy, Save or Reject may therefore replace only provisional local cancellation
or `outcomeUnknown`. Duplicate, delayed and out-of-order lifecycle frames are idempotent; no
terminal state may create a second clipboard/history side effect or revive a body.

## Persistence and privacy

`local_link_transfers` records only opaque ID, peer reference, direction, fixed text kind, byte
size, timestamps, finite lifecycle state, terminal action and finite failure class. The v1.5 schema
adds `reconciling`/`outcomeUnknown` but no plaintext, preview, ciphertext, digest, endpoint, source
app, path, key, nonce or old payload column. Pending text is zeroizing native process memory only;
it is never placed in SQLite/WAL, blob storage, logs, diagnostics or a WebView DTO, and cannot
survive restart.

Copy uses a content-free `local_link_effect_claims` row containing transfer ID, fixed action/state,
a caller-generated random marker, pasteboard change count and timestamps. The claim is committed
before the native pasteboard write; text and marker are written together; claim/terminal metadata
then complete in one transaction. On restart, only the matching current marker proves completion.
All other prepared claims become `outcomeUnknown`; the native write is never repeated. Save commits
the History row and terminal transfer state in one transaction. Reject commits its terminal CAS
before the in-memory body is removed.

The macOS lifecycle adapter observes workspace sleep/wake, session active/inactive and termination,
plus Network.framework path status. The supervisor uses generation tokens, a 500 ms debounce and
bounded 250 ms/1 s/2 s/5 s retries. Sensitive suspension clears pending bodies and closes native
reveal before transport stop. Status reconciliation uses the same generation invalidation and
measured pause/resume deadline extension. These implementation/reducer tests do not substitute for
the real-Mac lifecycle gate.

See [PRIVACY.md](../../PRIVACY.md), [data-flow.md](../privacy/data-flow.md), and
[module-boundary.md](../open-source/module-boundary.md) for the corresponding user and module
boundaries.

## Beta gate

This Candidate remains No-Go for Beta until the same candidate SHA has real two-Mac evidence,
including 50 pairing cycles, 200 text requests, cancellation/receiver-decision races, lifecycle
cleanup, disk/log scans, AP isolation, network switching, sleep/wake and performance checks.
