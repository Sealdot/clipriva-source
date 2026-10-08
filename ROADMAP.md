# ClipRiva roadmap

This roadmap describes direction, not a fixed delivery commitment. Scope and timing may change based
on user feedback and project resources.

## Current source candidate — 2.0.0-alpha.1

- Local text, image, RTF/HTML, and local file-reference history.
- Search, pinning, retention, recycle bin, safe restore, and optional direct paste.
- Menu-bar lifecycle, global Quick Paste shortcut, launch-at-login control, and privacy-first
  capture rules.
- Local-only Labs experiments, disabled by default.
- Smart Collections, Item notes, and optional local automation with scoped grants.
- Local Link Preview/Alpha, disabled by default; real two-Mac acceptance remains open.

## Next milestone — release readiness

- Measure and tune Quick Paste latency for large retained histories.
- Complete manual compatibility checks in the release matrix.
- Resolve issues found during four consecutive weeks of maintainer daily use.
- Complete the Local Link real two-Mac matrix before promoting it beyond Preview/Alpha.

## Medium-term — stable macOS release

- Developer ID signing and Apple notarization.
- Universal Apple Silicon and Intel binary.
- Clean-machine installation and upgrade verification.
- Compatibility smoke tests on supported macOS versions.
- Published support channel and release checksums.

## Long-term direction

Cross-device sync, MCP, and Agent workflows are not scheduled. A separate product layer can be
considered only after Core v1 is signed and stable, has completed four weeks of maintainer daily use
without an unresolved data-loss or privacy-severity defect, and at least three independent users
repeatedly request the same broader workflow.

Any future layer must use versioned application services. It must not query or synchronize the local
SQLite database file directly.

## Not planned for v1

- Windows or Linux support.
- Accounts, cloud AI, public plugins, or a cloud sync dependency.
- A network connection, model provider, or Agent integration required for Core.

For detailed delivery gates, see [docs/roadmap.md](docs/roadmap.md).
