# Security policy

ClipRiva records clipboard data by design. Privacy regressions, incorrect device trust,
unauthenticated network access and unintended disclosure are treated as security issues.

## Supported versions

Security reports are accepted for the latest source on `main`. There is no supported public binary
release yet; stable-version support rules will be documented before one is published. Local Link
remains a default-off Preview / two-Mac Alpha until its recorded release gates pass. Preview/Alpha
availability is not a claim that the feature is ready for sensitive production data.

## Reporting a vulnerability

Do not open a public issue for a vulnerability that may:

- expose clipboard content, pending Local Link text, History or local files;
- bypass capture, send, size, deny-list or sensitive-content policy;
- establish trust with the wrong Local Link peer;
- permit a man-in-the-middle, replay, duplicate Copy/Save or post-revocation session;
- disclose or replace the Local Link Keychain identity;
- start a listener or discovery without explicit consent;
- expose an unintended network, Keychain, SQL, filesystem or shell boundary;
- replace, corrupt or redirect an app-owned blob so unverified content is restored;
- place plaintext, keys, device identity or endpoints in logs/diagnostics.

Report privately through [GitHub private vulnerability reporting](https://github.com/Sealdot/clipriva-source/security/advisories/new)
for the public source repository or by email to
`sealdot404@gmail.com`.

Do not include real clipboard content, credentials, Keychain values, private/public identity keys,
fingerprints, device names, IP addresses/ports or unredacted packet captures. If neither private
channel is available, contact a maintainer without reproduction details or sensitive data. A
complete private report should include:

- the affected version or commit;
- the macOS version and hardware architecture;
- whether Local Link was off, pairing, connected or awaiting a receiver decision;
- a minimal reproduction using clearly synthetic text and device labels;
- the data, network, identity or permission boundary affected;
- whether clipboard content, trust state or private key material may already have been disclosed;
- whether the issue remains after disable, revoke, restart or Local Link identity reset.

## Core trust boundaries

- Core capture, search, History, Quick Paste and Labs work without Local Link, an account or a
  network service.
- The WebView has a fixed command allow-list and no arbitrary SQL, path, shell, socket, endpoint,
  raw packet or Keychain command.
- Rich-text bytes and file-reference lists are stored in the private content-addressed blob
  directory; referenced file contents are not imported. Blob keys are bounded canonical SHA-256
  identifiers, and every read verifies private directories, regular-file identity, size and digest
  before bytes can reach a restore path.
- Application deny-list and sensitive-content rules run before persistence and before a Local Link
  outbound payload is created.
- Restore mode does not require Accessibility permission.
- Direct paste requires an explicit setting and macOS Accessibility trust. If trust is denied or
  revoked, ClipRiva restores the clipboard and does not synthesize a key event.
- Labs is local and disabled by default. It has no cloud executor or model-provider connection.

## Local Link trust boundaries

Local Link is a separate default-off native Preview / Alpha. Only an explicit user enable starts its
private TCP listener and Bonjour/mDNS runtime; disable, reset and exit stop them and clear active
native state. The only permitted body destination is an explicitly paired local-LAN Mac over
`Noise_XX_25519_ChaChaPoly_BLAKE2s`; there is no plaintext, cloud-relay or offline-queue fallback.
The detailed contract and threats are in
[`docs/local-link/PROTOCOL.md`](docs/local-link/PROTOCOL.md) and
[`docs/local-link/THREAT_MODEL.md`](docs/local-link/THREAT_MODEL.md).

The current contract, tests and Preview/Alpha evidence gate are recorded in
[`docs/clipriva-1.0-implementation-and-acceptance.md`](docs/clipriva-1.0-implementation-and-acceptance.md).

- New installations and upgrades from the no-network preview do not inherit consent to bind a TCP
  port or advertise Bonjour/mDNS.
- Explicit enable starts only the documented native TCP and Bonjour/mDNS runtime. Discovery uses
  fresh random instance/host labels and allow-listed `v`/`pair` metadata; discovery is routing, not
  trust.
- The protocol uses `Noise_XX_25519_ChaChaPoly_BLAKE2s`, bounded framing, strict pairing schemas,
  bilateral SAS confirmation and finite Receipt/Ack/Status convergence with fresh ephemeral
  material. Unit and same-host tests do not prove authentication or delivery on two physical Macs.
- Pairing/confirmation and Send fail closed with content-free errors unless consent, identity,
  authenticated peer binding, policy and transport state all succeed. There is no plaintext/legacy
  fallback.
- The local long-lived Noise identity private key is stored as a versioned macOS Keychain item and
  is available only to the native identity adapter. It is never an SQLite, IPC, log or diagnostic
  value.
- Keychain failure is fail-closed. Missing/corrupt identity with existing peers requires explicit
  reset/re-pairing rather than silent key replacement.
- The Local Link Keychain item is non-synchronizing, versioned and protected with
  `AccessibleWhenUnlockedThisDeviceOnly`. An explicit reset keeps Local Link off, clears pending
  state, deletes the item and marks local peers `needsRePairing`; signed/reinstall behavior remains a
  Preview/Alpha gate.
- A Send attempt reruns policy/type/size checks for one explicitly selected UTF-8 text Item before
  authenticated transport. No body enters an offline queue.
- Authenticated inbound pending text is native-memory-only and expires under a background deadline
  worker within the offer's 60-second bound; the WebView remains metadata-only.
- Reveal accepts only an opaque transfer ID and returns `void`; native AppKit code receives a
  short-lived zeroizing clone rather than sending the body through Tauri IPC. Expiry, terminal
  actions, revoke, disable, exit and restart clear implemented buffers. Lock/user-switch/disconnect
  behavior and forced closure of an already visible native surface remain real-macOS gates.
- Transfer/session IDs, nonces, TTL checks and bounded deduplication make the receiver action
  idempotent. A retry creates new material from a current explicit Item selection; Transfers is not
  an old-payload queue.
- The first terminal Receipt wins. Only `notDelivered` carries a finite failure reason; duplicate,
  reordered or conflicting receipts cannot replace the recorded terminal result, and status queries
  never authorize a second side effect.
- One trusted peer may have one pending request and at most five requests per rolling minute; the
  application has at most three pending requests globally. Limits apply before retaining plaintext.
- Revocation closes current sessions, clears pending text, invalidates the peer binding and causes
  authentication by the old identity to fail. It does not delete local History or rotate the local
  identity.
- Transfer summaries retain at most 20 content-free records for at most 24 hours. Logs and
  diagnostics exclude body, preview, digest, IDs, name, fingerprint, source App, path, address/port
  and key material.

Bonjour/mDNS is local-link discovery, not proof that a TCP address can never be routed. Every
reachable connection requires mutual authentication. The application has no cloud relay, account
directory or offline delivery queue.

## Threat model and security-analysis artifacts

The repository-wide model is
[`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md); the Local Link model remains the
detailed protocol supplement. Review both against the exact frozen release commit.

Security findings, proofs of concept and raw scan artifacts are sensitive. Keep them outside the
checkout in a private location with an approved reader list and retention period. Do not commit or
paste raw findings, source excerpts, tokens or internal endpoints into public issues, pull-request
comments or release notes. Use a private security advisory or another maintainer-approved private
channel.

Any optional external source scan requires maintainer approval for the selected service/account,
credential scope, source handling, cost and artifact visibility before it runs. The scanner is
development/release tooling, not a ClipRiva dependency. Cancellation, timeout, tool error, missing
coverage or invalid artifacts are `incomplete/error`, never “no vulnerabilities.” A finding is
resolved only after targeted validation on current code; disappearance from an incomplete result is
`unknown`.

## Local Link security release gate

Preview/Alpha must remain default off and must not be promoted to Beta until reviewers approve the
actual Noise parameter string/library, Keychain service/account/accessibility attributes, Bonjour
service/TXT schema, listener binding, message framing, replay retention, lock detection and cleanup
paths. The same build SHA must pass the two-real-Mac matrix: 50 pairing runs, 200 text transfers,
Copy/Save/Reject idempotency, cancellation, expiry, revocation, replay, lock/user-switch, network
isolation/change, sleep/wake, performance, packet inspection and disk/log scans.

Browser fixtures, same-process loopback and single-Mac unit/integration tests do not satisfy the
network/authentication gate. Plaintext fallback, incorrect trust, private-key disclosure, pending
body persistence, duplicate Copy/Save, successful post-revocation authentication or failure shown
as delivered is an absolute No-Go.

## Maintainer response

Sealdot404 will acknowledge a complete report as soon as practical, reproduce it, identify affected
versions and coordinate disclosure after a fix or mitigation is available. If a report suggests
actual key or clipboard disclosure, maintainers should preserve redacted evidence, stop affected
build distribution and provide explicit identity-reset/re-pairing guidance; do not publish packet,
Keychain or clipboard evidence.
