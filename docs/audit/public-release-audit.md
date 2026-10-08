# Public-release risk audit

> Historical 2026-07-26 binary-release review. See the
> [2026-09-30 public source audit](public-source-audit-2026-09-30.md) for the separate GitHub
> visibility decision; this report's earlier baseline is not the current source candidate.

- **Audit date:** 2026-07-26
- **Source baseline:** `0b8510b9e311ac25252bcab963b91e26d5f49d49`
- **Repository scope:** 178 tracked files and all 13 reachable commits at the time of the audit.
- **Decision:** Not ready to publish yet. The repository content has no high-severity public-source
  finding, but the required final build and release-environment checks remain open.

This is an engineering audit, not legal advice or a substitute for incident response. It evaluates
only material that is tracked at the stated baseline; untracked working files are not part of a
future GitHub source archive unless they are deliberately added later.

## Findings

| Area | Result | Evidence and required action |
| --- | --- | --- |
| Current and historical credentials | Conditional pass | Searched every reachable revision for private-key headers and common cloud, GitHub, Slack, and assignment-shaped token patterns. Matches are deliberate synthetic values in unit tests, sensitive-content detection, and test instructions. No usable credential was identified. Before the final tag, run `gitleaks detect --redact --source .` and a full-history scanner in a trusted release environment. |
| Commercial services and production source | Pass | Static source and boundary-document review found no account, billing, server, deployment, telemetry, sync, cloud-model, MCP, or Agent implementation. Future operations are documented as private boundaries, not checked-in services. |
| Production configuration and internal endpoints | Pass | `.env.example` is value-free. Application source has only Tauri's local development URL/IPC values, schema URLs, and synthetic example URLs; no production endpoint or internal domain was found. |
| User data and personal paths | Pass | No tracked database, clipboard export, log, or user-data dump was found. The only `/Users/...` strings are explicit synthetic test paths using `example`. |
| Signing material | Pass | No tracked `.p8`, `.p12`, `.pfx`, `.mobileprovision`, private-key, or keystore file was found. `.gitignore` covers those artifacts. Actual signing and notarization remain release-owner work outside the repository. |
| Assets and brand material | Conditional pass | Sealdot404 confirmed that icons, root assets, and documentation screenshots are self-owned, Codex-assisted, and human-reviewed. Reconfirm provenance for every newly added asset before the final tag; [TRADEMARK.md](../../TRADEMARK.md) governs brand use. |
| Dependency licenses | Conditional pass | The transitive metadata review in [the SBOM and license review](sbom-license-review.md) found no missing license field or GPL/AGPL/SSPL/Commons-Clause/non-commercial declaration. This is not a legal distribution opinion; review notice and copyleft obligations again when either lockfile changes. |
| Documentation and product claims | Conditional pass | Main README, privacy, security, boundary, and release documents match the current static implementation review. Local Markdown links must be rechecked on the final tag. Claims about no unsolicited network traffic still require the real-machine network observation listed in the release checklist. |
| Test accounts and contact data | Pass | `sealdot404@gmail.com` is the intentional security/project contact. Other email matches are `alice@example.com` fixtures. No test-account credential was found. |
| Commit metadata | Informational | All reachable commits use Sealdot's GitHub no-reply author address. Confirm that this public attribution is desired before making the repository public. |

## Transitive dependency summary

The locked dependency sets were reviewed from installed Node package metadata and `cargo metadata
--locked --format-version 1`:

- Node.js: 127 unique package/version entries; none had a missing license field or a detected
  restricted-license declaration.
- Rust: 484 registry packages (plus the local ClipRiva package); none had a missing license field
  or a detected restricted-license declaration.

The scan includes permissive-license families plus a small number of MPL-2.0, Unicode-3.0,
Boost-1.0, and other multi-license declarations. These are not automatically incompatible with the
project's Apache-2.0 source license, but their exact distribution obligations should be checked for
the final artifact.

## Open release gates

1. Complete the blocked clean-checkout Rust test and unsigned App build described in
   [the clean-environment report](clean-environment-test.md).
2. Re-run the secret scan, dependency review, Markdown-link check, and `pnpm release:verify` on the
   final release candidate after all feature work is merged.
3. Run a vulnerability scan in the release environment, including the production Node dependency
   set and the Rust lockfile.
4. Build, sign, notarize, hash, and Gatekeeper-test the intended macOS artifact; complete the
   applicable real-machine rows in `docs/beta-validation-matrix.md`.
5. Have Sealdot404 manually approve the asset provenance, public documentation claims, Git author
   metadata, version, changelog, and release notes before any public action.

No GitHub visibility change, tag, GitHub Release, installer upload, signing operation, or source
history rewrite is authorized by this audit.
