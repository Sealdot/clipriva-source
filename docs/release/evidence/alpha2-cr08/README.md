# CR-08 migration checkpoint

**Test commits:** `f7ec92f` (interrupted v25→v26 checkpoint), `a9e8080` (shared media and synthetic backup), local and unpushed. **Status:** combined synthetic database checkpoint passed; real installed-data upgrade remains open. No migration or retention behavior was changed.

## Baseline and test location

- `git show 91afcd8:src-tauri/src/db/migrations.rs` and current `src-tauri/src/db/migrations.rs` both end at schema 26; `git diff 91afcd8..HEAD -- src-tauri/src/db/migrations.rs src-tauri/src/media` showed no migration/media source changes before this test. The current app still opens a database by applying only migrations newer than its `PRAGMA user_version`, then enforcing the existing retention/recycle policy.
- `src-tauri/src/db/mod.rs` test `migration_26_retries_after_interruption_without_losing_local_facts` creates a disposable v25 SQLite file with synthetic Saved, manual Collection, Smart Collection, Note, manual OCR, custom Filter and preferences. It injects a SQL error inside the v26 transaction, rolls back, confirms schema remains 25, then calls `Database::open` to retry v26. Afterward it checks these facts, settings, schema 26 and foreign-key integrity. The test deletes its own disposable directory on success and never reads a user database.
- The same v25 fixture now contains an active image and a recycled image pointing to one content-addressed PNG Blob. After migration, it checks both references and bytes, restores the recycled row, relaunches, and deletes each row in turn: the Blob survives the first permanent deletion and disappears after the last reference. Before the injected failure, `VACUUM INTO` snapshots the quiescent v25 SQLite database and the Blob is copied through the verified local store. Opening this synthetic backup upgrades its schema and recovers Saved, Note, recycled Item and the original media bytes. This is a manual test fixture, not an implemented user backup feature or a live-WAL backup procedure.
- Earlier repository tests cover individual rich representations, shared Blob lifecycle, recycle/restore, retention, OCR lifecycle and older migrations. Those separate tests do not prove the combined real upgrade fixture described in `docs/testing/clipriva-2.0-task-protocol.md`.

## Executed commands and results

| Command | Result |
| --- | --- |
| `git show 91afcd8:src-tauri/src/db/migrations.rs` with version search; `git diff 91afcd8..HEAD -- src-tauri/src/db/migrations.rs src-tauri/src/media` | **passed** read-only comparison; both latest schema 26, no source diff in the named paths. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked migration_26_retries_after_interruption_without_losing_local_facts -- --nocapture` | **passed**, exit 0; one synthetic interruption/retry test. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | **passed**, exit 0. |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | **passed**, exit 0. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked -q` | **passed**, exit 0; 241 passed / 1 pre-existing ignored benchmark. |
| `git diff --check` | **passed**, exit 0. |

Additional checks for test commit `a9e8080`:

| Command | Result |
| --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked migration_26_retries_after_interruption_without_losing_local_facts -- --nocapture` | **passed**, exit 0; combined synthetic interrupted upgrade, shared Blob/recycle, relaunch and quiescent backup restore. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | **passed**, exit 0. |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | **passed**, exit 0. |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked -q` | **passed**, exit 0; 241 passed / 1 pre-existing ignored benchmark. |
| `git diff --check` | **passed**, exit 0. |

This is a repository test, with no UI screenshot or recording. The command and result are the direct evidence; a browser-rendered database mock or older recording would not validate an upgrade. Final clean-tree rerun after the evidence commit is reported in the handoff.

## Open validation and risk

- **passed, synthetic only:** quiescent v25 SQLite snapshot plus verified Blob copy, schema 26 reopen, shared-reference byte/hash preservation, recycle restoration and last-reference cleanup. This does not validate a live process backup or installed application data.
- **not_run:** actual alpha.1-to-alpha.2 installed app-data relaunch, live-WAL backup coordination, interruption during real process startup and real media/filesystem permission matrix. A real user database is never used as the test fixture.
- **blocked:** signed/notarized package upgrade on a clean Mac, pending owner release decision and package. No `alpha.2` version change, signing, push, tag or release is authorized here.
- The synthetic test builds all migrations through v25, then covers the v25→v26 transaction and selected local facts, not every historic upgrade path or real installed alpha.1 profile. `Database::open` still executes retention/purge on open; any later retention or destructive migration must have a new owner-reviewed plan and combined fixture before implementation.

## Impact and rollback

No new permission, network path, stored data, schema, dependency or migration was added. Test data is synthetic; privacy, security and license impact is none. Rollback: revert `a9e8080` for the shared-media/backup extension or `f7ec92f` for the original checkpoint; neither changes user data.
