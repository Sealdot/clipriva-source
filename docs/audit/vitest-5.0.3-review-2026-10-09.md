# Vitest 5.0.3 dependency and migration review

- Review date: 2026-10-09.
- Change: [PR #9](https://github.com/Sealdot/clipriva-source/pull/9), Vitest 4.1.10 -> 5.0.3.
- Baseline: `a6c7c852508f306951ffc212ecad002d66b485eb` after PRs #12, #14 and #13.
  Main was merged without conflicts; the candidate retains those dependency updates and audit records.
- Dependency source commit: `f4be0a03897f741bae075c5c831e9dbd411a166d`. The follow-up adds audit documentation only.
  Lockfile hashes below bind the evidence to the dependency graph.
- Tools: Node.js 24.14.0, pnpm 11.9.0 and Rust/Cargo 1.97.1; local macOS ARM64 host.

## Dependency changes

Vitest is a development/test dependency. Its updated tool graph adds or updates 14 locked
package/version entries and removes 24 superseded entries. This includes @vitest/mocker and
@vitest/spy 5.0.3, Chai 6.3.0, tinybench 6.2.0 and why-is-node-running 3.2.1. Every changed
entry is listed in registry evidence. No direct application dependency, application source,
Rust dependency or test source changes are included.

The installed package requires Node `^22.12.0 || ^24.0.0 || >=26.0.0` and Vite
`^6.4.0 || ^7.0.0 || ^8.0.0`. Node 24.14.0 and Vite 8.3.2 satisfy these ranges.
The project already declares Vite directly. JSDOM remains 29.1.1, React Testing Library
16.3.3 and jest-dom 7.0.0; browser, UI and coverage add-on packages are not installed.

## Major-version migration review

Reviewed the [official Vitest 5 migration guide](https://vitest.dev/guide/migration/)
against `vite.config.ts`, `src/test/setup.ts` and the existing test sources:

| Migration area | Project evidence and decision |
| --- | --- |
| `clearMocks` now defaults to true | Accept the new default. The setup installs a clipboard mock implementation; tests assert calls made during the current test and already clear/reset relevant mocks. Do not set `clearMocks: false` to preserve cross-test history. |
| Hoisted mocks must be at module scope | The API and performance-test files place `vi.hoisted` and `vi.mock` at module scope. No nested hoisted calls were found. |
| Async assertions must be awaited | Existing `.resolves` and `.rejects` assertions are awaited. The final full suite must still pass under the stricter runtime checks. |
| Matcher and fake-timer changes | The timer test restores real timers after each test. No Temporal usage, custom matcher augmentation or empty-string `toThrow` assertion was found. Existing jest-dom assertions remain part of TypeScript checking and the full suite. |
| Removed/changed APIs | No `test.sequential`, Vitest `bench` API, `expect.poll`, custom reporter, test-name CLI filter or inline-project configuration was found. Native Rust release benchmarks are separate Cargo commands. |
| Browser/coverage behavior | The project uses the JSDOM environment. Vitest browser/UI modes and coverage threshold configuration are not used, so this review does not claim to test their migrations. |

No compatibility shim, disabled assertion, excluded test or configuration change is added.
Run all 237 existing tests in 26 files on the final clean candidate. Matching the previously
validated baseline count does not establish coverage of every possible Vitest feature;
it verifies the project's current suite alongside the source-level migration review.

## License coverage

| Input | Components | Missing licenses | Restricted without a permissive alternative | Unsupported expressions |
| --- | ---: | ---: | ---: | ---: |
| Installed macOS ARM64 Node metadata, including build/test dependencies | 119 | 0 | 0 | 0 |
| Reachable all-feature macOS ARM64 Rust registry graph | 317 | 0 | 0 | 0 |
| Reachable all-feature all-platform Rust registry graph | 518 | 0 | 0 | 0 |

These are fresh reruns for the candidate. All 14 new/updated npm versions declare MIT, and
their public registry integrity values match the lockfile. The Rust lockfile is unchanged;
its two all-platform `r-efi` entries retain MIT/Apache-2.0 alternatives to LGPL. Reports retain
original expressions and legacy slash normalizations. Installed Node metadata does not cover
every unchanged optional package on other platforms. Metadata declarations do not establish
artifact-specific NOTICE or redistribution readiness.

## Security findings: improvement with residual test-dependency risk

| Audit scope | Low | Moderate | High | Critical |
| --- | ---: | ---: | ---: | ---: |
| Main baseline, all Node dependencies | 3 | 11 | 4 | 0 |
| This candidate, all Node dependencies | 3 | 9 | 4 | 0 |
| This candidate, production dependencies only | 0 | 0 | 0 | 0 |

The update removes the two old Vitest/@vitest/mocker entries for the
[redirect-mock file-read advisory](https://github.com/advisories/GHSA-82fw-gwwq-j7x9).
The advisory's fixed 5.x line includes 5.0.3. No new affected package/version/advisory tuple
appears relative to main. The all-dependency audit still exits nonzero with 16 package-advisory
entries; it must not be reported as passing. All retained affected versions are development/test
dependencies: undici 7.28.0 via JSDOM (also Vitest's JSDOM peer), and source-map-js 1.2.1 via
JSDOM/css-tree. Four high severity entries remain. Their advisory URLs, paths and patch ranges
are preserved in the portable report; the Vite path already resolves source-map-js 1.2.2.

No ignore list was added. Review compatible undici and source-map-js updates in a focused
follow-up; this major test-runner PR does not silently upgrade JSDOM or claim the retained
findings are harmless. The production-only audit is a separate passing scope. RustSec results
on the unchanged Rust graph remain separate final-candidate CI evidence, including existing
reviewed advisory warnings.

## Inspectable evidence and reproduction

- [macOS ARM64 component licenses](vitest-5.0.3-evidence/macos-arm64.json).
- [All-platform Rust component licenses](vitest-5.0.3-evidence/all-platforms.json).
- [Changed/removed npm versions, declarations and integrity checks](vitest-5.0.3-evidence/changed-npm-packages.json).
- [Candidate and baseline npm security findings](vitest-5.0.3-evidence/npm-security-findings.json).

```text
pnpm-lock.yaml: 742a3b8b1004aa2097211dffa29e7cce35e0f63baf15d53aae3488b65bdc413a
src-tauri/Cargo.lock: 3b46a8006f79e76aa99ed7baf1ac27d714ce56154f2055071513dbeb94d58a28
```

From the source root, use the pinned versions and keep raw metadata outside the repository:

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
pnpm check
node scripts/check-release-version.mjs
cargo fmt --manifest-path src-tauri/Cargo.toml --check
pnpm audit --prod --registry=https://registry.npmjs.org --json
# Expected to fail while the recorded test-dependency findings remain:
pnpm audit --registry=https://registry.npmjs.org --json
```

Audit the baseline manifest/lockfile in a separate temporary directory to reproduce the
security comparison. Fetch each changed npm entry's recorded registry URL and compare
name, version, license and `dist.integrity` with the lockfile. Raw metadata and personal paths
are excluded from committed evidence. The final [PR checks](https://github.com/Sealdot/clipriva-source/pull/9/checks)
and `source-evidence-<commit>` artifact supply independent final-candidate CI evidence.
The PR description/final review identify the exact tested head and completed runs.

## Validation boundaries and follow-up

Run lint, TypeScript, all existing frontend tests, production build, version agreement and Rust
formatting on the final clean commit. Final CI covers native lint/tests, release benchmarks,
RustSec review and five macOS build combinations. Local native compilation is omitted to limit
disk use. Existing frontend chunk-size warnings remain separate build output.

This changes test tooling and its defaults, with no application network route, telemetry,
stored data type or OS permission changes. Passing tests/builds do not prove every WebKit
rendering behavior, real-device runtime behavior, signed binaries or artifact-specific notices.
Those release gates and the remaining JSDOM dependency advisories need separate follow-up.
