# Dependency and license inventory

Tauri CLI update (2026-10-09, PR #12): the development/build CLI now resolves to 2.12.1.
Its 11 platform packages update to the same version; no direct application or Rust dependency
changes. See the [fresh license review and scope](tauri-cli-2.12.1-review-2026-10-09.md).

Public source preview (2026-10-08): no direct product dependency or resolved version changes.
Manifest changes add project license metadata only. `cargo-audit` 0.22.2 (MIT OR Apache-2.0)
and RustSec advisory data (CC0-1.0) are downloaded CI tooling, not product dependencies.
See [the public preview record](source-public-preview-2026-10-08.md).

- **Assessment date:** 2026-08-12
- **Scope:** direct dependencies resolved by `pnpm-lock.yaml` and `src-tauri/Cargo.lock`. The lock
  files remain the authoritative transitive dependency set. This inventory is not legal advice.

## Node.js dependencies

| Dependency | Version | Purpose | License | Direct | Risk |
| --- | --- | --- | --- | --- | --- |
| `@tanstack/react-query` | 5.101.4 | Frontend query/cache state | MIT | Yes | Low |
| `@tauri-apps/api` | 2.11.1 | Typed frontend access to Tauri | Apache-2.0 OR MIT | Yes | Low |
| `@tauri-apps/plugin-clipboard-manager` | 2.3.2 | Clipboard plugin bindings | MIT OR Apache-2.0 | Yes | Low |
| `lucide-react` | 1.26.0 | UI icons | ISC | Yes | Low |
| `react` | 19.2.8 | UI runtime | MIT | Yes | Low |
| `react-dom` | 19.2.8 | Browser rendering | MIT | Yes | Low |
| `@biomejs/biome` | 2.5.5 | Format/lint checks | MIT OR Apache-2.0 | Yes, development | Low |
| `@tauri-apps/cli` | 2.12.1 | Tauri development/build CLI | Apache-2.0 OR MIT | Yes, development | Low |
| `@testing-library/jest-dom` | 7.0.0 | DOM test assertions | MIT | Yes, development | Low |
| `@testing-library/react` | 16.3.2 | React component tests | MIT | Yes, development | Low |
| `@types/node` | 26.1.1 | Node.js types | MIT | Yes, development | Low |
| `@types/react` | 19.2.17 | React types | MIT | Yes, development | Low |
| `@types/react-dom` | 19.2.3 | React DOM types | MIT | Yes, development | Low |
| `@vitejs/plugin-react` | 6.0.4 | Vite React integration | MIT | Yes, development | Low |
| `jsdom` | 29.1.1 | Browser-like test environment | MIT | Yes, development | Low |
| `typescript` | 7.0.2 | Type checking | Apache-2.0 | Yes, development | Low |
| `vite` | 8.1.5 | Frontend development/build | MIT | Yes, development | Low |
| `vitest` | 4.1.10 | Test runner | MIT | Yes, development | Low |

## Rust dependencies

| Dependency | Version | Purpose | License | Direct | Risk |
| --- | --- | --- | --- | --- | --- |
| `block2` | 0.6.2 | Own the copied Objective-C block used by the macOS Network.framework Local Link path monitor callback | MIT | Yes, macOS | Low; already present in the locked transitive graph, now explicit because production code uses its API directly |
| `chrono` | 0.4.45 | Time handling | MIT OR Apache-2.0 | Yes | Low |
| `mdns-sd` | 0.20.3 | Native allow-listed Bonjour/mDNS discovery for explicitly enabled Local Link | Apache-2.0 OR MIT | Yes | Medium; network-boundary code, default-off Preview/Alpha and real-Mac gate required |
| `objc2` | 0.6.4 | macOS Objective-C bindings | MIT | Yes, macOS | Low |
| `objc2-app-kit` | 0.3.2 | macOS AppKit bindings | MIT | Yes, macOS | Low |
| `objc2-foundation` | 0.3.2 | macOS Foundation bindings | MIT | Yes, macOS | Low |
| `objc2-vision` | 0.3.2 | Manual local image-text extraction through macOS Vision | Zlib OR Apache-2.0 OR MIT | Yes, macOS | Low; system-framework binding only, no remote model, network route or new permission |
| `rusqlite` | 0.37.0 | SQLite persistence | MIT | Yes | Low |
| `serde` | 1.0.229 | Serialization | MIT OR Apache-2.0 | Yes | Low |
| `serde_json` | 1.0.151 | JSON serialization | MIT OR Apache-2.0 | Yes | Low |
| `sha2` | 0.10.9 | Content hashing | MIT OR Apache-2.0 | Yes | Low |
| `snow` | 0.10.0 | Noise XX handshake and encrypted framing for explicitly paired Local Link peers | Apache-2.0 OR MIT | Yes | Medium; security-critical protocol code, loopback is not real-Mac evidence |
| `subtle` | 2.6.1 | Constant-time comparison for identity/fingerprint validation | BSD-3-Clause | Yes | Medium; security-sensitive comparison primitive |
| `tauri` | 2.11.5 | Desktop runtime | MIT OR Apache-2.0 | Yes | Low |
| `tauri-build` | 2.6.3 | Tauri build integration | MIT OR Apache-2.0 | Yes, build | Low |
| `tauri-plugin-autostart` | 2.5.1 | Optional launch-at-login | MIT OR Apache-2.0 | Yes | Low |
| `tauri-plugin-clipboard-manager` | 2.3.2 | Native clipboard integration | MIT OR Apache-2.0 | Yes | Low |
| `tauri-plugin-global-shortcut` | 2.3.2 | Global shortcut support | MIT OR Apache-2.0 | Yes | Low |
| `thiserror` | 2.0.19 | Error definitions | MIT OR Apache-2.0 | Yes | Low |
| `tokio` | 1.53.1 | Async runtime | MIT | Yes | Low |
| `uuid` | 1.24.0 | Local identifiers | MIT OR Apache-2.0 | Yes | Low |
| `x25519-dalek` | 2.0.1 | X25519 static-key generation and public-key derivation for the candidate identity | BSD-3-Clause | Yes | Medium; security-critical identity primitive |
| `zeroize` | 1.9.0 | Clear owned Local Link identity and pending-plaintext buffers on drop | Apache-2.0 OR MIT | Yes | Medium; best-effort secret-buffer hygiene, not a platform memory guarantee |
| `security-framework` | 3.7.0 | Store the versioned Local Link identity in macOS Keychain | MIT OR Apache-2.0 | Yes, macOS | Medium; OS credential-store lifecycle and accessibility require real-Mac review |

## Other materials and unresolved checks

| Item | Status | Risk | Required follow-up |
| --- | --- | --- | --- |
| SQLite | Bundled through `rusqlite`; no separately committed binary found | Low | Keep its license notice in the final third-party review. |
| macOS frameworks | Used through Tauri/Objective-C bindings; not committed to this repository | Low | Validate distribution obligations during signed-release preparation. |
| Application icons and screenshots | Stored in `src-tauri/icons/`, `assets/`, and `docs/assets/`; confirmed by Sealdot404 as self-owned, Codex-assisted, and human-reviewed | Low | Reconfirm the provenance of every new asset; the brand policy controls name/logo use. |
| Fonts, illustrations, audio, copied external code | No separate third-party asset declaration found | Medium | Require provenance review before public release; do not assume a future asset is redistributable. |
| Transitive Node/Rust dependencies | Locked but not individually listed in this direct-dependency inventory | Medium | Run a full SBOM/license scan in the release environment before public release. |

No direct dependency has a GPL, AGPL, SSPL, Commons Clause, non-commercial, or source-unavailable
license according to the installed package/crate metadata reviewed for this inventory. Recheck when
the lockfiles change.

## Private source CI tooling (2026-10-08)

No direct product dependency changes in this verification round. The existing GitHub CI
workflow additionally uses `actions/upload-artifact` v4.6.2, pinned to
`ea165f8d65b6e75b540449e92b4886f43607fa02`; its upstream license is MIT. It stores only
portable source-check evidence for 30 days and is not bundled in ClipRiva. The target
metadata coverage and remaining all-platform/binary obligations are in
[source-ci-verification-2026-10-08.md](source-ci-verification-2026-10-08.md).
