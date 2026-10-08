# macOS 13 Universal compatibility review

- **Review date:** 2026-08-12
- **Source commit:** `6b59d345dd90545833d7553463ae9d5319cbec1f`
- **Build host:** macOS 14.2.1 on Apple Silicon
- **Scope:** binary and packaging compatibility for macOS 13 or later on Apple Silicon and Intel

## Reproducible build and verification

The source commit was checked out into a clean detached worktree. Dependencies were installed from
the committed lockfile with `pnpm install --frozen-lockfile`, and the full release gate passed before
the distributable was built:

```bash
pnpm release:verify
CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_INCREMENTAL=0 \
  pnpm tauri build --target universal-apple-darwin --bundles app,dmg --no-sign
scripts/verify-macos-compatibility.sh
```

The release gate completed with 191 frontend tests passing and 203 Rust tests passing. The one
ignored Rust test is the existing release-mode Quick Paste performance benchmark and was not
silently converted into a pass.

## Artifact evidence

| Check | Result |
| --- | --- |
| Executable architectures | `x86_64 arm64` |
| `Info.plist` minimum system | `13.0` |
| arm64 Mach-O `LC_BUILD_VERSION` minimum | `13.0` |
| x86_64 Mach-O `LC_BUILD_VERSION` minimum | `13.0` |
| DMG container verification | valid |
| Unsigned DMG SHA-256 | `31300a1372e3999fe07959190c19c5a94fffc34b6a444566ebcd953e8784a2d3` |

The verifier checks the two architecture slices independently before reporting success. This
evidence establishes the requested architecture and deployment-target metadata; it does not replace
runtime testing on the oldest supported operating system.

## Signing, notarization, and remaining device checks

The reviewed build intentionally used `--no-sign`. `codesign --verify --deep --strict` reports that
the application is not signed, and Gatekeeper assessment rejects it because there is no usable
signature. The build host has no valid Developer ID signing identity, so this artifact is for
controlled internal compatibility testing only and is not a public-release candidate.

Before external distribution, build the same Universal target with an authorized Developer ID
Application identity, notarize the DMG with Apple, staple the ticket, and rerun the signing,
Gatekeeper, checksum, and compatibility checks in `docs/release/release-checklist.md`.

Required real-device validation remains:

1. Install and launch on an Apple Silicon Mac running macOS Ventura 13.7.8.
2. Exercise text, image, rich-text, and file-reference capture; Quick Paste and Direct Paste; manual
   image-text extraction; relaunch from the Dock; and permission-denied recovery.
3. Repeat installation, launch, capture, and paste smoke tests on an Intel Mac running macOS 13 or
   later.

## Privacy, security, and license impact

This compatibility change adds no dependency, network request, account, telemetry, remote model,
stored data type, export path, OS permission, or third-party asset. It changes the declared minimum
macOS version, produces a Universal binary, and adds release verification/documentation only.
Accordingly, no privacy-boundary or SBOM/license inventory update is required for this change. The
unsigned status is explicit and must not be represented as a notarized or generally distributable
release.
