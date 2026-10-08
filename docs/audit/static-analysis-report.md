# Static-analysis report

- **Assessment date:** 2026-07-26
- **Scope:** TypeScript/React frontend and Rust/Tauri desktop runtime at commit `e850d9a`, with the
  P1 command additions assessed before commit.

## Current checks

| Area | Command | Tooling | Status | Notes |
| --- | --- | --- | --- | --- |
| Frontend formatting and lint | `pnpm lint` | Biome 2.5.5 recommended rules | Pass | Checks tracked frontend/configuration files and avoids generated output. |
| TypeScript | `pnpm typecheck` | TypeScript project references with strict mode | Pass | `tsc -b --pretty false`; `pnpm build` also type-checks before Vite builds. |
| Frontend build | `pnpm build` | TypeScript and Vite | Pass | Produces a production browser bundle without a personal path or environment dependency. |
| Rust formatting | `pnpm lint:rust` | rustfmt | Pass | Uses the pinned Rust toolchain contract and checks without rewriting files. |
| Rust lint and unused-code diagnostics | `pnpm lint:rust` | Clippy with warnings denied | Pass | Runs all targets and features; CI fails on a warning. |
| Rust tests | `pnpm test:rust` | cargo test | Pass | Covers native policy, persistence, media, Quick Paste, and Labs modules. |
| Dependency cycles | Not configured | — | Not assessed automatically | TypeScript and Cargo build graphs are checked, but no separate cycle detector is installed. Add one only when an actual cycle risk appears. |

## Validation result

On 2026-07-26, all configured checks passed in the P1 working tree: Biome lint, TypeScript build
check, 42 frontend tests, production frontend build, rustfmt, Clippy with warnings denied, and 71
Rust tests. Rust commands used the installed `stable` toolchain, which reported version 1.97.1 and
therefore matched the newly pinned toolchain version without downloading a second toolchain.

## Toolchain contract

| Runtime | Source of truth | Version |
| --- | --- | --- |
| Node.js | `.nvmrc` | 24.14.0 |
| pnpm | `package.json` `packageManager` | 11.9.0 |
| Rust | `rust-toolchain.toml` | 1.97.1 with `rustfmt` and `clippy` |
| Rust minimum supported version | `src-tauri/Cargo.toml` | 1.85 |

The exact Rust toolchain is pinned for reproducibility while the Cargo manifest retains its declared
minimum supported Rust version. Any future MSRV change needs an explicit compatibility decision and
CI coverage.

## Recommendations

1. Keep `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`, `pnpm lint:rust`, and
   `pnpm test:rust` green before merge.
2. Do not run `pnpm format` as a broad cleanup during feature work; it writes files. Make any
   repository-wide formatting change a separate reviewable pull request.
3. Add a focused dependency-cycle or architecture-boundary check only if the codebase gains enough
   feature modules for that failure mode to be likely.
