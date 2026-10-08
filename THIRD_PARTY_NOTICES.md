# Third-party notices

The public source preview adds no vendored code or asset. CI uses upstream `cargo-audit` 0.22.2
(MIT OR Apache-2.0), verified by pinned release-asset SHA-256, and RustSec advisory data
(CC0-1.0). These are downloaded development tools/data, not shipped application dependencies.
Their exact provenance is recorded with CI evidence. Package manifests now explicitly declare
the project Apache-2.0 code license. Existing asset ownership and brand constraints remain.

This source-distribution notice lists known direct dependencies; `pnpm-lock.yaml` and
`src-tauri/Cargo.lock` contain the full resolved dependency graphs. Binary distribution requires
an artifact-specific review of bundled licenses and notices.

## Node.js direct dependencies

- `@tanstack/react-query` — MIT
- `@tauri-apps/api` — Apache-2.0 OR MIT
- `@tauri-apps/plugin-clipboard-manager` — MIT OR Apache-2.0
- `lucide-react` — ISC
- `react`, `react-dom` — MIT

Development-only direct dependencies are listed in [the dependency inventory](docs/audit/dependency-inventory.md).

## Rust direct dependencies

- `chrono`, `serde`, `serde_json`, `sha2`, `tauri`, `tauri-build`, Tauri plugins, `thiserror`, and
  `uuid` — MIT OR Apache-2.0
- `mdns-sd`, `snow`, `zeroize`, and macOS-only `security-framework` — MIT OR Apache-2.0
- `subtle` and `x25519-dalek` — BSD-3-Clause
- `block2`, `objc2`, `objc2-app-kit`, `objc2-foundation`, `rusqlite`, and `tokio` — MIT
- macOS-only `objc2-vision` — Zlib OR Apache-2.0 OR MIT

## Assets and system components

Application icons, root assets, and documentation screenshots are self-owned, Codex-assisted,
human-reviewed project assets as represented by Sealdot404. The assets remain subject to
[the ClipRiva brand policy](TRADEMARK.md), not the Apache-2.0 code license.

The 2026-08-11 app-icon update only adds an optical transparent safe area to the existing
self-owned ClipRiva mark and regenerates its platform renditions from `assets/app-icon.svg`.
It introduces no third-party artwork, font, screenshot, or other external material.

The screenshots added under `docs/release/evidence/alpha2-*` and
`docs/release/evidence/m0-91afcd8/screenshots/` in September 2026 were generated from this
repository's synthetic browser fixtures and project-owned UI. The native first-run dialog capture
shows only the project's onboarding screen. They contain no third-party visual assets or real
clipboard content and remain project-owned documentation evidence, not distribution screenshots.

### v1.2 user-story document images (2026-07-29)

`docs/ClipRiva-v1.2-当前功能用户故事（含配图）.docx` embeds four screenshots
captured from the locally running ClipRiva browser preview. They use only the
synthetic fixtures defined in `src/features/clipboard/browserRepository.ts` and
were generated within this repository for the document. They contain no user
clipboard content, account data, credentials, endpoints, or third-party visual
material. No third-party asset was added; retain this provenance record for the
next signed-binary review.

Future assets require a provenance review before inclusion. The current transitive metadata review
is recorded in [the SBOM and license audit](docs/audit/sbom-license-review.md); regenerate it for
the exact signed binary candidate and complete bundled notices before distributing an app.

## Source candidate export (2026-10-08)

This candidate preserves all included project-owned assets byte-for-byte. No name, logo,
icon, screenshot, font, illustration, audio or copied third-party source is added or changed.
Redistribution provenance relies on the existing Sealdot404 ownership and human-review
representations above; no new independent ownership verification is claimed. The ClipRiva
brand rule in [TRADEMARK.md](TRADEMARK.md) remains in effect. The three historical Word
planning documents, including the v1.2 illustrated document described above, are omitted
from the allow-listed export. Their provenance entry is retained as historical context.

The archive includes project source, documentation, project-owned UI screenshots and icon
renditions. It contains no installed Node/Rust dependency source, runtime database, clipboard
export or application bundle. [NOTICE](NOTICE) preserves project attribution; downstream
binary packaging must still collect the licenses and NOTICE files of actual bundled components.

## Private runner verification follow-up

The source verification branch adds project-authored CI metadata tooling, not third-party
vendor source or a product dependency. The workflow uses the MIT-licensed upstream
`actions/upload-artifact` v4.6.2 pinned to
`ea165f8d65b6e75b540449e92b4886f43607fa02` as development tooling only. Icons, screenshots,
project branding, manifests and lockfiles are unchanged. Fresh target-license evidence and
remaining binary notice obligations are documented in
[the private runner verification](docs/audit/source-ci-verification-2026-10-08.md).
