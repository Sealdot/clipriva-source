# Changelog

All notable changes to ClipRiva are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions are intended to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Smart Collections with bounded local rules and explicitly opened, searchable local notes.
- A Labs-gated Filter Composer for allow-listed multi-step text transformations, live preview,
  edit/delete flows and contextual shortcut slots 1–9.
- A process-memory Stack for intentional multi-clip collection, ordering and sequential activation,
  with a configurable shortcut distinct from Quick Paste.
- A default-off, capability-gated local automation protocol for the bundled CLI and Apple
  Shortcuts shell actions.

### Changed

- Aligned package, Cargo, Tauri and active documentation at the private `2.0.0-alpha.1`
  engineering-candidate version.

### Security

- Keep notes out of ordinary Item DTOs and require explicit content access; Smart Collection rules,
  Filter pipelines, Stack state and automation capabilities are all finite and bounded.
- Keep Stack state in process memory and keep automation disabled by default with no arbitrary
  shell, SQL, filesystem, destructive-history or remote-Agent capability.

## [1.5.0] - 2026-08-09

This section freezes the private `1.5.0` release candidate. It is not a publication record; Local
Link and public distribution remain No-Go until the repository's clean-build, real-two-Mac,
privacy, signing and notarization gates pass.

### Added

- v1.5 Release Candidate organization tools: conservative Command classification and bounded,
  user-authored local tags with History/Quick Paste search and recycle-bin lifecycle coverage.
- A v1.5 implementation, automated-test and real-two-Mac release-gate record.
- A v1.5 first-use and troubleshooting guide covering Quick Paste, tags, Local Link and local-data
  reset boundaries.

- Open-source preparation records, contributor guidance, and community-file scaffolding.
- License decision record, brand policy, privacy/data-flow documentation, and release-process
  templates.
- Local Link v0.9 product, interaction, state/Receipt, security, QA, and Beta-evidence specifications
  that keep single-Mac engineering acceptance separate from the signed two-real-Mac release gate.
- A transport-independent terminal Receipt state machine with Receipt acknowledgement/status-query
  wire schemas, bounded pairing message schemas, and loss/duplicate/reordering tests.
- An explicit Local Link identity reset, terminal-summary cleanup, autonomous pending-transfer
  expiry, and an opaque native secure-reveal command whose IPC response contains no text.
- Local Link v0.8 Preview UI for Devices, pairing comparison, Send, Incoming, and content-free
  Transfers states, with explicit fixture and transport-unavailable labeling.
- Candidate Noise XX framing, macOS Keychain identity, Bonjour metadata, replay protection, and
  bounded receiver state-machine modules; the production network adapter remains fail-closed.

### Changed

- Quick Paste zero-input results now preserve the native recent-activity order instead of
  re-promoting old pinned clips in the WebView.
- Local Link re-pairing now resolves anonymous nearby candidates explicitly instead of passing a
  persisted device identity to the discovery-candidate command.

- Refined the Local Link Preview surfaces with compact device states, user-language send stages,
  native-reveal intent, receiver keyboard actions, content-free diagnostic IDs, non-modal Transfers,
  44-point interaction targets, and identity-reset confirmation.
- Hardened the macOS Local Link identity item with a when-unlocked, this-device-only Keychain access
  class and fail-closed missing-identity handling when trusted peers exist.
- Replaced legacy Local Link retry and preview-trust states with finite v0.8 transfer states,
  upgrade-safe `needsRePairing`, 20-record/24-hour summaries, and a documented two-Mac gate.

### Fixed

- Prevented onboarding, pairing, Send, Incoming, Transfers, and item details from competing as
  simultaneous primary surfaces.

### Removed

- Nothing yet.

### Security

- Kept user tags in local SQLite and out of Local Link payloads, transfer summaries and local
  diagnostics; labels are bounded to eight per Item and 24 visible characters each.

- Kept received text out of public Transfer DTOs and the WebView reveal response; terminal summaries
  can be cleared without creating a payload retry path.
- Kept Local Link default off and its native listener/discovery limited to explicit enable. Public
  release remains No-Go until the two-real-Mac and privacy acceptance matrix passes.

## [0.1.0] - Unreleased

No `0.1.0` Git tag exists at the time of this entry. This historical section records the initial
repository baseline before the private `1.5.0` candidate freeze.

### Added

- Local-first clipboard capture, search, history retention, pinning, and a recoverable recycle bin.
- Text, image, RTF/HTML, and local file-reference capture and restoration for macOS.
- Quick Paste, menu-bar lifecycle, configurable shortcut, safe restore, and opt-in direct paste.
- Local capture controls, sensitive-content pause, application deny-list, and local diagnostics.
- Default-off local Labs enrichment, offline lexical retrieval, and allow-listed local text actions.

### Changed

- Improved core clipboard flows and daily reliability safeguards.

### Security

- Kept Core offline and added capture-time protections for high-confidence sensitive content.
