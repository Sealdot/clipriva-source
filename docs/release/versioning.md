# Versioning policy

- **Status:** applies to the source-visible `2.0.0-alpha.1` candidate and future releases.
- **Current repository version:** `2.0.0-alpha.1`, a Smart Workflows engineering candidate with
  no public Git tag or endorsed macOS installer.

ClipRiva uses Semantic Versioning in the form `MAJOR.MINOR.PATCH`, optionally followed by a
prerelease identifier such as `-beta.1`. Versions in `package.json`, `src-tauri/Cargo.toml`, and
`src-tauri/tauri.conf.json` and the workspace package in `src-tauri/Cargo.lock` must always match;
`pnpm release:verify` enforces this.

## Version meaning

| Change | Version action | ClipRiva examples |
| --- | --- | --- |
| Backward-compatible bug fix | Increment `PATCH` | Correct a data-recovery bug without changing documented behavior. |
| Backward-compatible feature | Increment `MINOR` | Add an optional Core capability without changing current storage or permission contracts. |
| Incompatible public contract | Increment `MAJOR` after 1.0 | Change a documented data format, supported platform promise, or public integration contract incompatibly. |
| Pre-release validation | Append prerelease identifier | `0.1.0-alpha.1`, `0.1.0-beta.1`, `0.1.0-rc.1`. |
| Urgent production correction | Increment `PATCH` | `1.0.0` to `1.0.1`; use a release note with the security/reliability impact. |

Before 1.0, ClipRiva may make breaking changes, but they must still be recorded in the changelog,
release notes, and migration/recovery guidance when local data or user workflows are affected.

## Release channels

| Channel | Purpose | Requirements |
| --- | --- | --- |
| `alpha` | Engineering and maintainer validation | Public source is allowed; do not represent it as a stable or supported app release. |
| `beta` | Controlled external validation | Complete privacy/security review for the tested scope, documented known issues, and source or explicitly labeled unsigned distribution. |
| `rc` | Candidate for the next stable release | All planned release gates complete except final publication sign-off. |
| Stable | General supported release | Signed/notarized macOS distribution, compatibility evidence, release notes, checksums, and support channel. |

## Release procedure

1. Choose the version from the changes actually included; do not reserve a version for an unfinished
   feature.
2. Update all three version sources in one focused commit.
3. Move confirmed entries from `CHANGELOG.md` `Unreleased` into the versioned section.
4. Run `pnpm release:verify`, then complete the full
   [release checklist](release-checklist.md).
5. Create an annotated Git tag in the form `vMAJOR.MINOR.PATCH[-prerelease.N]` only after all
   required checks pass.
6. Create the GitHub Release from that tag, use
   [the release template](../../.github/RELEASE_TEMPLATE.md), and attach only verified artifacts.

Once a version has been published, do not modify its tag, source contents, checksums, or release
assets in place. Publish a corrective version instead.
