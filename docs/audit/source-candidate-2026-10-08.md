# Initial local source-review candidate — 2026-10-08

> Historical evidence. Current source-publication status and scope are in
> [the public preview record](source-public-preview-2026-10-08.md).

> Historical first local export/check round. The follow-up
> [private runner verification](source-ci-verification-2026-10-08.md) supersedes its local-only
> push status and native/target-license evidence gaps. Preserve this record as evidence for
> the initial root commit, not the final verification commit.

This is a local, reviewable source-publication candidate for `2.0.0-alpha.1`. It is not a
public repository, a tag, a release, an installer or a Local Link approval. Feature work is
frozen. Any subsequent source or documentation change invalidates the candidate evidence
and requires a new commit, scan and archive.

## Export and history boundary

The source baseline is `dcfdb52caa1357e89cc5940adb676fb24bcad866`. The original private
repository, refs and history are retained. Its historical personal-path disclosure is not
reproduced here and is not remediated by rewriting that repository. This export copies only
350 approved blobs from that baseline using the exact paths in
[source-export-allowlist.txt](source-export-allowlist.txt). It starts a separate Git root
commit with no parent, remote, alternate object store or imported private history. The local
handoff records the final candidate commit and tree; no push or remote parity is claimed.

The original 355-file tracked tree loses only three historical `.docx` planning documents
and two `.vscode` editor settings files. Historical prose references to the omitted Word
inputs are retained; they are not build inputs or promised archive attachments. No `.git`,
ignored or untracked material is copied. Runtime databases/WALs, captured clipboard data,
local blobs, logs, `.env` values, private configuration, dependency installations, caches,
Tauri generated files and compiled/bundled output are outside the export. Synthetic test
fixtures and documented synthetic UI screenshots remain included.

All implementation, manifest, lockfile and asset blobs are byte-identical to the baseline.
Only the English/Chinese READMEs and THIRD_PARTY_NOTICES.md change. This audit, the exact
allow-list and root NOTICE are added. No version or runtime behavior changes. The candidate
commit uses a synthetic review identity; project ownership attribution is retained in LICENSE
and NOTICE.

## License, notices and material provenance

LICENSE matches the official Apache-2.0 text after whitespace normalization and substitution
of the appendix copyright template with the existing `Copyright 2026 Sealdot404`. The
[official terms](https://www.apache.org/licenses/LICENSE-2.0.txt) govern; no new license
interpretation or binary-redistribution approval is asserted. NOTICE preserves attribution
and links to THIRD_PARTY_NOTICES.md and TRADEMARK.md. The existing brand policy is unchanged.

Every included icon/screenshot is unchanged and covered by the existing Sealdot404
self-owned, Codex-assisted, human-reviewed provenance representation. This task adds no
third-party asset and does not independently establish ownership. No installed third-party
code is vendored. Binary distribution must still collect notices/licenses for the exact
components shipped, including relevant MPL/Unicode/multi-license obligations.

The direct inventory and prior [SBOM/license review](sbom-license-review.md) are retained
as dated evidence, not a fresh all-target result. Both lockfiles and both manifests are
unchanged. Fresh portable metadata and its coverage/limitations are supplied in the handoff.
Neither package.json nor Cargo.toml declares a machine-readable project license field;
the root LICENSE establishes the intended source license. Adding package metadata later
would trigger the dependency/SBOM follow-up rules and a new candidate.

## Privacy and security review

PRIVACY.md, docs/privacy/data-flow.md and docs/open-source/module-boundary.md were reviewed
against the unchanged implementation and are retained verbatim. This packaging task adds
no data type, network path, account, telemetry, export capability inside the app, OS permission
or dependency. The source export does not read an app database, Keychain or live clipboard
and no desktop application is launched.

Core/Labs are documented as local; Local Link is the separate default-off network boundary.
Static review and frontend boundary tests do not establish real-machine network behavior.
Default-off automation uses an owner-only local socket. OCR, notes, Collections, diagnostics
and Local Link memory/Keychain constraints remain subject to the documented native and
manual evidence gates. No application-wide “never networks” claim is made.

A Gitleaks 8.30.1 directory scan detects six already-audited synthetic inputs, recognition
rules or a protocol test-plan sentence. Scanner exit 1 is retained and findings are reviewed;
this is not reported as a zero-finding scanner pass. Final tree and one-commit history scans
are recorded outside the source after commit. Absolute home-directory paths in tests use
only the reviewed synthetic personas `example`, `person` and `alice`. Raw detections and
actual historical path values are not included in this report. An unknown secret-like
finding must stop export and follow SECURITY.md.

## Validation and remaining gaps

The accompanying REPRODUCE.md and final validation record identify the exact committed
tree, tool versions, commands, exits and coverage. Final source checks run after the root
commit with no tracked or untracked source changes. Historical reports in docs/audit and
docs/release/evidence refer to their own old commits and must not be promoted to evidence
for this candidate.

Rust formatting can run without building dependencies. All-target offline Cargo metadata
and macOS-filtered offline metadata fail because this host lacks the mdns-sd index/cache.
The separate public-registry metadata attempt and fresh Node review are recorded in the
handoff; missing coverage is not a license or vulnerability pass.

Clippy, Rust tests and a clean native build remain incomplete for this candidate. The
host has less than the release checklist's 10 GiB free-space requirement for a fresh
Tauri build and has Command Line Tools rather than full Xcode. No installer is produced.
The aggregate `pnpm release:verify` therefore remains incomplete. Rust vulnerability
scanning is not complete; source checks are not a security certification.

Local Link real two-Mac pairing/transfer, packet/privacy/lifecycle/Keychain checks, macOS
compatibility/permission smoke tests, signed identity behavior, Developer ID signing,
notarization, Gatekeeper and real-device installation/distribution remain **unverified**.
No old unsigned artifact, browser fixture, loopback result or previous commit's Rust pass
closes these gates. Public hosting/settings, contributor/asset final approval and exact
binary notices remain future publication work under the release checklist.

The deliverable is suitable for local source review, with explicit remaining evidence gaps.
It provides no authorization to publish, push, tag, release, sign or notarize.
