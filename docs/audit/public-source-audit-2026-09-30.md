# Public source audit — 2026-09-30

> Historical evidence. Current source-publication status and scope are in
> [the public preview record](source-public-preview-2026-10-08.md).


This review is for making the GitHub source repository visible. It does not approve an installer,
GitHub Release, tag, signed artifact, or promotion of Local Link beyond Preview/Alpha. The
[macOS release checklist](../release/release-checklist.md) still governs binary distribution.

## Source and history

- The reviewed tree includes the `2.0.0-alpha.1` Smart Workflows changes merged with the current
  `main` lineage. The final candidate must be scanned again after its documentation commit.
- `gitleaks` 8.30.1 scanned every locally fetched ref, including Git history and remote branches.
  Six detections were reviewed without copying candidate values into this report: five are
  synthetic test inputs or sensitive-content policy rules; one is a protocol test-plan sentence.
  No usable credential was identified.
- All 57 available GitHub Actions runs were downloaded to a temporary location outside the
  repository (174 text logs). A redacted `gitleaks` directory scan found zero detections.
- One historical planning document contained a real contributor home-directory path. The current
  tree now describes the uncommitted input generically. The old path remains in reachable Git
  history, so visibility requires a maintainer decision to accept that disclosure or authorize a
  coordinated history rewrite. Do not copy the path into issues, comments, or audit output.
- The tracked tree has no environment values, signing material, application database, clipboard
  export, or local diagnostic dump. The named project/security contact is intentional.

## Dependency, asset, and product claims

- The manifests and lockfiles were unchanged in this source-publication change. The existing
  [SBOM/license review](sbom-license-review.md) covers the locked dependencies; the
  [third-party notices](../../THIRD_PARTY_NOTICES.md) now record the added synthetic screenshots.
  Recheck bundled notice obligations for the exact binary artifact.
- A production Node audit using the public npm registry returned zero advisories on this date.
  The release-specific Rust vulnerability and signed-artifact reviews remain open.
- `pnpm check` passed (26 test files, 237 tests). `cargo fmt --check` passed. Rust Clippy was
  attempted but the host ran out of disk space while compiling dependencies; no Rust lint or test
  success is claimed for this source-publication change.
- The English and Chinese READMEs state that source is an alpha engineering candidate and that
  there is no endorsed download. The screenshot is a synthetic browser fixture. Local Link remains
  default off and is not validated for two physical Macs.
- This task changes documentation and GitHub metadata only. It does not add application data,
  permissions, telemetry, network destinations, or third-party runtime dependencies.

## GitHub publication checks

- Before the visibility change: decide how to handle the historical home-directory path, verify
  the exact pushed commit and all remote refs, and re-run the secret scan on that candidate.
- An accurate description and topics have been added. At publication: enable available security
  reporting and dependency alerts, and verify the public README, license, issue templates, and
  security contact.
- After publication: confirm the public URL and settings from an unauthenticated request. Keep
  unsigned local bundles and unverified Local Link behavior out of GitHub Releases.
