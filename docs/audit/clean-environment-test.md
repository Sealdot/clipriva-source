# Clean-environment verification

> 2026-10-08 source preview: clean runner dependency installation and source tests are distinct
> from application installation. See [the public preview record](source-public-preview-2026-10-08.md)
> and exact Release CI evidence. Native installation and historical binary gates below remain
> unverified; no installer is published.


- **Run date:** 2026-08-09
- **Frozen source candidate:** `cfa2ead91bffb90bcd754e5c6356590b26cf2f0a` (`1.5.0`)
- **Status:** Passed for the requested clean source, browser-start, performance and unsigned-bundle
  scope; not a release approval.
- **Environment:** clean detached Git clone on macOS 14.2.1 (Apple Silicon), Node.js 24.14.0,
  pnpm 11.9.0 through Corepack, Rust 1.97.1 and Apple Command Line Tools. Full Xcode was not
  selected, so signing and notarization were outside this run.

The clone contained only committed files from the frozen candidate. `git status` was clean, no
project `.env` existed, and the clone contained no maintainer planning documents, application data,
signing material or prior build output. The shared volume started with about 9.3 GiB free, below the
checklist's preferred 10 GiB margin, but the build completed without storage exhaustion.

## Candidate results

| Step | Command or action | Result | Notes |
| --- | --- | --- | --- |
| Clone | Clean local clone; detached checkout of the full candidate SHA | Pass | Version check reported `1.5.0` consistently in package, Cargo and Tauri configuration. |
| Install | `pnpm install --frozen-lockfile --registry=https://registry.npmjs.org` | Pass | The host's preconfigured private mirror was unreachable; the explicit public registry completed with 127 packages reused from the verified store and the 193-entry supply-chain policy check passing. |
| Browser development start | `pnpm dev --host 127.0.0.1`, then a local HTTP request | Pass | Vite 8.1.5 became ready and returned the `ClipRiva` HTML title. The process was then stopped normally. |
| Source release gate | `pnpm release:verify` | Pass | Biome, TypeScript and the frontend production build passed; Vitest passed 16 files / 145 tests. Rust formatting, all-target/all-feature Clippy with warnings denied, and tests passed: 193 passed, 0 failed, 1 release benchmark intentionally ignored by the default suite. |
| Release performance test | `cargo test --release quick_paste_searches_10k_items_within_target -- --ignored --nocapture` | Pass | The synthetic 10,000-item Quick Paste search completed in 74.018458 ms, below the 100 ms single-run target. It does not replace multi-run p50/p95 measurement. |
| Unsigned bundle | `pnpm release:bundle:unsigned` | Pass | Repeated the full source gate and built `src-tauri/target/release/bundle/macos/ClipRiva.app`. The bundle reports version/build `1.5.0`, identifier `com.clipriva.desktop`, and a thin arm64 executable. |
| Signature boundary | Strict `codesign` verification and Gatekeeper assessment | Expected rejection | Both returned nonzero. The executable has only an ad-hoc linker signature, no Team ID and no sealed bundle resources; this is not a distributable signed application. |
| Desktop application launch | `pnpm desktop:dev` or opening the unsigned bundle | Not run | No isolated macOS test user was available. Launching the exact candidate would use the maintainer's normal app-data directory, so this run did not risk reading or mutating existing clipboard history. A clean-user install/smoke test remains required. |

## Artifact evidence

- Unsigned test archive: `ClipRiva-1.5.0-cfa2ead-unsigned.zip`, 5,345,284 bytes.
- Archive SHA-256: `f824faed1e306f683ea3b75d007f928f6e79c5c80a87b36f600b0de670a1efc9`.
- arm64 executable SHA-256: `abdd2d4c0e39e9e83525b1351c1b4852eea75ce976a54ed6c8a70c77648c0954`.
- The archive is an ephemeral internal-test handoff, not a release asset. It is unsigned,
  unnotarized and must not be publicly distributed.
- After evidence capture, `git clean -fdx` removed the clone's generated `dist`, `node_modules`,
  Tauri `gen` and 5.4 GiB Rust `target` output. The clean source clone remains only as a 97 MiB
  temporary detached checkout; the archived unsigned handoff was retained separately.

## Candidate conclusion and remaining gates

The requested frozen-version, clean source gate and unsigned-bundle work is complete for the exact
candidate SHA. No product code, dependency, network behavior, stored data type, OS permission or
third-party asset changed in the freeze. The accompanying SBOM/license rerun retained 127 Node and
517 Rust package/version entries with zero missing-metadata or restricted-license findings.

Public release remains **No-Go**. A clean isolated-user desktop launch and install, two-real-Mac
Local Link matrix, privacy marker scans, full Xcode build, Developer ID signing, notarization,
Gatekeeper acceptance, universal binary and supported-macOS compatibility evidence are still open.

## Historical attempt — 2026-07-26

- **Run date:** 2026-07-26
- **Source baseline:** `0b8510b9e311ac25252bcab963b91e26d5f49d49`
- **Status:** Partially passed; not a release approval.
- **Environment:** clean local Git clone on macOS 14.2.1 (Apple Silicon), Xcode Command Line Tools,
  Node.js 24.14.0, pnpm 11.9.0, and Rust 1.97.1.

The clone contained only committed files from the source baseline. It did not include the
maintainer's untracked product-planning documents, local application data, signing credentials, or
existing build output.

### Historical results

| Step | Command or action | Result | Notes |
| --- | --- | --- | --- |
| Clone | `git clone --local --branch codex/clipriva-0.4-revised-reliability --single-branch . <temporary-directory>` | Pass | Checked out the recorded source baseline with a clean status. |
| Read setup instructions | Read `README.md` and `docs/development/environment-variables.md` | Pass | The instructions correctly state that the current application has no project-defined `.env` variables. |
| Install frontend dependencies | `pnpm install --frozen-lockfile --registry=https://registry.npmjs.org`, then `pnpm install --frozen-lockfile --offline` | Pass | The host's preconfigured private package mirror was unreachable; explicitly using the public registry completed the first install, and the offline repeat completed from the resulting store. This is host configuration, not a repository dependency. |
| Start browser development mode | `pnpm dev --host 127.0.0.1` | Pass | Vite started at `http://127.0.0.1:1420/`; a local HTTP request returned the `ClipRiva` HTML title. |
| Frontend validation | `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build` (through `pnpm release:bundle:unsigned`) | Pass | Biome and TypeScript passed; Vitest passed 8 files / 42 tests; Vite produced a production build. |
| Rust formatting and linting | `pnpm release:verify` | Partial | The clean clone reached the Rust validation stages without a source diagnostic, but the end-to-end command could not be allowed to finish before storage exhaustion. |
| Rust tests and unsigned application build | `pnpm release:bundle:unsigned` | Blocked | The temporary clone's Tauri/Rust build artifacts exhausted the shared build volume before `cargo test` and the unsigned `.app` had a final result. No unsigned application was produced. |

### Historical failure and cleanup

At the time of the attempted Tauri build, the shared volume had about 58 MiB free. The temporary
clone grew to about 4.1 GiB while the existing workspace `src-tauri/target/` already used about
3.8 GiB. The clean-clone directory was then deleted; it was disposable verification output and did
not contain source changes. Available space returned to about 4 GiB.

### Historical required re-run before publication

1. Use a clean macOS 14+ build machine or free enough local space for a second Tauri target
   directory; plan for at least 10 GiB free beyond existing build output.
2. Clone the final release commit, not this development baseline.
3. Run `pnpm install --frozen-lockfile`, `pnpm desktop:dev`, `pnpm release:verify`, and
   `pnpm release:bundle:unsigned` to completion.
4. Record the Rust-test result, generated application path, architecture, and SHA-256 checksum.
5. Complete the signed/notarized clean-machine installation steps in
   [the release guide](../release.md) before distributing any official binary.

The source-level checks that passed here are useful regression evidence, but they do not replace a
completed unsigned build, Apple signing, notarization, Gatekeeper assessment, or the compatibility
matrix.
