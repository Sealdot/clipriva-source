# React Testing Library 16.3.3 dependency review

- Review date: 2026-10-09.
- Change: [PR #13](https://github.com/Sealdot/clipriva-source/pull/13), `@testing-library/react` 16.3.2 -> 16.3.3.
- Baseline: `b37c2255e3fe3c49038478cf54e47c7087fe509f` after PRs #12 and #14. Main was merged without conflicts;
  this candidate retains Tauri CLI 2.12.1, Vite 8.3.2 and their audit records.
- Dependency source commit: `03faa2ca709d6ccf89d2814c5ed80980a091b8df`. The follow-up changes audit documentation only.
  Lockfile hashes below bind the evidence to the dependency graph.
- Tools: Node.js 24.14.0, pnpm 11.9.0 and Rust/Cargo 1.97.1; local macOS ARM64 host.

## Changes and compatibility

The lockfile replaces exactly one package version. No peer or transitive dependency version
changes relative to the baseline. The [upstream release](https://github.com/testing-library/react-testing-library/releases/tag/v16.3.3)
fixes re-entrant `act()` handling when dispatching events and includes comment/type typo fixes.
The package retains Node >=18, React/React DOM 18 or 19, matching React types, and
`@testing-library/dom` ^10 peer requirements. The project's pinned Node 24.14.0, React/React DOM
19.2.8, React types 19.2.17, React DOM types 19.2.3 and DOM testing library 10.4.1 satisfy them.
No application source, runtime dependency or test configuration changes are included.

## License coverage

| Input | Components | Missing licenses | Restricted without a permissive alternative | Unsupported expressions |
| --- | ---: | ---: | ---: | ---: |
| Installed macOS ARM64 Node metadata, including build/test dependencies | 129 | 0 | 0 | 0 |
| Reachable all-feature macOS ARM64 Rust registry graph | 317 | 0 | 0 | 0 |
| Reachable all-feature all-platform Rust registry graph | 518 | 0 | 0 | 0 |

These are fresh reruns for this candidate. The updated npm package declares MIT, and its public
registry integrity matches the locked value. Rust dependencies and their lockfile are unchanged;
the two all-platform `r-efi` entries retain MIT/Apache-2.0 alternatives to LGPL. Reports preserve
original expressions and legacy slash normalizations. Installed Node metadata does not cover
every optional package on other platforms. License declarations do not establish binary NOTICE
coverage or artifact redistribution readiness.

## Security findings

| Audit scope | Low | Moderate | High | Critical |
| --- | ---: | ---: | ---: | ---: |
| Main baseline, all Node dependencies | 3 | 11 | 4 | 0 |
| This candidate, all Node dependencies | 3 | 11 | 4 | 0 |
| This candidate, production dependencies only | 0 | 0 | 0 | 0 |

The all-dependency audit exits nonzero with 18 package-advisory entries. The set of affected
package/version/advisory tuples is identical to the baseline; this update neither introduces
nor fixes a reported vulnerability. All recorded affected versions are development/test
dependencies. The full audit must not be reported as passing. The residual findings are:

| Retained dependency | Path/scope | Follow-up |
| --- | --- | --- |
| undici 7.28.0 | Existing JSDOM HTTP/WebSocket dependency, also reached through Vitest's JSDOM peer | Review a compatible 7.29.1 or later patch; individual advisories are in the portable report |
| Vitest and @vitest/mocker 4.1.10 | Existing test runner/mocking graph | [4.1.11 fixes the redirect-mock file-read advisory](https://github.com/advisories/GHSA-82fw-gwwq-j7x9) |
| source-map-js 1.2.1 | Existing JSDOM/css-tree graph; Vite's separate path resolves 1.2.2 | [1.2.2 fixes the indexed source-map denial of service](https://github.com/advisories/GHSA-68fv-2mgg-jv7q) |

No ignore list was added. Remediation needs a focused test-dependency security follow-up.
The production-only audit is a separate passing scope. RustSec checks on the unchanged Rust
graph remain separate CI evidence, including review of existing advisory warnings.

## Inspectable evidence and reproduction

- [macOS ARM64 component licenses](react-testing-library-16.3.3-evidence/macos-arm64.json).
- [All-platform Rust component licenses](react-testing-library-16.3.3-evidence/all-platforms.json).
- [Changed npm version, peer requirements, license and integrity check](react-testing-library-16.3.3-evidence/changed-npm-packages.json).
- [Current and baseline npm security findings](react-testing-library-16.3.3-evidence/npm-security-findings.json).

```text
pnpm-lock.yaml: 8dc8ca42969c0f432ef656f1cf34168b8e096b19b63c0370d9ff3e5bbcd38dc9
src-tauri/Cargo.lock: 3b46a8006f79e76aa99ed7baf1ac27d714ce56154f2055071513dbeb94d58a28
```

From the source root, use the pinned tool versions and keep raw metadata outside the repository:

```bash
review_dir=$(mktemp -d)
pnpm install --frozen-lockfile
pnpm licenses list --json > "$review_dir/node.json"
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --all-features \
  --filter-platform aarch64-apple-darwin --format-version 1 > "$review_dir/cargo-target.json"
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --all-features \
  --format-version 1 > "$review_dir/cargo-all.json"
python3 scripts/review-dependency-licenses.py --node-metadata "$review_dir/node.json" \
  --cargo-metadata "$review_dir/cargo-target.json" --target aarch64-apple-darwin \
  --commit "$(git rev-parse HEAD)" --output "$review_dir/target"
python3 scripts/review-dependency-licenses.py --node-metadata "$review_dir/node.json" \
  --cargo-metadata "$review_dir/cargo-all.json" --target all-platforms \
  --commit "$(git rev-parse HEAD)" --output "$review_dir/all-platforms"
pnpm audit --prod --registry=https://registry.npmjs.org --json
# Expected to fail while the recorded test-dependency findings remain:
pnpm audit --registry=https://registry.npmjs.org --json
```

Audit the baseline manifest/lockfile in a separate temporary directory to reproduce the
comparison. Fetch the changed npm entry's recorded registry URL to compare name, version,
license and `dist.integrity` with the lockfile. The final [PR checks](https://github.com/Sealdot/clipriva-source/pull/13/checks)
and `source-evidence-<commit>` artifact provide independent final-candidate evidence.
Raw Cargo/pnpm metadata and personal paths are excluded from committed reports.

## Validation and boundaries

Run frontend formatting/lint, TypeScript, all existing frontend tests and production build
on the clean final candidate, plus source-version agreement and Rust formatting. Native Rust
lint/tests, release benchmarks, RustSec review and the five macOS build combinations are
covered by final-candidate CI; local native compilation is omitted to limit disk use.
The PR description and final review record the exact tested head and completed CI runs.

This changes a development/test library. No application networking, telemetry, stored data
type or OS permission changes are included. Existing tests and native builds do not establish
every WebKit rendering behavior, real-device runtime behavior, binary notices, signing or
notarization. Those release gates and the recorded test-dependency vulnerabilities remain
separate follow-up work.
