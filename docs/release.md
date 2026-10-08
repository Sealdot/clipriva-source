# macOS release guide

ClipRiva v1 supports macOS 13 and newer. Source and unsigned local builds may be published during the
beta, but a downloadable public binary is not considered stable until it is signed, notarized and
verified on the compatibility matrix below.

## Automated release gate

Run the complete source-level release gate from the repository root:

```bash
pnpm release:verify
```

It verifies that the version in `package.json`, `src-tauri/Cargo.toml`, and
`src-tauri/tauri.conf.json` is identical, then runs frontend lint/tests/build and Rust formatting,
Clippy-with-warnings-as-errors, and tests.

For an unsigned Apple Silicon internal-test build, run:

```bash
pnpm release:bundle:unsigned
```

The application bundle is written to:

```text
src-tauri/target/release/bundle/macos/ClipRiva.app
```

This is an internal-test artifact, not a public macOS release. It deliberately uses `--no-sign` and
will not satisfy Gatekeeper for normal Internet distribution.

For an unsigned Universal internal-test build that targets both Apple Silicon and Intel, run:

```bash
rustup target add x86_64-apple-darwin
pnpm tauri build --target universal-apple-darwin --bundles app,dmg --no-sign
scripts/verify-macos-compatibility.sh
```

The verifier requires both `arm64` and `x86_64` slices, checks that each slice and `Info.plist`
declare macOS 13.0, verifies the DMG and prints its SHA-256. Passing this verifier does not satisfy
Developer ID signing, notarization, Gatekeeper or a real Ventura/Intel smoke test.

## Public distribution gate

Before publishing outside an internal test group, provide the following release-owner inputs:

1. A permanently owned reverse-DNS bundle identifier, privacy policy and support channel. The source
   is licensed under Apache-2.0.
2. An Apple Developer Program membership and a `Developer ID Application` signing certificate.
3. A complete Xcode installation with Apple notarization tooling.
4. Notarization credentials: either App Store Connect API values (`APPLE_API_ISSUER`, `APPLE_API_KEY`,
   `APPLE_API_KEY_PATH`) or Apple ID values (`APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`).
5. A final clean-machine installation and Gatekeeper smoke test for clipboard capture, Quick Paste,
   tray behavior, launch at login and Accessibility-permission fallback.

With a valid signing identity installed locally, build the distributable app and DMG without
committing credentials:

```bash
rustup target add x86_64-apple-darwin
APPLE_SIGNING_IDENTITY='Developer ID Application: Your Organization (TEAMID)' \
pnpm tauri build --target universal-apple-darwin --bundles app,dmg
```

Tauri can use the notarization environment variables above during the build. After the build,
verify the signed app and record the installer hash:

```bash
codesign --verify --deep --strict --verbose=4 \
  src-tauri/target/release/bundle/macos/ClipRiva.app
spctl --assess --type execute -vv \
  src-tauri/target/release/bundle/macos/ClipRiva.app
shasum -a 256 src-tauri/target/release/bundle/dmg/*.dmg
```

Do not add Apple credentials, `.p8` keys, `.p12` certificates, or generated installers to version
control. Tauri's current [macOS signing guide](https://v2.tauri.app/distribute/sign/macos/) documents
the supported signing and notarization environment variables.

## Compatibility matrix

Record evidence for each row before v1.0. A checkmark requires a real machine or clean virtual
machine; source-level CI does not satisfy this gate. Use the detailed
[Beta validation matrix](beta-validation-matrix.md) to record the test steps, actual result and
issue links for every run. The support tiers and automated-runner limits are defined in the
[macOS support policy](macos-support.md).

| System | Architecture | Install and launch | Text/image | RTF/HTML/files | Quick Paste restore | Direct paste allow/deny/revoke |
| --- | --- | --- | --- | --- | --- | --- |
| Ventura 13 latest | Apple Silicon | [ ] | [ ] | [ ] | [ ] | [ ] |
| Ventura 13 latest | Intel | [ ] | [ ] | [ ] | [ ] | [ ] |
| Sonoma 14 latest | Apple Silicon | [ ] | [ ] | [ ] | [ ] | [ ] |
| Sonoma 14 latest | Intel | [ ] | [ ] | [ ] | [ ] | [ ] |
| Sequoia 15 latest | Apple Silicon | [ ] | [ ] | [ ] | [ ] | [ ] |
| Sequoia 15 latest | Intel | [ ] | [ ] | [ ] | [ ] | [ ] |
| Tahoe 26 latest | Apple Silicon | [ ] | [ ] | [ ] | [ ] | [ ] |
| Tahoe 26 latest | Apple-supported Intel | [ ] | [ ] | [ ] | [ ] | [ ] |

For each row, also verify:

1. A denied application and a high-confidence secret never appear in SQLite or Labs enrichment.
2. Pinned clips survive retention and clear-unpinned operations.
3. Relaunch and login startup preserve settings and history.
4. Restore mode never triggers an Accessibility prompt.
5. Direct paste restores the clipboard and shows a useful fallback when permission is denied or
   revoked.
6. Core and Labs generate no unsolicited network traffic.

## Product-readiness gate

Do not start a sync, MCP or Agent product layer until:

- this matrix and the stable signing gate are complete;
- the maintainer has used ClipRiva daily for four consecutive weeks without an unresolved data-loss
  or privacy-severity defect;
- at least three independent users repeatedly request the same broader workflow.
