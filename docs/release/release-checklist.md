# Release checklist

This checklist prepares a release; it does not authorize publication by itself. Do not publish an
unsigned, unnotarized, or unverified artifact as an official stable macOS release.

## Scope and version

- [ ] Confirm the release owner (Sealdot404) and the target version.
- [ ] Confirm `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` have the same
  SemVer version.
- [ ] Update `CHANGELOG.md`, `README.md`, `README.zh-CN.md`, known issues, and roadmap status.
- [ ] Confirm the release scope contains no unreviewed feature, migration, permission, privacy, or
  platform-boundary change.
- [ ] For a build containing real Local Link, confirm new installs and preview-era upgrades keep
  real transport off; no preview preference is treated as listener/discovery consent.
- [ ] For v1.0 Local Link, confirm explicit enable alone starts only the documented private
  listener/Bonjour runtime; pairing creates trust only after both SAS confirmations and Send reaches
  only an authenticated paired peer. Unit/browser/same-host evidence must not be described as
  two-real-Mac transport evidence.
- [ ] Freeze the exact release-candidate commit. Any subsequent feature, dependency, privacy, or
  documentation change requires this checklist's source review to be repeated.

## Source and security review

- [ ] Run `pnpm install --frozen-lockfile` in the release environment.
- [ ] Run `pnpm release:verify`.
- [ ] Run the P0 secret scan with redaction, including reachable Git history.
- [ ] Review the full transitive dependency/SBOM and license report.
- [ ] Run vulnerability scans for the production Node dependency set and Rust lockfile in the
  release environment; record tool versions, findings, and remediation decisions.
- [ ] Confirm `LICENSE`, `THIRD_PARTY_NOTICES.md`, `TRADEMARK.md`, `PRIVACY.md`, and asset
  provenance are current.
- [ ] Review `docs/security/THREAT_MODEL.md` against the exact candidate. Confirm every changed
  surface, asset, attacker, trust boundary and high-risk invariant has current automated evidence
  or an explicit manual/proof gap.
- [ ] Confirm the Local Link protocol, threat model, Keychain lifecycle, Bonjour/TXT schema,
  listener scope, pending-memory lifecycle, failure allow-list and rollback behavior match the
  exact release-candidate source and dependencies.
- [ ] If an external code-security scan was separately authorized, record its exact target, tool
  version/mode, knowledge-base digests, completion state, coverage, finding counts, artifact
  digests and validation decisions. Cancellation, timeout, cost stop, tool error, missing coverage
  or invalid artifact is incomplete and cannot satisfy this item.
- [ ] Keep raw findings, PoCs, source excerpts and scan artifacts outside the checkout in the
  approved private location. Commit only a maintainer-reviewed aggregate summary that contains no
  unresolved vulnerability detail, credential, user data, internal endpoint or personal path.
- [ ] Confirm strict Pairing/Receipt/Ack/Status schemas reject unknown fields and invalid terminal
  failure combinations; conflicting or late receipts cannot replace the first terminal result.
- [ ] Confirm the WebView Transfer DTO and reveal response contain no body/preview/digest. Exercise
  timeout, terminal action, disable, revoke, lock/user-switch and exit while the native reveal is
  visible; any stale plaintext surface is a No-Go.
- [ ] Confirm no credential, signing key, personal path, user data, production configuration, or
  internal endpoint is present.
- [ ] Confirm GitHub security reporting, CODEOWNERS, and security contact are reachable.
- [ ] Inspect the built App's Info.plist, entitlements and Tauri capabilities. Bonjour/Local Network
  declarations must be accurate and minimal; WebView windows must not receive raw socket, endpoint,
  trust-anchor or Keychain capability.
- [ ] Confirm synthetic test keys/device labels/endpoints only. Do not attach real Keychain exports,
  peer fingerprints, private network captures or clipboard text to release evidence.

## Clean checkout gate

- [ ] Use a clean macOS 13+ build machine, or free at least 10 GiB beyond existing build output
  before creating a second Tauri target directory.
- [ ] Clone the frozen release-candidate commit into a new directory with no local `.env`,
  application database, signing material, or untracked source files.
- [ ] Run `pnpm install --frozen-lockfile`, `pnpm desktop:dev`, `pnpm release:verify`, and
  `pnpm release:bundle:unsigned` to completion in that checkout.
- [ ] Record the Node, pnpm, Rust, Xcode, macOS, architecture, test, bundle path, and SHA-256
  evidence in `docs/audit/clean-environment-test.md`.
- [ ] Install and smoke-test the unsigned local App before beginning signing. A successful source
  build does not replace this step.

## Build and compatibility

- [ ] Build the intended macOS artifacts from a clean checkout.
- [ ] Run `scripts/verify-macos-compatibility.sh` and confirm both Universal slices and the
  `LSMinimumSystemVersion`/Mach-O deployment target match the supported matrix.
- [ ] Complete Apple signing and notarization with credentials stored outside the repository.
- [ ] Verify signatures and Gatekeeper assessment.
- [ ] Generate SHA-256 checksums for every distributed artifact.
- [ ] Perform clean-machine installation and upgrade verification.
- [ ] Complete the applicable rows in `docs/beta-validation-matrix.md`.
- [ ] Verify clipboard text/image, RTF/HTML/files, Quick Paste restore, tray behavior, launch at
  login, and direct-paste permission allow/deny/revoke behavior.
- [ ] On a clean install and an upgrade with an existing valid blob, verify the app-data/blob/hash
  directories are owner-only `0700` and blobs are `0600`. Confirm the historical representation
  still restores and broader legacy blob permissions are tightened.
- [ ] With clearly synthetic data, replace one blob with changed bytes, a symlink and a
  non-regular object. Each restore/reuse must fail closed without overwriting a target, writing
  bytes to the clipboard, allocating beyond the 32 MiB limit or displaying an absolute app-data
  path. Remove the synthetic fixture after recording the bounded result.
- [ ] Confirm Core and Labs produce no unsolicited network traffic.
- [ ] With Local Link off, confirm no Local Link TCP listener, Bonjour registration, browse traffic
  or session survives startup, upgrade, disable or quit.
- [ ] With Local Link explicitly enabled, verify only the documented Bonjour/mDNS discovery and
  authenticated paired-peer TCP traffic occurs. Disable/reset/quit must remove listener, discovery,
  sessions and pending bodies; no cloud, relay, offline queue or plaintext route may appear.
- [ ] Verify the real macOS Keychain item across first setup, restart, upgrade, reset, reinstall,
  access failure and signing identity. Confirm `AccessibleWhenUnlockedThisDeviceOnly`, no Keychain
  synchronization, fail-closed missing identity with trusted peers, and reset-to-`needsRePairing`.
  Private material must not appear in SQLite, IPC, diagnostics, logs or the source archive.
- [ ] Complete the Local Link rows in `docs/beta-validation-matrix.md` on the exact candidate SHA:
  50 pairing ceremonies, 200 text transfers, at least 50 each Copy/Save/Reject, cancellation,
  expiry, replay/duplicate, revocation, lock/user-switch, network isolation/change, sleep/wake,
  performance, packet inspection and disk/log scans.
- [ ] Confirm transfer summaries retain at most 20 content-free rows for at most 24 hours and cannot
  replay an old payload. Clearing terminal summaries must not change History/trust/pending state.
  Pending bodies clear autonomously within the documented 60-second/lifecycle boundary even when
  Transfers is never opened.
- [ ] Keep Local Link labeled Preview/Alpha and default off unless every Local Link gate passes. A
  browser fixture, loopback, single-Mac test or source review is not Beta evidence.

## Final approval

- [ ] Sealdot404 confirms the target version, `CHANGELOG.md`, README claims, known issues, and
  `.github/RELEASE_TEMPLATE.md` release notes.
- [ ] Sealdot404 records the Local Link Go/No-Go decision. Any incorrect trust, plaintext fallback,
  body persistence, duplicate Copy/Save, post-revocation access or failed transfer reported as
  delivered is an absolute No-Go.
- [ ] Sealdot404 confirms that the final public-risk audit, asset provenance, Git author metadata,
  signing evidence, and compatibility evidence are acceptable for publication.

## Publication

- [ ] Create the annotated Git tag only after the preceding checks pass.
- [ ] Draft release notes from `.github/RELEASE_TEMPLATE.md`; include only verified claims.
- [ ] Upload verified artifacts and their checksums to the GitHub Release.
- [ ] Mark alpha/beta/rc releases as prereleases.
- [ ] Publish only after Sealdot404's final review.
- [ ] Verify public download links, release notes, checksums, and the source archive after
  publication.

## After publication

- [ ] Monitor `sealdot404@gmail.com` and the issue tracker for release regressions.
- [ ] Record installation, privacy, or compatibility defects in the next `Unreleased` changelog
  section.
- [ ] Do not alter the published tag or replace artifacts; issue a new version when correction is
  necessary.
