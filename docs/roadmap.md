# ClipRiva delivery roadmap

> The active 2.0 alpha.2 work is tracked in the [single execution ledger](planning/clipriva-2.0-execution.md); this roadmap remains historical context.

This roadmap keeps ClipRiva Core useful as a complete open-source clipboard tool. ClipRiva Labs is
an optional proving ground, not a dependency of the core workflow.

## v0.1 — Open-source beta

- [x] Local text and image clipboard history.
- [x] Search, pin, copy, delete and clear-unpinned behavior.
- [x] Global Quick Paste, menu-bar lifecycle and launch-at-login control.
- [x] Retention limits, application deny-list and sensitive-content pause.
- [x] Apache-2.0 license, security policy and source build instructions.
- [x] Labs setting persisted as disabled by default.
- [x] First-run privacy explanation.

Exit condition: a developer can build ClipRiva on macOS 13+, understand what it stores and use the
core workflow without seeing experimental or future-platform concepts.

## v0.x — Daily-driver completion

- [x] RTF/HTML capture and restoration through the macOS pasteboard.
- [x] Local file-reference capture and restoration without copying file contents.
- [x] Configurable Quick Paste shortcut.
- [x] Restore-only default and opt-in direct paste with Accessibility fallback.
- [ ] Measure and tune Quick Paste latency with large retained histories.
- [ ] Complete manual compatibility checks in the release matrix.
- [ ] Resolve issues found during four consecutive weeks of maintainer daily use.

Exit condition: ClipRiva can replace a basic clipboard manager for the maintainer and external beta
testers without data-loss or privacy-severity defects.

## v1.5 — Private release candidate

- [x] Conservative Command classification and bounded, local user tags.
- [x] Recent-activity ordering for zero-input Quick Paste.
- [x] Default-off Local Link v3 engineering implementation and automated regression gate.
- [x] Package, Cargo and Tauri versions aligned at `1.5.0`.
- [x] Complete a clean-checkout unsigned bundle for the frozen candidate SHA.
- [ ] Complete the 50-pairing / 200-transfer real-two-Mac matrix and privacy scans.

Exit condition: the exact candidate SHA passes the clean build and real-device gates without any
absolute No-Go finding. Until then, Local Link remains default off and Preview/Alpha.

## v1.x — Stable macOS release

- [ ] Developer ID signing and Apple notarization.
- [ ] Universal Apple Silicon and Intel binary.
- [ ] Clean-machine installation and upgrade verification.
- [ ] Smoke tests on macOS 13, 14, 15 and 26.
- [ ] Published support channel and release checksums.

Exit condition: a non-developer can install a verified build and depend on the documented core
behavior.

## ClipRiva Labs

Labs currently contains deterministic local summaries, experimental local retrieval and allow-listed
text actions. It is disabled by default, remains offline and carries no cross-release stability
promise.

Labs work must not add network access, account requirements or reliability dependencies to Core.

## Exploration gate

Cross-device sync, MCP and Agent workflows are not scheduled. A separate product layer may begin
only after all of the following are true:

1. ClipRiva Core v1 has a signed, notarized stable release.
2. The maintainer has used Core daily for four consecutive weeks without unresolved data-loss or
   privacy-severity issues.
3. At least three independent users repeatedly request the same cross-device or Agent workflow.

Any future layer must use versioned application services. It must not query or synchronize the
SQLite database file directly.
