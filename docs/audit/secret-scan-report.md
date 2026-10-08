# Sensitive-information scan report

- **Scan date:** 2026-07-26
- **Repository baseline:** `22272ff` (all 7 reachable commits)
- **Scope:** tracked source and documentation, non-ignored working-tree files, Office documents in
  the working tree, Git history, credential-like filenames, local paths, internal-address patterns,
  and author metadata. Dependency/build output and `.git` internals were excluded from working-tree
  content scanning. Git history was scanned separately.

## Result

No real credential, private key, signing artifact, local `.env` file, production endpoint, or
user-data dump was identified. The scans produced only the expected synthetic inputs used to test
ClipRiva's sensitive-content protections and documentation telling testers not to use real
secrets.

The report intentionally does not reproduce any candidate values.

## Method and coverage

| Check | Coverage | Result |
| --- | --- | --- |
| High-confidence token and private-key signatures | Working tree and all reachable Git revisions | No real credential identified; expected fixtures only |
| Assignment-shaped secret patterns | Working tree and all reachable Git revisions | No real credential identified |
| Credential-like filenames | Working tree and every historical tree | Only the safe `.env.example` template is present |
| Local path, private-network, email, and user-data indicators | Working tree and Git metadata | One synthetic path fixture, synthetic email fixtures, and a GitHub no-reply author address; no personal local path or user data found |
| Office files | Six `.docx` and two legacy `.doc` files in the working tree | No high-confidence indicator found |
| Dedicated scanners | `gitleaks`, `trufflehog`, and `detect-secrets` | Not installed in this environment |

The history check used `git rev-list --all` and `git grep` across all seven reachable commits, so
removing a value from the current tree would not have hidden a historical finding.

## Findings

| ID | Location | Risk | Assessment | Recommended handling |
| --- | --- | --- | --- | --- |
| S-01 | `src-tauri/src/clipboard/policy.rs`, `monitor.rs`, `db/mod.rs`, `quick_paste.rs`, and `context/mod.rs` | Low | Pattern matches are test fixtures or the code that recognizes and blocks sensitive clipboard content. No usable credential was found. | Keep the tests; continue using clearly synthetic values. Review any future fixture before committing it. |
| S-02 | `docs/beta-validation-matrix.md` | Low | A test instruction names a private-key header as an example and explicitly says not to copy a real key. | Keep the warning. Do not replace it with a real credential while testing. |
| S-03 | `src-tauri/src/db/mod.rs` | Low | Absolute-path matches are deliberately synthetic macOS file-reference test data, not a developer's local path. | Keep synthetic paths only in tests. |
| S-04 | Git commit metadata in all reachable commits | Low | A GitHub no-reply author address is public metadata if the repository is made public. It is not a runtime secret. | Before publication, confirm the contributor accepts that attribution. If not, change the author identity and rewrite history only with explicit maintainer approval. |
| S-05 | `.env.example` and `docs/release.md` | Informational | The template contains no values. Release documentation names required signing/notarization variables but contains no credentials. | Keep real signing values only in local/CI secret stores; never commit them. |

## Remediation completed in this phase

- Expanded `.gitignore` to cover local environment files, signing material, service-account files,
  databases, logs, caches, generated planning/QA workspaces, and temporary files while preserving
  `.env.example`.
- Replaced the previously unused future-sync and browser-demo entries in `.env.example` with a
  value-free statement of the current configuration contract. Neither variable is read by the
  application at this baseline.
- Added environment-variable documentation and the open-source boundary reports.

## Required follow-up before first public release

1. Run `gitleaks detect --redact --source .` and a full-history scanner such as `trufflehog git
   file://$(pwd)` in a trusted release environment. Do not copy findings into an issue or report
   without redaction.
2. Enable GitHub secret scanning, push protection, and Dependabot once a GitHub repository is in
   scope.
3. Re-run this audit after adding CI, signing, release automation, a cloud service, or any sample
   configuration.
4. If a real credential is ever found, revoke or rotate it first; deleting it from the current
   tree is insufficient. Then follow the maintainer-approved history-remediation process.
