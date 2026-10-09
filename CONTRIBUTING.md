# Contributing to ClipRiva

## Before opening a change

1. Keep capture and local history usable without an account or network.
2. Avoid adding a dependency when a small, tested module is sufficient.
3. Do not pass raw clipboard content to a remote service in fixtures, logs or telemetry.
4. Add migrations for schema changes; never mutate an existing released migration.
5. Update `docs/architecture.md` when a boundary changes.
6. Keep ClipRiva Labs optional and disabled by default.
7. Do not add sync, MCP, Agent or cloud execution infrastructure to the v1 core roadmap.
8. Follow the [open-source change rules](docs/development/open-source-change-rules.md). They define
   the required documentation and audit follow-up for dependencies, privacy boundaries, assets, and
   sensitive material.

## Development environment

ClipRiva currently supports macOS 14 or newer. Install Node.js 24.15.0 (see `.nvmrc`), pnpm 11.9.0,
Rust 1.97.1 (see `rust-toolchain.toml`), Xcode Command Line Tools, `rustfmt`, and `clippy`.

Installation enforces the supported Node range `^24.15.0 || >=26.0.0` through
`pnpm-workspace.yaml`. Use the version pinned in `.nvmrc` to match CI; Node 24.14.0
is no longer supported by the JSDOM 30 test environment.

```bash
pnpm install --frozen-lockfile
pnpm desktop:dev
```

## Local checks

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm lint:rust
pnpm test:rust
```

Frontend files are formatted and linted with Biome. Rust follows `rustfmt`.

## Branches and commits

Create a focused branch from the current integration branch. Use one of these prefixes:

```text
feature/xxx
fix/xxx
docs/xxx
refactor/xxx
chore/xxx
test/xxx
```

Use a concise imperative commit subject, for example:

```text
feat: add device discovery
fix: prevent duplicate clipboard records
docs: update local setup guide
chore: configure lint workflow
```

## Commit shape

Prefer focused commits using a concise imperative subject. Include:

- the user-visible or architectural outcome;
- the privacy/security impact, if any;
- tests added or a reason automated verification is not possible.

## Pull requests

1. Search existing issues and discussions before starting work.
2. Keep a pull request to one problem and rebase or merge the requested base branch before review.
3. Explain the change, why it is needed, related issue, validation, screenshots (when UI changes),
   and privacy/security/compatibility impact.
4. Run the relevant local checks. Do not mark a check as passed if it was skipped.
5. Request review only when the description and self-review checklist are complete.

## Issues and proposals

Use the GitHub issue forms for bugs and feature ideas once the repository is public. Bug reports
should include reproducible steps, version, macOS version, installation method, expected and actual
behavior, and sanitized logs or screenshots. Remove clipboard contents, paths, account identifiers,
and credentials before posting.

Feature proposals should state the user scenario, the current limitation, the desired result,
alternatives considered, and whether the proposal fits the local-first Core boundary. A willingness
to help implement or test is useful but never required.

Discuss these changes before implementation:

- new permissions, network connections, accounts, telemetry, cloud/model providers, sync, MCP, or
  Agent integrations;
- schema or migration strategy changes;
- new native platform targets or large dependencies;
- changes to clipboard retention, secret handling, direct paste, or any privacy boundary;
- broad refactors, UI redesigns, or trademark/branding changes.

The project does not accept credentials, signing material, user clipboard data, copied proprietary
code or assets without redistribution rights, invasive telemetry, or v1 Core changes that require
an account or remote service.

## Security reports

Do not open a public issue for a suspected vulnerability. Follow the private reporting instructions
in [SECURITY.md](SECURITY.md).
