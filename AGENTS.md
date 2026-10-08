# ClipRiva change-automation rules

These repository rules apply to every future Codex or contributor change. They supplement the
project's local-first and open-source boundaries; they never authorize publication, a tag, a
release, a push, signing, or notarization.

## Required change follow-ups

| Change detected | Required follow-up before the task is complete |
| --- | --- |
| `package.json`, `pnpm-lock.yaml`, `src-tauri/Cargo.toml`, or `src-tauri/Cargo.lock` changes | Update the direct-dependency inventory when a direct dependency changes. Re-run the Node and Rust license/SBOM review described in `docs/audit/sbom-license-review.md`; record new license, missing-metadata, or restricted-license findings. |
| A network request, account, telemetry, error reporting, sync, remote model, data export, new stored data type, or OS permission changes | Update `PRIVACY.md`, `docs/privacy/data-flow.md`, and `docs/open-source/module-boundary.md` in the same task. Add or update tests for the changed boundary. Do not claim a capability is local-only unless the implementation and tests support it. |
| An icon, screenshot, illustration, font, audio asset, copied code, or other third-party material changes | Record its provenance in `THIRD_PARTY_NOTICES.md` or the relevant audit document. Do not add it until its redistribution right is known. Reconfirm the ClipRiva brand rule for name or logo changes. |
| Environment configuration, deployment endpoint, signing material, sample data, fixture, or documentation changes | Keep real credentials, user data, production endpoints, local personal paths, and private keys out of the repository. Use clearly synthetic fixtures. If a secret-like value is found, stop, redact it from reports, and follow `SECURITY.md`; do not publish, rotate, or rewrite history without maintainer authorization. |

## Completion checks

1. Run the checks relevant to the changed area and report any intentionally skipped check.
2. State the privacy, security, and license impact in the task handoff or pull-request description.
3. Preserve focused commits; do not combine these documentation updates with unrelated feature work.
4. Before an eventual public release, follow the mandatory gates in
   `docs/release/release-checklist.md`. A release candidate must be re-audited after the final
   feature merge.
5. Treat validation as commit-specific evidence. Run the final relevant checks with no tracked or
   untracked source changes after the candidate commit is created. If the task explicitly authorizes
   a push, push that exact commit and run `scripts/verify-git-sync.sh` before claiming that local and
   remote are synchronized. Without push authorization, report the local commit as unpushed and do
   not claim remote parity.

For the contributor-facing explanation and examples, see
`docs/development/open-source-change-rules.md`.
