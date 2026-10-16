# Remaining PRs compatibility and dependency review

- Review date: 2026-10-16; repository: `Sealdot/clipriva-source`.
- Baseline: `17ec4c17a43d210cb31bf0bdc9b5e5e16e9a605d` after integration #23.
- Dependency/repair source: `3328b7ea5bd33beffffb608d358b7ad6e0691109`.
  Audit follow-ups add inventory, notices and portable evidence; a mechanical Luhn-expression
  update follows for the newly applicable Rust lint. Component reports bind the dependency
  graph with both lockfile SHA-256 values.
- Validation toolchain: Node 24.15.0, pnpm 11.28.6 (final), Rust/Cargo 1.97.1; local macOS ARM64.

## Included work

| PR | Change | Original head |
| --- | --- | --- |
| [#3](https://github.com/Sealdot/clipriva-source/pull/3) | checkout 6 → 7.0.1 | `62e057a9d7fd6b27b12cbcc9f3784a57e8b82322` |
| [#4](https://github.com/Sealdot/clipriva-source/pull/4) | mdns-sd 0.20.3 → 0.21.5 | `a930922bfa5086db2ee3391cade7f2e7abdb3a8b` |
| [#5](https://github.com/Sealdot/clipriva-source/pull/5) | pnpm/action-setup 6.0.8 → 6.1.0 | `9591df1355fc7fbd987bfb9d6b37d17aadcc2931` |
| [#6](https://github.com/Sealdot/clipriva-source/pull/6) | global-shortcut 2.3.2 → 2.4.0 and required Tauri runtime graph | `bbb44fe8c0cfe4d0237e8f9dfc2d5b560d350e72` |
| [#7](https://github.com/Sealdot/clipriva-source/pull/7) | sha2 0.10 → 0.11.0, with compatibility repair | `5da8798408f452dbe90bff2aec8663de7ac0a63b` |
| [#8](https://github.com/Sealdot/clipriva-source/pull/8) | x25519-dalek 2.0.1 → 3.0.0 | `0a6b0ff0116152295133a0ff4b62cf21c5655fd5` |
| [#10](https://github.com/Sealdot/clipriva-source/pull/10) | tokio 1.53.1 → 1.53.2 | `768eefc1bff91659a6946cfb3a640f1d5a474e81` |
| [#15](https://github.com/Sealdot/clipriva-source/pull/15) | Bilingual first-use README, reusing approved images | `e99c75424c6d830ef7acb043fcd6f38aa586e74a` |
| [#16](https://github.com/Sealdot/clipriva-source/pull/16) | setup-node 6 → 7.1.0 | `861c15194396d351b12215f7dc39d2a864ae505e` |

All original heads are preserved as ancestors of the combined branch. Original cancelled
checks are incomplete evidence, not passes. The new combined candidate requires full CI
and all five macOS native compatibility jobs before an authorized merge.

## Failure causes and repairs

### SHA-256 compatibility

The failing #7 CI reports that the new digest output no longer implements `LowerHex`.
Replace all eleven old digest-formatting sites, including streaming representation hashes,
with one project-authored lowercase byte encoder. The resulting strings retain leading zeroes,
64 lowercase hex digits and the existing content-addressed paths. No content or schema migration
is introduced. Keep the older transitive sha2 0.10.9 required by Noise/Tauri dependencies.

New regressions check known empty/abc SHA-256 values, streaming equivalence, zero/high-byte
encoding, the pre-upgrade represented-content hash and a known blob key after reopen/dedup.
The x25519 major upgrade is separately guarded by a version-1 identity blob derived using
previous x25519-dalek 2.0.1 and sha2 0.10.9 with a repeated-byte synthetic fixture. Public key,
fingerprint and exact blob bytes must remain unchanged; altered public keys must still fail.
No real device key or clipboard content is used.

### Intel pnpm setup

The failed #5 Intel jobs select a native pnpm self-update and fail with
`ERR_PNPM_PNPM_ENGINE_IDENTITY_UNVERIFIABLE`. The first attempted repair loaded the pinned
Node before the action. Combined CI then proved that pnpm/action-setup 6.1.0 still invokes
the failing self-update with Node 24.15.0 active and standalone=false. Those failed/cancelled
runs are not accepted evidence.

Supersede the action with tools/ci-pnpm/package-lock.json: npm ci installs an exact pnpm
package under the runner temporary directory, validates committed registry integrity and
runs no install scripts. Check its installed version against the root packageManager pin,
then expose its JavaScript CLI on PATH. No native self-update or integrity-policy bypass is
used. The bootstrap is included in native matrix path filters. Store caching from the old
action is removed; each runner performs the frozen application install normally.

The new explicit bootstrap audit also reveals public high-severity advisories for the old
pnpm 11.9.0 tool. Upgrade the root tool pin and active setup documentation to 11.28.6; its
isolated lockfile audit reports zero findings. The application Node graph is unchanged.
The separate bootstrap adds one MIT development-tool component, with registry integrity
verified. All five runners, especially both Intel runners, must validate this final repair.

### Required runtime and toolchain updates

The downloaded global-shortcut 2.4.0 manifest requires Tauri `2.12` and tauri-plugin build tooling
`2.7`; it is not an isolated shortcut patch. Preserve its compatible runtime graph, including
Tauri 2.12.1 and build tooling 2.7.1. Their maximum declared Rust requirement is 1.90, so raise
the project's stale 1.85 floor to 1.90. Validation still uses the pinned 1.97.1 toolchain; no
claim of running the complete suite on the minimum Rust version is made. Raising the declared
floor also enables Clippy’s is_multiple_of lint; replace the existing Luhn check’s equivalent
modulo expression with that standard method, without suppressing warnings or changing policy.

The lockfile adds/updates 57 Rust registry package versions. Their downloaded crate files
match every committed registry checksum. Runtime/UI dependencies change, so native compilation
on the full OS/architecture matrix remains required. Real native behaviour needs separate
acceptance; synthetic browser screenshots and compilation do not establish it.

## Privacy and Local Link boundaries

Update PRIVACY.md, data-flow.md, module-boundary.md and the acceptance record together with
the native changes. mdns-sd selects only `async`; defaults, logging and serde stay off.
New advertisement tests cover both pairing states while allowing only `v`/`pair` metadata.
Existing disabled-readiness/no-transport, policy, authentication and lifecycle tests remain.
x25519 retains static-secret verification and zeroization; Noise's protocol/cipher suite and
Keychain attributes/blob version do not change. No new network destination, permission,
stored data type, telemetry, cloud route, export, trust rule or fallback is introduced.

The README changes retain the original reviewed first-use content, use existing approved captures, and
keep frozen-release pins distinct from current main; only the active pnpm pin changes to 11.28.6. All 62 relative-link file targets were
checked on the combined tree. No image, icon, logo, font or other asset bytes change.

## License, integrity and security evidence

| Scope | Components | Missing | Restricted without permissive alternative | Unsupported |
| --- | ---: | ---: | ---: | ---: |
| Installed macOS ARM64 Node packages | 117 | 0 | 0 | 0 |
| Reachable all-feature macOS ARM64 Rust registry graph | 315 | 0 | 0 | 0 |
| Reachable all-feature all-platform Rust registry graph | 514 | 0 | 0 | 0 |

The application Node lockfile/versions are unchanged; packageManager now pins pnpm 11.28.6.
The separate CI bootstrap lockfile adds one MIT component with zero npm advisory findings. New/updated Rust declarations have no missing,
unsupported or restricted-without-permissive-alternative findings. The reports retain original
expressions, legacy syntax normalization and both r-efi permissive alternatives to LGPL.
The three action tags/commits, MIT licenses and Node 24 metadata were checked against upstream;
pnpm's annotated tag was dereferenced before comparison. Tool binaries/sources are not vendored.

Fresh full and production npm audits report zero findings. Fresh locked RustSec review reports
zero vulnerabilities and two reviewed warnings, with no ignored advisories: proc-macro-error
maintenance and glib unsoundness. Both are absent from the macOS target graph. The runtime
upgrade removes five previously flagged unmaintained Unicode crates, reducing warnings from
seven to two. This is not approval to distribute the unsupported Linux dependency graph.

- [macOS component evidence and lockfile hashes](remaining-prs-evidence/macos-arm64.json).
- [All-platform Rust evidence](remaining-prs-evidence/all-platforms.json).
- [Changed Rust downloads and checksum verification](remaining-prs-evidence/changed-rust-packages.json).
- [Action provenance and license hashes](remaining-prs-evidence/action-provenance.json).
- [Application npm advisory summary](remaining-prs-evidence/npm-audit-summary.json) and
  [separate patched pnpm bootstrap review](remaining-prs-evidence/pnpm-bootstrap.json).
- [Reviewed RustSec aggregate](remaining-prs-evidence/rust-advisory-review.json) and
  [tool/database provenance](remaining-prs-evidence/rust-audit-provenance.txt).

Raw metadata with personal paths, downloaded tooling and raw security findings remain outside
the checkout. Installed Node evidence does not cover every optional package on other platforms;
license metadata does not replace exact binary notices or redistribution review.

## Validation and remaining acceptance

The combined source passes all 26 frontend files / 237 tests, frontend lint, TypeScript and
production build. Native library tests pass: 247 tests, zero failures, one explicitly ignored
benchmark. Six regressions were added without relaxing existing assertions or exclusions.
Both locked all-feature Cargo metadata graphs, license gates, checksum review, npm audits and
RustSec gate pass. The existing >500 kB frontend chunk warning remains.

After the audit follow-up commit, run final relevant checks on a clean source tree, push that
exact commit and run scripts/verify-git-sync.sh. Record exact candidate and combined CI results
in the PR/handoff. Combined CI must run locked Clippy/all-target/all-feature checks with warnings
denied, full Rust tests, the explicitly ignored release benchmark and all five native builds.

Native UI/focus/global-shortcut receipt, actual target-app paste, real two-Mac Local Link,
signed installation/notarization, frozen Release download verification and binary-specific
notices remain outside this source integration. The source preview and default-off Local Link
release gates remain unchanged; no release, tag or binary distribution is authorized here.
