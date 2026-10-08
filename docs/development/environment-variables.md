# Environment variables

**Last reviewed:** 2026-07-26

ClipRiva currently has no application-defined environment variables. A normal local build and the
desktop application work without a `.env` file.

## Application configuration

`.env.example` is intentionally value-free. It documents that there is no user-supplied local
configuration at this point in the project. Do not add a local `.env` file unless a documented
feature explicitly requires one; all `.env` variants except `.env.example` are ignored by Git.

The former `VITE_CLIPRIVA_SYNC_URL` and `VITE_CLIPRIVA_BROWSER_DEMO` placeholders were not read by
the application and were removed from the template during the open-source preparation audit. The
current browser adapter is selected by runtime detection, not by an environment variable.

## Tool-provided build variables

| Variable | Required from developers | Used by | Purpose | Behavior when absent |
| --- | --- | --- | --- | --- |
| `TAURI_ENV_PLATFORM` | No | `vite.config.ts` | Lets Tauri select a Windows-compatible target when it invokes the frontend build. | Vite uses the macOS/Safari target. |
| `TAURI_ENV_DEBUG` | No | `vite.config.ts` | Lets Tauri select development-friendly minification and sourcemap settings. | The production minifier is used and sourcemaps are disabled. |

These are supplied by Tauri for its own build modes. They are not secrets and should not be added
to `.env.example` or set as a deployment configuration contract.

## Release credentials

Apple signing and notarization names appear only in [the release guide](../release.md). They are
release-owner inputs, not application configuration. Store their values in a local secure store or
the CI platform's secret manager; never put them in `.env.example`, source files, documentation,
or Git history.

If a future capability needs configuration, add it here before adding code. Each entry must state
whether it is required, its safe local value, how developers obtain it, and the behavior when it is
missing. Production endpoints and personal account values are not valid defaults.
