# SBOM and transitive-license review

- **Review date:** 2026-08-23
- **Source baseline:** the `codex/clipriva-2.0-smart-workflows` candidate commit containing this
  review; its manifest and lockfile diff changes only ClipRiva's own version from `1.5.0` to
  `2.0.0-alpha.1`
- **Purpose:** release-preparation evidence for the lockfiles; not legal advice and not a
  substitute for an artifact-specific notice review.

## Inputs and reproducible commands

The committed [`pnpm-lock.yaml`](../../pnpm-lock.yaml) and
[`src-tauri/Cargo.lock`](../../src-tauri/Cargo.lock) define the dependency sets. The following
commands produced the metadata summary below on the stated baseline:

```bash
pnpm install --frozen-lockfile
pnpm licenses list --json
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --format-version 1
```

Run them again after any lockfile change. The generated JSON contains local absolute paths and is
therefore deliberately not committed as a portable release artifact.

## Results

| Ecosystem | Components reviewed | Missing license metadata | Detected GPL/AGPL/SSPL/Commons-Clause/non-commercial declaration |
| --- | ---: | ---: | ---: |
| Node.js | 127 unique package/version entries | 0 | 0 |
| Rust | 518 unique registry package/version entries | 0 | 0 |

The 2026-08-23 rerun found no direct-dependency or resolved third-party version change:
`pnpm-lock.yaml` is byte-unchanged, while the only `Cargo.lock` diff is ClipRiva's own package
version. Fresh current-platform `pnpm licenses` metadata covered 125 installed entries with zero
missing metadata and zero reviewed restricted-license finding; the byte-unchanged lockfile retains
the previously reviewed 127-entry all-platform result in the table. Fresh offline Cargo metadata
for the distributable `aarch64-apple-darwin` graph covered 317 registry entries with the same
zero/zero result. The unchanged all-target lock graph therefore retains the previously reviewed
518-entry result below; an attempted all-target source refresh was stopped after the crate mirror
timed out while fetching non-macOS GTK sources. This limitation does not hide a dependency-set
change, but the final release environment must still rerun the all-target command.

The 2026-08-12 rerun adds the macOS-only direct dependency `objc2-vision` 0.3.2 for explicit local
image-text extraction. Its metadata declares `Zlib OR Apache-2.0 OR MIT`. Default features are off;
only the Vision request/observation APIs used by the manual extractor are enabled. The resolved
Rust registry set grows from 517 to 518 package/version entries. Node remains at 127. Both sets have
zero missing-license metadata and zero declaration lacking a permissive alternative among the
reviewed restricted-license patterns.

Node metadata used MIT (100), Apache-2.0 (7), ISC (4), BSD variants (4), MPL-2.0 (2), MIT-0 (2),
CC0-1.0 (1), BlueOak-1.0.0 (1), and compatible multi-license expressions (6). Rust metadata is
primarily MIT/Apache-2.0 variants, with BSD, Zlib, Unicode-3.0, MPL-2.0, Boost-1.0, ISC, Unlicense,
and other multi-license expressions present in the transitive graph.

Before this OCR change, the all-target Rust graph contained 517 entries. An earlier change made
`block2` 0.6.2 (MIT) a direct macOS dependency for the Network.framework path-monitor callback;
that exact crate/version was already present transitively, so that declaration did not grow the
resolved package set. The earlier Rust increase is associated with the Local Link direct
`mdns-sd` 0.20.3,
`snow` 0.10.0, `subtle` 2.6.1, `x25519-dalek` 2.0.1, `zeroize` 1.9.0 and macOS-only
`security-framework` 3.7.0 dependencies plus their transitive graphs. Their metadata declares only
Apache-2.0/MIT alternatives or BSD-3-Clause. No new missing-license or restricted-license finding
was recorded. The v1.5 candidate starts Local Link networking only after explicit user enable;
dependency presence and loopback tests still do not establish real-Mac delivery, encryption or Beta
readiness. The current rerun found no missing metadata and no declaration lacking a permissive
alternative among the reviewed GPL/AGPL/SSPL/Commons-Clause/non-commercial patterns. `r-efi`
5.3.0/6.0.0 declare `MIT OR Apache-2.0 OR LGPL-2.1-or-later`; the distributable build may select
the MIT or Apache-2.0 alternative, so this is recorded as a multi-license notice obligation rather
than a restricted-license finding.

The 2026-08-12 commands completed successfully with Cargo HTTP multiplexing disabled. The generated
Node and Rust JSON metadata stayed outside the repository because it contains local absolute paths.

## Interpretation and release requirements

1. This is metadata evidence, not a conclusion that every dependency can be redistributed in every
   form. A package's declared SPDX expression can be incomplete or may carry notice obligations.
2. Review `THIRD_PARTY_NOTICES.md`, bundled SQLite obligations, and every dependency shipped in the
   final macOS artifact before release. Tooling and test-only packages are not necessarily shipped.
3. Pay particular attention to MPL-2.0 and Unicode-3.0 entries when preparing final notices and to
   target-specific Rust dependencies selected by the release build.
4. Run the package-manager and Rust vulnerability scans in the final release environment; neither a
   license report nor a lockfile alone establishes vulnerability status.
5. Preserve the two lockfiles in the release tag and attach a regenerated SBOM in the chosen
   artifact format if distribution policy requires one.
