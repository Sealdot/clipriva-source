# History at rest protection decision

**Status:** proposed, owner decision required before Core GA.

**Related task:** [CR-09](../planning/clipriva-2.0-execution.md).
**Implementation:** none authorized by this ADR.

## Current verified boundary

The repository stores ordinary clipboard History, searchable indexes, Notes/OCR derivatives and media Blobs locally. `PRIVACY.md` does not claim these are encrypted at rest. Local Link identity material has a separate Keychain boundary; that does not encrypt ordinary History. Privacy Cover omits application presentation; it is not at-rest encryption or a guarantee against all screen capture. Current source tests do not establish protection from an adversary who can read the same user's disk.

This is a source review, not a device or signed-package audit:

- `src-tauri/src/lib.rs` creates/repairs the app-data directory before opening `clipriva.sqlite3`; `src-tauri/src/media/mod.rs` applies owner-only `0700` to its Blob directories and `0600` to Blob files, checks file type and verifies Blob bytes against their content-addressed SHA-256 key. The existing tests exercise these Blob controls.
- `src-tauri/src/db/mod.rs` opens SQLite with WAL and foreign keys, then applies migrations and retention. The repository has no application-level encryption for the main SQLite database, WAL, SHM, FTS data, Notes/OCR text or ordinary Blob bytes. The database opener does not explicitly migrate SQLite/WAL/SHM file modes; `PRIVACY.md` relies on the private parent directory for this boundary. Actual file modes on an installed profile are **not_run**.
- The synthetic CR-08 v25→v26 fixture now checks a quiescent `VACUUM INTO` snapshot and verified Blob copy, restore, shared references and recycle lifecycle. It does not prove a backup taken while the app is capturing or a real alpha.1 installed-data upgrade.

## Threat model for the decision

| Scenario | Assets exposed or protected | Current evidence and limit |
| --- | --- | --- |
| Another local account attempts ordinary filesystem access | History database, WAL/SHM and Blob directory | App-data/Blob directories are tightened to owner-only `0700` on startup in source. Installed-profile ACLs, inherited permissions and actual SQLite sidecar modes still need native inspection. |
| A copied, unencrypted backup or offline disk image is read outside the account's OS protection | SQLite rows, FTS indexes, Notes/OCR, source metadata and Blob bytes | No application-level at-rest encryption. OS disk/backup encryption may help where configured, but ClipRiva does not enforce or validate it; this scenario remains exposed in the product claim. |
| Another process already runs with the same user's authority while ClipRiva is unlocked | Application files, clipboard, WebView/process memory and any available key | File modes and a future at-rest key cannot by themselves prevent this process from reading accessible plaintext. Treat this as a separate endpoint-compromise boundary, not an encryption success criterion. |
| Crash, migration or restore leaves an extra copy | SQLite WAL/SHM, search indexes, temporary database, copied Blob and backup | Existing synthetic migration/backup tests cover a quiescent fixture only. Real startup interruption, live-WAL backup coordination, plaintext remnants and installed upgrade are **not_run**. |

The protected data set for any future at-rest scheme must include the database, WAL/SHM, FTS/search text, Item Notes, manual OCR, ordinary media Blobs, temporary files and backups. Encrypting only the visible Item body would leave searchable derivatives and representations readable.

## Decision to make

| Choice | Impact | Release consequence |
| --- | --- | --- |
| Keep current local storage, harden permissions/retention and disclose residual risk | Small migration surface; device/user filesystem access can still expose local History, indexes and Blobs. Verify directory/file modes, diagnostics, cleanup, excluded apps and Cover. | Owner may allow Core GA only after a documented threat model and explicit residual-risk acceptance. |
| Block GA pending essential protection | More time and migration complexity; prevents a known unacceptable exposure from being hidden by wording. May narrow feature/support scope. | Core GA remains No-Go until the protection and its upgrade/failure tests pass. |
| Research full at-rest encryption as a later candidate | Requires an evaluated scheme for DB, WAL, FTS, Blob, Notes/OCR, temporary files, backup/checkpoint and key recovery. “Encrypt body only” is insufficient. | Does not by itself authorize 2.1 or permit the current Core GA; schedule follows threat and user evidence. |

Before choosing encryption, answer: when is the key available during login, lock, background capture and restart; what happens when it is unavailable; how are device move/key loss handled; how are upgrade/downgrade/exit atomic; and where might plaintext/index remnants remain? Use established cryptography and native key management only after review. No new OS permission, key policy or destructive migration is approved here.

## Required decision record

- Threat actors, protected assets and the same-user/process limit: **source-reviewed above**; installed-device evidence remains **not_run**.
- Decision and rationale: **pending owner confirmation**.
- Validation method: inspect owner/ACL modes on a disposable installed profile; exercise lock/restart, retention and cleanup, interrupted migration, clean backup/restore and plaintext/index/remnant scans with synthetic canaries. For encryption, add key-unavailable, key-loss, device-move, rollback/downgrade and atomic-upgrade cases before implementation is accepted. Native and signed-package checks remain **not_run**.
- Migration and recovery path for any encryption choice: **pending design and owner approval**; no destructive migration is authorized by this ADR.
- Remaining risk and owner acceptance: **pending**.
- Core GA: **blocked until `allow` or `block` is explicitly recorded**.

The user must review choices and impact before an encryption implementation or release decision. The existing app remains an alpha engineering candidate while this is proposed.
