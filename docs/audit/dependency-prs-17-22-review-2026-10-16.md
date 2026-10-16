# Dependency PRs #17–#22 review

- Review date: 2026-10-16; public repository: `Sealdot/clipriva-source`.
- Baseline: `ce4478bbebb4e73ae57e158c7bebcce8da54b7bf` (`main`).
- Combined dependency commit: `0856c02060ee7a4f37b73dfbb7f31544f90ad847`.
  The follow-up adds only inventory, notices and audit evidence. Lockfile hashes in the
  component reports bind that evidence to the reviewed dependency graph.
- Tools: Node 24.15.0, pnpm 11.9.0, Rust/Cargo 1.97.1 on macOS ARM64.

## Reviewed changes

| PR | Update | Reviewed head | Existing GitHub checks |
| --- | --- | --- | --- |
| [#17](https://github.com/Sealdot/clipriva-source/pull/17) | upload-artifact 4.6.2 → 7.0.2 | `468f768fe67e9718f2107a2a870b4ce22e0c9f0b` | CI passed |
| [#18](https://github.com/Sealdot/clipriva-source/pull/18) | jest-dom 7.0.0 → 7.0.1 | `8b0f49e9877df958a684501a81cf1ea6b7894832` | CI and five compatibility jobs passed |
| [#19](https://github.com/Sealdot/clipriva-source/pull/19) | @types/node 26.1.1 → 26.6.5 | `940b49648611b7f3770843fc2814bd06331891fe` | CI and five compatibility jobs passed |
| [#20](https://github.com/Sealdot/clipriva-source/pull/20) | clipboard-manager JS 2.3.2 → 2.4.2 | `808a7fbea25d251cdf34233c3325203ba302cd82` | CI and five compatibility jobs passed |
| [#21](https://github.com/Sealdot/clipriva-source/pull/21) | lucide-react 1.26.0 → 1.55.0 | `38edb185244e9ed7ceffcf01d2ee567ed924e41e` | CI and five compatibility jobs passed |
| [#22](https://github.com/Sealdot/clipriva-source/pull/22) | Vitest 5.0.3 → 5.0.4 | `610e828505d9dd4f65208b11dafd152ec92e22e5` | CI and five compatibility jobs passed |

All six heads are ancestors of the combined local branch. Existing per-PR CI checks exercised
GitHub's individual synthetic merge commits; they do not establish native compatibility of
this new combined commit. No remote branch, PR state, release or tag was changed in this review.

## Combination and compatibility

The adjacent application dependency entries conflict when combining #20 and #21. Preserve
both upgrades. The Vitest importer conflicts when combining #19 and #22; additionally,
Git's automatic merge leaves jest-dom's new optional Vitest peer pointing at the old test
runner and Node types. Resolve the lockfile to one Vitest 5.0.4 / @types/node 26.6.5 graph,
then regenerate peer snapshots with the pinned pnpm. Frozen installation succeeds.
No application source, tests, test exclusions or assertions change.

The combined lockfile adds/updates 12 npm package versions and removes 10 superseded versions.
These include @tauri-apps/api 2.12.3 as the clipboard binding's transitive dependency and
@babel/runtime 7.29.10 through Testing Library. The project's direct @tauri-apps/api remains
2.11.1. All changes are listed in the linked registry comparison. Rust's lockfile is unchanged.

The app does not import the JS clipboard-manager package; native clipboard operations remain
on the unchanged Rust plugin 2.3.2 and existing command/capability boundary. No native plugin
upgrade, permission or data-flow change is introduced by #20. Lucide remains compatible with
the existing imports, React 19 and TypeScript checks. Keep the complete ISC and Feather MIT
notices described in THIRD_PARTY_NOTICES.md.

For #17, the official v7.0.2 tag resolves to the committed SHA. Its action metadata uses Node 24
and defaults to zipped archives and excluded hidden files. Existing workflow inputs, read-only
permissions, evidence directory and 30-day retention remain unchanged. The repository uses
GitHub.com hosted macOS runners. The successful [PR #17 CI run](https://github.com/Sealdot/clipriva-source/actions/runs/38808335317)
uploaded an artifact that was downloaded and inspected during this review; its evidence commit
matches PR #17's synthetic merge `8e87c4b76bb1967e905132c60c948ccbe83d2e0d`. No action source
is vendored or shipped in the app.

## Fresh license and advisory evidence

| Scope | Components | Missing | Restricted without a permissive alternative | Unsupported |
| --- | ---: | ---: | ---: | ---: |
| Installed macOS ARM64 Node packages, including development/test | 117 | 0 | 0 | 0 |
| Reachable all-feature macOS ARM64 Rust registry packages | 317 | 0 | 0 | 0 |
| Reachable all-feature all-platform Rust registry packages | 518 | 0 | 0 | 0 |

All 12 added/updated npm versions match official registry integrity values. Their package
metadata declares MIT, ISC or Apache-2.0/MIT alternatives. The installed Lucide LICENSE also
retains MIT attribution for Feather-derived icons, beyond its ISC metadata declaration.
Both all-platform r-efi entries retain permissive alternatives to LGPL. Node coverage is the
installed host graph; it is not a new review of every unchanged optional platform package.
These declarations do not replace a binary-specific notice review.

Fresh full and production-only npm audits both report zero vulnerabilities in every severity.
Rust code and lockfile are unchanged. RustSec was not rerun locally; the inspected successful
PR #17 CI evidence reports zero vulnerabilities and seven reviewed informational warnings
(six maintenance warnings and one glib unsoundness warning outside the macOS target graph),
with no ignored advisories. That is historical evidence for its exact commit and database,
not a fresh RustSec result for this combined candidate.

- [macOS component report and lockfile hashes](dependency-prs-17-22-evidence/macos-arm64.json).
- [All-platform Rust component report](dependency-prs-17-22-evidence/all-platforms.json).
- [Changed npm versions and registry integrity comparison](dependency-prs-17-22-evidence/changed-npm-packages.json).
- [Fresh npm advisory summary](dependency-prs-17-22-evidence/npm-audit-summary.json).

## Local validation and remaining gates

The combined dependency commit passes frozen installation, Biome lint, TypeScript, all
26 frontend test files / 237 tests, production build, Rust formatting and diff whitespace
checks. The existing frontend chunk-size warning (>500 kB) remains. Both Cargo metadata
commands completed successfully against the locked, unchanged Rust graph. Repeat relevant
checks after the audit documentation commit with a clean tracked/untracked source tree;
record that final SHA and results in the handoff.

Local Rust compilation/tests, ignored performance benchmarks, a combined GitHub CI run,
real-device native UI/clipboard checks, the OS/architecture matrix, two-Mac Local Link,
signing, notarization and binary notices were not run for this combined candidate. Individual
PR CI successes remain scoped to their own synthetic merge commits. A future authorized
integration must run CI on the combined tree before merge; release gates remain mandatory.

## Other pending work observed

There are no open Issues. Nine other PRs remain outside the screenshot's six-update batch:
#15 (bilingual README) and #16 (setup-node) have passing checks; #3, #4, #5, #6, #8 and #10
have cancelled/incomplete checks, with failures also recorded on #5. #7 (sha2 0.11.0) has a
real compile failure: digest output no longer implements the LowerHex formatting used by
the app. It needs a separate compatibility fix and fresh checks. No older PR was merged,
closed, rebased, rerun or approved as part of this batch.

## Privacy, security and license impact

No application runtime code, stored data, OS permission, network destination, account,
telemetry, synchronization or model boundary changes. CI's existing evidence upload remains
bounded to the same paths and retention; no product telemetry or data export is added.
Reports contain portable package metadata, public upstream URLs and lockfile hashes, without
personal paths, credentials or user clipboard data. No new standalone artwork or brand change.
The updated permissively licensed icon dependency retains both upstream notice obligations.
No security or native-release readiness is inferred from license metadata or frontend tests.
