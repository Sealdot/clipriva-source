# Open-source change rules

**Last reviewed:** 2026-07-26

ClipRiva publishes source while the macOS app remains an engineering candidate. These rules make
the open-source safeguards part of normal development work. The root [`AGENTS.md`](../../AGENTS.md)
contains the same mandatory rules in a form that future Codex tasks can apply automatically.

## Change-trigger matrix

| When a change includes | Do this in the same change |
| --- | --- |
| A Node or Rust dependency, lockfile, or build dependency | Review license/SBOM impact. Update `docs/audit/dependency-inventory.md` when the direct dependency list changes, and refresh `docs/audit/sbom-license-review.md` for the full resolved graph. |
| Network access, account, analytics, error reporting, sync, cloud/model provider, export, stored data type, or permission | Update `PRIVACY.md`, `docs/privacy/data-flow.md`, and `docs/open-source/module-boundary.md`. Add boundary tests and avoid unverified local-only or privacy claims. |
| Icon, logo, screenshot, illustration, font, audio, copied code, or other external material | Confirm redistribution rights before adding it. Record third-party material in `THIRD_PARTY_NOTICES.md`; recheck `TRADEMARK.md` for ClipRiva name or logo use. |
| Environment variable, configuration example, fixture, test data, diagnostics, release workflow, or documentation | Keep it value-free or clearly synthetic. Never commit a real credential, signing certificate, user clipboard content, database, personal local path, test account, or production endpoint. |

## How to handle a suspected secret

Do not paste the candidate value into an issue, commit message, report, or chat. Stop the task,
redact the evidence, and use the private process in [SECURITY.md](../../SECURITY.md). A current-tree
delete does not remove a historical secret; credential rotation and history remediation require the
maintainer's explicit decision.

## What this does not authorize

These rules do not authorize making the GitHub repository public, pushing a branch, creating a tag
or GitHub Release, uploading an installer, changing the license, signing or notarizing an app, or
rewriting history. Those actions remain release-owner decisions.

## Release handoff

When a version is frozen, use [the release checklist](../release/release-checklist.md). It requires a
fresh audit on the final commit, a clean macOS build, signing and notarization evidence, real-machine
compatibility checks, and Sealdot404's final approval.

## Commit-specific validation and Git parity

Do not describe a dirty working-tree test as evidence for a remote commit. The completion sequence
for a task that explicitly authorizes a push is:

1. Keep unrelated local artifacts ignored or outside the repository and split the intended change
   into focused commits.
2. Confirm there are no tracked, staged, or unignored files after the candidate commit.
3. Run the final relevant checks on that clean commit. Generated output must stay in ignored paths.
4. Push that exact commit to its configured upstream branch.
5. Run `scripts/verify-git-sync.sh`. It queries the configured remote and fails if the local commit,
   remote-tracking ref, or actual remote branch SHA differs.

If a task does not authorize a push, stop after the clean local validation and explicitly report the
commit as unpushed. Repository rules and a successful test never authorize publication, a tag, a
release, signing, notarization, or a push by themselves.
