# ClipRiva 2.0 task and evidence protocol

**Status:** M0 fixture and method definition; results belong to the [execution ledger](../planning/clipriva-2.0-execution.md) and candidate evidence directories. Existing acceptance matrices and [Local Link GA gate](../release/local-link-ga-evidence-gate.md) retain their stricter requirements.

## Evidence labels

| Label | Meaning | Can establish |
| --- | --- | --- |
| `browser-mock` | Current React UI with its in-memory synthetic repository or standalone design prototype | Layout, copy, keyboard, and component state only |
| `rust-repository` | `cargo test` on synthetic SQLite/Blob fixtures | Repository and pure state-machine behavior; not target-app delivery |
| `native-dev` | Real Tauri webview and macOS clipboard/window APIs on a disposable profile | Only observed OS/version, content type, and app journey |
| `frozen-signed` | Exact signed/notarized package hash on a recorded device | Install, upgrade, permission and release-candidate behavior |
| `dual-mac` | The same frozen signed package on two real Macs over a real network | Local Link pairing, transfer and lifecycle gate |

Use `passed`, `failed`, `blocked`, and `not_run` literally. `blocked` requires an external condition such as another Mac, signing identity, or owner decision. `not_run` means no valid attempt. Prior logs and browser fixtures cannot change a native or release row to `passed`.

## Synthetic content set

Use a disposable isolated app-data profile and synthetic marker values only. Do not write user clipboard bodies, actual Notes/Collection names, paths, peer names, addresses, or credentials into evidence.

| Case | Item / query | Expected observation |
| --- | --- | --- |
| CH-1 | text `版本规划与体验升级`; queries `版本规划`, `规划` | Both find the Item, with clear scope and stable rank. |
| CH-2 | text `release Release 版本`; queries `release`, `Release` | Case strategy is explicit and source bytes remain unchanged. |
| SY-1 | code `const user_id = "sample-42";`; queries `user_id`, `sample-42`, `"` | Symbols/quotes never reach unsafely built SQL or an exposed FTS expression. |
| SY-2 | URL `https://example.test/a/b?x=1`; queries `a/b?x=1`, `b?x`, `/` | Document supported short/punctuation behavior and any visible bounds. |
| NT-1 | body `例行内容`, Note `发布公告`; query `发布公告` | Label Note match; Copy uses `例行内容`, never Note text. |
| OCR-1 | local synthetic image with manually extracted `图中文字样` | Search finds only after manual OCR; lifecycle follows image. |
| IME-1 | Chinese IME composing `规划` | Composition Enter never invokes Copy or Direct Paste. |
| ERR-1 | invalid/recycled Item, revoked Accessibility, changed frontmost app, full disk (fault injection) | No false success; query/selection kept, no wrong-app dispatch, no Stack advance. |

Include rich text, PNG, color, file reference, Saved, two manual Collections, Smart rule, Note, OCR, and shared Blob in the disposable upgrade fixture. Keep a pre-upgrade consistent snapshot through a verified native database backup or paused writes; never copy SQLite/WAL files during active writes as a supposed backup. Verify settings and relationships before/after migration and rollback from the snapshot only.

## Actual command entry points

Read commands from `package.json`, `src-tauri/Cargo.toml`, and `scripts/` before running them. Current relevant entries are:

```sh
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm release:verify
```

Run release, native, migration, marker-scan and screenshot checks only when their dependencies and target environment are available. `pnpm release:verify` is a source gate, not signed install evidence. The existing 10k release benchmark is ignored by ordinary `cargo test`; run its explicit release command separately when performance work begins, then collect repeated raw samples instead of reporting one benchmark as P95. If manifests/locks change, follow [the SBOM/license audit](../audit/sbom-license-review.md) and dependency inventory. If a privacy/network/permission/stored-data boundary changes, update the required privacy/module documents and boundary tests in the same task.

## Native task script

1. Record full commit, app/package hash, `sw_vers`, architecture, hardware class, build mode, scaling, target app, content type, and whether the profile was new or an upgrade. Do not record a hardware serial number.
2. With a disposable profile, copy two synthetic Items. Open Quick Paste with its actual global shortcut. Capture the search, selected result, More/Preview, Copy outcome, and prior-app return. Verify the system clipboard contains the selected synthetic Item and that History did not gain a false self-write.
3. Repeat for Chinese/URL/code queries; explicitly measure result selection and IME composition. For Direct Paste, separately test permission allow, deny and revoke, focus change, target exit, and unknown dispatch. Record “command sent” only when established; never infer target acceptance from `pasteSent` alone.
4. Capture main workspace, Inspector, settings, capture paused/excluded, Privacy Cover, no result, request failure, unavailable file reference, stale Filter preview, and Local Link Preview-off status. Screenshots must use synthetic content or crop out unrelated user data.
5. With Labs explicitly enabled, run one synthetic text Filter from the main Inspector and one contextual Quick Paste shortcut. Capture Before/After and confirm that preview, Cancel, Escape, a changed rule, a changed/recycled Item, and a failed clipboard write leave the system clipboard and content-free audit unchanged. Confirm once; verify exactly one clipboard write, unchanged original History Item, a content-free completion audit, focus return and no automatic retry. Repeat with a rule change while the preview is open and require another preview. Record the separate state when copy succeeds but audit persistence fails.
6. Test relaunch data/setting retention and Stack clearing. For Local Link, do not count this single-Mac script toward the dual-Mac gate.

## Performance and user tasks

Fix a single device/OS/build mode and report it with each result. For 10k synthetic Items, measure at least 100 hot invocations and 100 searches from native event/query start to interactive result, reporting raw values, P50/P95 and failures. The planning targets are hot invoke P95 ≤150 ms and 10k search P95 ≤200 ms; at 50k, search P95 ≤350 ms. These are targets pending baseline calibration, not present results. Record 30-minute idle CPU/RSS/disk/package size, with any unexplained increase above baseline 15% reviewed. Do not mix devices or dataset sizes into one percentile.

For beta, recruit 10 people for 12 matched tasks each and publish the denominator and failures, including first recovery, unfamiliar search, format reuse, permission degradation, Saved/Collection, cleanup/restore, Stack/Filter, and privacy understanding. A later 14-day controlled trial targets eight users with ten effective-use days each; report recruitment/withdrawal honestly and collect only voluntary content-free task times, error categories, and anonymous counts. No query/body upload or automatic telemetry.

## Manifest, screenshots and commit gate

Each candidate evidence directory records exact commit, app binary/bundle SHA-256, signing/notarization state, OS/architecture, date, command/exit, operator, screenshot/recording path and result. A Local Link directory additionally records both endpoint environments and numbered pairing/transfer attempts without payload or keys. Use filenames containing `browser-mock`, `native-dev`, or `frozen-signed` to prevent evidence mixing. For each CR delivery, include a rollback path and remaining risk.

After creating a candidate commit, run its final relevant checks with no tracked or untracked source changes. If any source changes afterward, create a new candidate commit and repeat relevant checks. Do not claim remote parity without an authorized push of that exact commit and `scripts/verify-git-sync.sh`. No push or release is authorized by this protocol.
