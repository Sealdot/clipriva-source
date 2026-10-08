# CR-09 at-rest risk decision preparation

**Status:** source review passed; owner decision and physical validation remain open. This document does not approve an encryption scheme, migration, GA release or 2.1/2.2 work. The decision choices and their effects are in [the ADR](../../../adr/history-at-rest-protection.md).

## Delivery and source locations

- `docs/adr/history-at-rest-protection.md`: source-based assets and threat scenarios, same-user/process limit, current owner-only directory/Blob controls, unencrypted ordinary History boundary, choices and validation needs.
- `src-tauri/src/lib.rs:171-173`, `src-tauri/src/db/mod.rs:218-225`, `src-tauri/src/media/mod.rs:152-191,339-386`, and `PRIVACY.md:79-82,124-133`: reviewed source and disclosure. No product code, key handling or data format changed.
- `docs/release/evidence/alpha2-cr08/README.md`: separate synthetic v25→v26 backup/restore and shared-Blob evidence. It does not prove native at-rest protection or live backup safety.

## Executed source checks

| Command | Result |
| --- | --- |
| `rg -n 'journal_mode\|Connection::open\(\|ensure_private_directory' src-tauri/src/lib.rs src-tauri/src/db/mod.rs` | **passed**, exit 0; app-data directory is repaired before SQLite open, and SQLite uses WAL. |
| `rg -n 'sqlcipher\|PRAGMA key\|cipher_page_size\|sqlite.*encrypt\|encrypted.*sqlite' src-tauri/src/db src-tauri/Cargo.toml` | Exit 1, no matches for these encryption hooks in the scoped database source/dependency manifest. This is supporting source evidence, not a cryptographic audit of a built binary. |
| `rg -n '0700\|0600\|SQLite file-mode\|not documented as encrypted' PRIVACY.md` | **passed**, exit 0; current disclosure matches the scoped source review. |
| `git diff --check` | **passed**, exit 0. |

## Evidence limits and decision gate

This is a document/source review, so no product screen or recording could establish the disk boundary. Installed SQLite/WAL/SHM modes and ACLs, OS lock/restart behavior, live backup coordination, plaintext remnants, and signed-package behavior are **not_run**. The synthetic CR-08 test is not substituted for these. The ADR remains **blocked** on owner selection and residual-risk acceptance before any encryption strategy, destructive migration or Core GA decision. Alpha engineering can continue under the existing disclosed boundary.

Privacy/security impact: no runtime change and no stronger confidentiality claim. License impact: no dependency, code or third-party asset added. Rollback: revert the ADR and evidence documentation commit after checking later references; no user data is affected.
