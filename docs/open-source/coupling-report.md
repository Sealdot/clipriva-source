# Open-source coupling report

- **Assessment date:** 2026-07-26
- **Status:** analysis only; no refactor was performed.

## Conclusion

The current ClipRiva Core workflow is independently runnable from source. It does not require a
login, official service, cloud/model provider, telemetry endpoint, or private SDK. The frontend
uses an in-memory browser repository during browser development and a fixed Tauri IPC allow-list in
the desktop build. The native implementation stores data locally in SQLite and local blob storage.

No private module is present in the repository for Core to depend on. The main open-source readiness
work is therefore preserving these local-first boundaries as new capabilities are added.

## Coupling checks

| Check | Evidence | Result | Recommendation |
| --- | --- | --- | --- |
| Startup dependency on an official service | Application config uses a local Vite development URL; the CSP permits only Tauri IPC and its local IPC host. | Pass | Keep production connections absent unless a separately approved feature introduces a documented adapter. |
| Login/account requirement | No account or authentication dependency is declared in application manifests or runtime code. | Pass | Treat any account feature as a separate product-layer decision. |
| Frontend-to-storage coupling | `src/features/clipboard/api.ts` selects a browser repository or typed Tauri commands; it does not access SQLite directly. | Pass | Keep this as the only feature runtime bridge and avoid database-file access from the WebView. |
| IPC privilege boundary | `src-tauri/src/commands.rs` exposes named clipboard, preferences, diagnostics, enrichment, and action commands. | Pass with review requirement | Continue reviewing each new command for arbitrary SQL, arbitrary paths, shell execution, or raw clipboard-content leaks. |
| Local persistence | `src-tauri/src/db/` and `src-tauri/src/media/` own SQLite and blob storage. | Pass | Future sync must call versioned application services rather than synchronize the database file. |
| Labs and semantic retrieval | `src-tauri/src/context/semantic.rs` implements an offline lexical fallback; no provider client is declared. | Pass, experimental | Keep Labs optional and local. Require a separate security and product review before adding embeddings or a remote provider. |
| Future cloud action surface | `src-tauri/src/actions.rs` builds a redacted preview and explicitly has no executor; the command surface cannot perform a cloud action. | Pass, experimental | Do not turn the preview into a remote executor without a dedicated adapter, consent flow, configuration review, and boundary decision. |
| Environment configuration | The former sync/browser environment placeholders were not read by code. Tool-provided `TAURI_ENV_*` values are build-only. | Pass after cleanup | Keep `.env.example` value-free until a real documented configuration need exists. |
| Release credentials | `docs/release.md` names Apple credential variables but no values are tracked. | Pass with operational dependency | Store values only in a local secure store or CI secrets; verify release workflows before publication. |

## No-refactor follow-up

1. Add a CI check that fails on new remote-client dependencies or a broadened CSP unless the change
   is approved with the relevant privacy review.
2. When Labs becomes public-facing, declare its compatibility and data-handling contract in the
   README and release notes.
3. Before adding sync or accounts, design a versioned application-service API and a separate
   private-service boundary. Do not let a cloud client read `clipriva.sqlite3` or blob files.
