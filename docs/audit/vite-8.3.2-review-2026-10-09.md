# Vite 8.3.2 dependency review

- Review date: 2026-10-09.
- Change: [PR #14](https://github.com/Sealdot/clipriva-source/pull/14), Vite 8.1.5 -> 8.3.2.
- Baseline: `b68e14e485809eb53647a9091aa2ae0ffd10e603` after PR #12. Main was merged without conflicts so this candidate
  retains Tauri CLI 2.12.1 and the prior audit records.
- Dependency source commit: `959cb915009b1c1fd52c33b6d310b09a9df352c1`. The follow-up adds audit documentation only.
  Lockfile hashes below bind the recorded evidence to the dependency graph.
- Tools: Node.js 24.14.0, pnpm 11.9.0 and Rust/Cargo 1.97.1; local macOS ARM64 host.

## Changes and compatibility

The direct upgrade also resolves Rolldown 1.2.12, Oxc types 0.152.0, PostCSS 8.5.28,
nanoid 3.3.19, picomatch 4.0.7 and source-map-js 1.2.2 on the Vite build path. The lockfile
adds or updates 22 package/version entries and removes 26 superseded entries, including the
old optional Rolldown WASM fallback graph. The changed platform variants are listed in the
registry evidence. Other locked source-map-js 1.2.1 entries remain on the separate JSDOM path.

Reviewed the [upstream 8.1.5-to-8.3.2 changelog](https://github.com/vitejs/vite/blob/v8.3.2/packages/vite/CHANGELOG.md).
It includes asset handling, CSS, development-server and Rolldown changes. Vite 8.3.2 supports
Node `^20.19.0 || >=22.12.0`; the pinned Node 24.14.0 satisfies that requirement.
`@vitejs/plugin-react` 6.0.4 accepts Vite `^8.0.0`; Vitest 4.1.10 accepts Vite 6, 7 or 8.
The optional `@vitejs/devtools` peer range changes to `^0.7.1`; that package is not installed
by this project. No application source or Vite configuration change is included: the existing
Safari 13 target, Oxc minifier and loopback development-server binding remain in place.
Final frontend tests/build and native CI results must validate this exact candidate.

## License coverage

| Input | Components | Missing licenses | Restricted without a permissive alternative | Unsupported expressions |
| --- | ---: | ---: | ---: | ---: |
| Installed macOS ARM64 Node metadata, including build/test dependencies | 129 | 0 | 0 | 0 |
| Reachable all-feature macOS ARM64 Rust registry graph | 317 | 0 | 0 | 0 |
| Reachable all-feature all-platform Rust registry graph | 518 | 0 | 0 | 0 |

These are fresh reruns. Every one of the 22 added/updated locked npm package versions was
also queried from the public npm registry; declarations are MIT or BSD-3-Clause, and all
registry integrity values match the lockfile. This separately covers the changed optional
platform packages, while the installed Node report does not claim to cover every unchanged
optional Node platform graph. The Rust lockfile is unchanged. Its two all-platform `r-efi`
entries retain MIT/Apache-2.0 alternatives to LGPL; the reports preserve original expressions
and legacy slash normalizations. License declarations do not establish binary NOTICE coverage.

## npm security findings: partial improvement, not a full audit pass

| Audit scope | Low | Moderate | High | Critical |
| --- | ---: | ---: | ---: | ---: |
| Main baseline, all Node dependencies | 3 | 12 | 5 | 0 |
| This Vite candidate, all Node dependencies | 3 | 11 | 4 | 0 |
| This candidate, production dependencies only | 0 | 0 | 0 | 0 |

The Vite upgrade removes [PostCSS's source-map file-read advisory](https://github.com/advisories/GHSA-fxqj-rqcc-2cmp)
and [nanoid's zero-size generator advisory](https://github.com/advisories/GHSA-2v37-7h3g-55p8).
No new affected package/version/advisory tuple was introduced relative to main. The all-dependency
audit still exits nonzero with 18 package-advisory entries; it must not be reported as passing.
The residual affected versions all existed in the baseline and are development/test dependencies:

| Retained dependency | Path/scope | Remediation reference |
| --- | --- | --- |
| undici 7.28.0 | JSDOM HTTP/WebSocket dependency, also reached through Vitest's JSDOM peer | Upgrade to a compatible 7.29.1 or later patch; see the individual advisories in the portable report |
| Vitest and @vitest/mocker 4.1.10 | Existing test runner/mocking graph | [4.1.11 fixes the redirect-mock file-read advisory](https://github.com/advisories/GHSA-82fw-gwwq-j7x9) |
| source-map-js 1.2.1 | Existing JSDOM/css-tree graph; Vite's separate path now resolves 1.2.2 | [1.2.2 fixes the indexed source-map denial of service](https://github.com/advisories/GHSA-68fv-2mgg-jv7q) |

These findings are recorded without an ignore list. They require a focused test-dependency
security follow-up; this Vite PR does not silently upgrade unrelated direct test dependencies
or claim the remaining findings are harmless. Its production-only npm audit is a different,
passing scope. RustSec checks remain separate CI evidence on the unchanged Rust graph.

## Inspectable evidence and reproduction

- [macOS ARM64 component licenses](vite-8.3.2-evidence/macos-arm64.json).
- [All-platform Rust component licenses](vite-8.3.2-evidence/all-platforms.json).
- [Changed npm versions, removed versions, license declarations and integrity checks](vite-8.3.2-evidence/changed-npm-packages.json).
- [Current and baseline npm security findings](vite-8.3.2-evidence/npm-security-findings.json).

```text
pnpm-lock.yaml: e2d64349ba4d5553c6faeed9b96df01cf1f8eae18ff4aabefb1ca2bca23fc03a
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

For the baseline comparison, audit the manifest/lockfile from the baseline commit in a separate
temporary directory. For each changed npm entry, fetch its recorded `source` URL and compare
name, version, license and `dist.integrity` with the lockfile. The committed evidence excludes
raw local Cargo/pnpm metadata and personal paths. The final [PR checks](https://github.com/Sealdot/clipriva-source/pull/14/checks)
and `source-evidence-<commit>` artifact provide independent final-candidate CI evidence.

## Boundaries and remaining work

This change updates development/build tooling. It changes no direct application dependency,
Rust dependency, application network route, telemetry, stored data type or OS permission.
Generated frontend output may change with the updated bundler. The metadata audit, unit tests
and native builds do not prove browser rendering on every supported WebKit version or signed
installer behavior. Binary NOTICE review, signing/notarization and real-machine release gates
remain separate. Remediate the recorded test-dependency vulnerabilities in a focused follow-up.
