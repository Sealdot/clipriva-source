# JSDOM 30.1.2 dependency and runtime review

- Review date: 2026-10-09.
- Change: [PR #11](https://github.com/Sealdot/clipriva-source/pull/11), JSDOM 29.1.1 -> 30.1.2.
- Baseline: `358a8109058ab68fda1ea7baf2a78bf8ab400501` after PR #9 and the prior dependency reviews.
  The dependency branch already contains this main baseline.
- Dependency/runtime source commit: `3516947b9a3b4f9755f825d7ed021a9edacee77c`.
  The follow-up adds audit documentation only; lockfile hashes bind the reports to the graph.
- Tools: Node.js 24.15.0, pnpm 11.9.0 and Rust/Cargo 1.97.1; local macOS ARM64 host.

## Required runtime correction

The [JSDOM 30 release](https://github.com/jsdom/jsdom/releases/tag/v30.0.0) raises Node
requirements to `^22.22.2 || ^24.15.0 || >=26.0.0`; the registry metadata for 30.1.2
retains that requirement. The prior `.nvmrc` pin, 24.14.0, is outside this range.
Earlier green CI on that runtime does not establish supported compatibility.

This PR pins `.nvmrc` to 24.15.0 and changes the project's declared range to
`^24.15.0 || >=26.0.0`, keeping the existing Node 24 development line while excluding
versions unsupported by JSDOM/Vitest. Both workflows already load `.nvmrc`. Active English
and Chinese setup instructions and `CONTRIBUTING.md` are updated; historical audit records
retain their original tool versions and results.

`engineStrict: true` in `pnpm-workspace.yaml` makes unsupported Node environments fail
installation. This is the configuration location read by pnpm 11; see the
[pnpm settings reference](https://pnpm.io/settings) and [engineStrict reference](https://pnpm.io/settings/cli#enginestrict).
The existing workspace selection and build-script allowlist are retained. A negative install
probe on Node 24.14.0 returns `ERR_PNPM_UNSUPPORTED_ENGINE`; a fresh frozen-lockfile install
on Node 24.15.0 succeeds. The local test runtime archive matches the official Node checksum
and stays outside the repository. No global runtime installation is changed by this review.
Only the pinned Node version is validated here; allowed future Node versions are not claimed
to have been tested.

## Dependency changes and test compatibility

The lockfile adds or updates 22 package/version entries and removes 26 superseded entries.
The graph includes undici 8.11.2, updated CSS selector/color tooling, URL/encoding packages
and JSDOM 30.1.2. The old source-map-js 1.2.1 entry is removed; paths now use the already
locked 1.2.2 version. Vitest remains 5.0.3, React Testing Library 16.3.3 and jest-dom 7.0.0.
No application or test source, test exclusions, assertion relaxations or Rust dependencies
are changed.

Reviewed [30.1.2 release behavior](https://github.com/jsdom/jsdom/releases/tag/v30.1.2),
including DOM/CSS performance and focus/style invalidation fixes. This project uses the
standard Vitest JSDOM environment and a clipboard mock in `src/test/setup.ts`; it defines
no custom JSDOM instance, ResourceLoader, VirtualConsole or runScripts configuration.
Existing component tests exercise focus restoration, selectors and DOM assertions; storage
tests exercise localStorage. Run the full 237-test, 26-file suite on the final supported
runtime, together with TypeScript and build checks. Passing the current suite does not
establish every JSDOM API or real WebKit behavior.

## License coverage

| Input | Components | Missing licenses | Restricted without a permissive alternative | Unsupported expressions |
| --- | ---: | ---: | ---: | ---: |
| Installed macOS ARM64 Node metadata, including build/test dependencies | 115 | 0 | 0 | 0 |
| Reachable all-feature macOS ARM64 Rust registry graph | 317 | 0 | 0 | 0 |
| Reachable all-feature all-platform Rust registry graph | 518 | 0 | 0 | 0 |

These are fresh reruns. All 22 changed npm versions declare MIT, MIT-0, BSD-2-Clause or
BlueOak-1.0.0 and match public registry integrity values. The Rust lockfile is unchanged;
both all-platform `r-efi` entries retain MIT/Apache-2.0 alternatives to LGPL. Reports
preserve original expressions and legacy slash normalizations. Installed Node metadata
does not cover every unchanged optional package on other platforms. License declarations
are not artifact-specific NOTICE or redistribution evidence.

## npm security results

| Audit scope | Low | Moderate | High | Critical |
| --- | ---: | ---: | ---: | ---: |
| Main baseline, all Node dependencies | 3 | 9 | 4 | 0 |
| This candidate, all Node dependencies | 0 | 0 | 0 | 0 |
| This candidate, production dependencies only | 0 | 0 | 0 | 0 |

The upgrade removes all 16 baseline package-advisory entries, including the four high severity
entries. The affected baseline versions were undici 7.28.0 and source-map-js 1.2.1, reached
through JSDOM and Vitest's JSDOM peer. The baseline findings, advisory URLs, paths, patched
ranges and removed tuples are preserved in the portable report. No new affected tuple appears,
and no ignore list was added. Both full and production-only npm audits now pass.

This is the registry advisory result for these lockfiles at review time, not a guarantee of
absence of unknown vulnerabilities. RustSec review is separate final-candidate CI evidence
on the unchanged Rust graph, including the existing reviewed maintenance/platform warnings.

## Inspectable evidence and reproduction

- [macOS ARM64 component licenses](jsdom-30.1.2-evidence/macos-arm64.json).
- [All-platform Rust component licenses](jsdom-30.1.2-evidence/all-platforms.json).
- [Changed/removed npm versions, declarations and integrity checks](jsdom-30.1.2-evidence/changed-npm-packages.json).
- [Candidate and baseline npm security findings](jsdom-30.1.2-evidence/npm-security-findings.json).
- [Runtime requirements, install probes and local Node archive provenance](jsdom-30.1.2-evidence/runtime-validation.json).

```text
pnpm-lock.yaml: e56cbfef412d37d879d5672d4433b707958456b40872632c740eccbd384b555c
src-tauri/Cargo.lock: 3b46a8006f79e76aa99ed7baf1ac27d714ce56154f2055071513dbeb94d58a28
```

Use the pinned runtime and package manager, and keep raw metadata outside the repository:

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
pnpm audit --registry=https://registry.npmjs.org --json
```

Audit the baseline manifest/lockfile in a separate temporary directory for the comparison.
Fetch each changed npm entry's recorded source URL and compare its license and integrity
with the lockfile. For the negative runtime probe, select Node 24.14.0 and expect
`pnpm install --offline --frozen-lockfile --ignore-scripts` to fail with the recorded engine
error. Restore Node 24.15.0 for all passing checks. Reports exclude personal paths and raw
local metadata. The final [PR checks](https://github.com/Sealdot/clipriva-source/pull/11/checks)
and `source-evidence-<commit>` artifact supply independent final-candidate evidence;
the PR description/final review record the tested head and completed runs.

## Validation boundaries

Final clean-commit checks include the install guard, supported install, lint, TypeScript,
all frontend tests, production build, version agreement, Rust formatting and fresh license
evidence comparison. CI supplies native Rust lint/tests, release benchmarks, RustSec review
and five macOS build combinations. Local native compilation is omitted to limit disk use.
The existing frontend chunk-size warning remains separate build output.

This updates a test environment and the development runtime contract. It changes no
application network route, telemetry, stored data type or OS permission. Real-device/WebKit
behavior, artifact-specific notices, signing and notarization remain separate release gates.
