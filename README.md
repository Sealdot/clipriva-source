# ClipRiva

[简体中文](README.zh-CN.md) | English

ClipRiva is an open-source clipboard history for macOS. Its Core helps you quickly find and
reuse copied text, images, rich text and file references without an account, analytics or network
connection.

ClipRiva Core is the product: a dependable clipboard tool that works fully offline. Optional local
experiments live in **ClipRiva Labs**, are disabled by default and never send clipboard content to a
remote service.

**Local Link v1.5 Release Candidate** is a separate, default-off same-LAN text handoff. Explicit
enable starts the native Bonjour/TCP service; authenticated Noise pairing requires both Macs to
confirm the same safety code, and every received text still requires Copy, Save or Reject. Pending
text stays in native memory for at most 60 seconds. Unit, browser, loopback and single-Mac tests are
not two-Mac evidence, so the feature remains a No-Go for binary distribution or validated real-device use until the 50-pairing /
200-transfer real-Mac gate and privacy review pass. It has no account, cloud relay, offline body
queue, automatic retry or automatic history sync.

> Project status: `2.0.0-alpha.1` is a frozen **source development preview** in
> [Sealdot/clipriva-source](https://github.com/Sealdot/clipriva-source). Source-only prereleases
> contain no installer or application binary. CI and downloaded-source verification are
> commit-specific evidence, linked from each Release. Native UI/installation, real-two-Mac
> Local Link, signing, notarization and real-device acceptance remain unverified. macOS 13+
> is the target; the verified runner is macOS 14 on Apple Silicon.

Read the [public source preview record](docs/audit/source-public-preview-2026-10-08.md)
for security warnings, reproduction and remaining gates. The
[private runner record](docs/audit/source-ci-verification-2026-10-08.md) and
[initial export audit](docs/audit/source-candidate-2026-10-08.md) are historical evidence.
The public repository has independent allow-listed history and contains no old private history.
Binary release requirements are separate; publishing source does not validate Local Link.

## Why ClipRiva

- **Fast keyboard workflow:** open Quick Paste globally, search, navigate and restore a clip without
  opening the main window.
- **Control over sensitive history:** pause capture, exclude applications, set retention limits and
  automatically stop before high-confidence secrets are stored.
- **Useful local formats:** capture and restore text, images, rich text and macOS file references.
- **Transparent by design:** history stays in the local app-data directory and the fixed Tauri IPC
  surface exposes neither arbitrary SQL nor arbitrary filesystem access.
- **No platform prerequisites:** the core clipboard workflow does not require an account, model
  provider, sync service or Agent integration.

macOS Tahoe includes a basic Clipboard view in Spotlight. ClipRiva focuses on the controls and
persistent keyboard workflow a dedicated, inspectable clipboard tool can provide.

## Current features

- Background clipboard capture with best-effort source-application attribution.
- SQLite persistence, exact-version grouping, FTS5 ranking, bounded local tags, pinning, a local
  recycle bin and configurable retention.
- Dynamic Smart Collections with bounded rules, plus explicitly opened, searchable Item notes.
- Conservative local classification for text, code, commands, links and colours.
- Text, PNG image, RTF/HTML and local file-reference capture and restoration.
- Global Quick Paste overlay, menu-bar lifecycle, close-to-tray behavior and opt-in launch at login.
- Configurable Quick Paste and sequential Stack shortcuts; Stack supports collect mode, ordering
  and deliberate next-Item activation without durable queue storage.
- Safe restore mode by default; optional direct paste requests macOS Accessibility permission only
  when the user enables it.
- Application deny-list, manual pause and automatic pause for high-confidence secrets.
- Default-off Local Link candidate with a this-device-only Keychain identity/reset path,
  Bonjour/mDNS discovery, authenticated Noise pairing, bounded text transfer, Incoming Center
  decisions, content-free status history and autonomous pending expiry.
- Browser-only in-memory adapter for UI development without starting Tauri.
- Frontend tests, Rust unit tests, formatting, linting and CI.

## ClipRiva Labs

Labs is disabled by default. Enabling it in Settings reveals deterministic local summaries, an
experimental local retrieval mode and a Filter Composer for one-to-eight allow-listed text steps,
live preview, contextual Filter shortcuts and content-free completion audit.

Local automation is independently disabled by default. When explicitly enabled, the bundled
ClipRiva CLI and Apple Shortcuts “Run Shell Script” action use capability-specific grants over an
owner-only local socket. There is no arbitrary shell, SQL, filesystem, network or Agent/MCP API.

Labs remains offline. It includes no cloud executor, model-provider connection, analytics endpoint,
sync service, MCP server or Agent handoff.

## Install and run

There is no official installer or downloadable app yet. Developers can build ClipRiva from source
on macOS; the resulting unsigned bundle is for local testing. Public signed and notarized binaries
remain a separate release gate.

### Requirements

- macOS 13 or newer
- Node.js 24.15.0 (see `.nvmrc`; dependency engine requirements are enforced during installation)
- pnpm 11.9.0
- Rust 1.97.1 (see `rust-toolchain.toml`)
- Xcode command-line tools

The maintained compatibility matrix covers Ventura 13, Sonoma 14, Sequoia 15 and Tahoe 26 on
Apple Silicon and supported Intel hardware. See the [macOS support policy](docs/macos-support.md)
for the distinction between compile, artifact, real-device and signed-distribution evidence.

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add rustfmt clippy
pnpm install --frozen-lockfile
pnpm desktop:dev
```

Run only the browser development adapter:

```bash
pnpm dev
```

Build an unsigned local application bundle:

```bash
pnpm release:bundle:unsigned
```

The bundle is written to `src-tauri/target/release/bundle/macos/ClipRiva.app`. Because it is unsigned,
it is intended for local development rather than public redistribution.

Get source from the [repository](https://github.com/Sealdot/clipriva-source) or its
[source-only prereleases](https://github.com/Sealdot/clipriva-source/releases).
Verify `SHA256SUMS` before extraction and follow the
[reproduction instructions](docs/audit/source-public-preview-2026-10-08.md).
Use the [issue tracker](https://github.com/Sealdot/clipriva-source/issues) for ordinary bugs;
report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).

## Screenshots

![ClipRiva main workspace](docs/assets/clipriva-main-workspace.png)

The screenshot shows the macOS browser development fixture with synthetic clipboard entries. It
illustrates the interface and is not proof of a signed app or real-Mac Local Link behavior.

## Daily use

- Open Quick Paste with `Command + Shift + Space` by default. The shortcut can be changed in
  Settings.
- Search and navigate with the arrow keys, then press Enter.
- Restore mode puts the selected content back on the clipboard; paste it with `Command + V`.
- Optional direct-paste mode returns focus to the previous application and sends `Command + V`
  after the user grants Accessibility permission.
- Press `Command + K` in the main window to focus history search.
- Clearing history shows a shared impact preview, preserves pinned clips by default and moves
  recoverable items to the local Recycle Bin.

The Chinese [v1.5 getting-started guide and FAQ](docs/help/v1.5-getting-started.md) covers the full
workflow, Local Link setup and troubleshooting. Candidate limitations are tracked in the
[1.5.0 known-issues record](docs/release/known-issues-1.5.md).

## Local data and privacy

On macOS, application data is stored under:

```text
~/Library/Application Support/com.clipriva.desktop/
```

This directory contains `clipriva.sqlite3` and content-addressed local blobs. Quit ClipRiva before
backing up or manually removing the directory. Removing it resets all history and settings and
cannot be undone. The Local Link candidate identity is stored separately in macOS Keychain, so
removing this directory may leave it behind. The Local Link security details include an explicit
identity reset that deletes the Keychain item, invalidates local peer bindings and requires every
device to pair again; it does not delete History. See [PRIVACY.md](PRIVACY.md) before testing
recovery.

- Denied applications are discarded before storage.
- Detected high-confidence secrets pause capture before persistence.
- Labs enrichment is generated only when Labs is enabled or an enabled Labs view requests it.
- Direct paste is optional; restore mode does not need Accessibility permission.
- Core, Labs and local diagnostics have no telemetry, remote service, account or model provider.
- Local Link is the only application network boundary. It starts its native same-LAN service only
  after explicit enable and stops it on disable/reset. It has no ClipRiva cloud relay, Internet
  fallback or offline body queue.

See [PRIVACY.md](PRIVACY.md) for data handling and [SECURITY.md](SECURITY.md) for reporting and
trust-boundary details.

## Development

Use the pinned pnpm workflow for reproducible local dependency resolution:

```bash
pnpm install --frozen-lockfile
pnpm dev
pnpm lint
pnpm typecheck
pnpm test
pnpm build
pnpm lint:rust
pnpm test:rust
```

`npm run dev`, `npm run lint`, `npm run typecheck`, `npm run test`, and `npm run build` expose the
same command names after dependencies are installed. Do not use `npm install` for this repository:
the committed lockfile and declared package manager are pnpm.

Repository structure:

```text
src/                         React application and clipboard UI
src-tauri/src/clipboard/     Platform capture, policy and Quick Paste adapters
src-tauri/src/db/            SQLite repository and migrations
src-tauri/src/media/         Content-addressed local representations
docs/                        Architecture, roadmap and release gates
```

Read [docs/user-stories.md](docs/user-stories.md), [docs/architecture.md](docs/architecture.md),
[ROADMAP.md](ROADMAP.md), [CONTRIBUTING.md](CONTRIBUTING.md), and the
[open-source boundary report](docs/open-source/module-boundary.md) before changing a privacy or
platform boundary.

## Project boundaries

ClipRiva v1 does not include Windows or Linux support, accounts, cross-device sync, cloud AI, MCP,
Agent integration or a public plugin SDK. Those ideas are not active roadmap commitments.

Local Link's explicit one-item request/decision flow is not cross-device history sync. Images,
files, rich text, automatic clipboard mirroring, Internet relay, offline delivery and automatic
retry remain out of scope.

If repeated real-world demand justifies a broader product, it will be developed as a separate layer
that depends on stable ClipRiva Core services instead of accessing the local database directly.

## License

Licensed under the [Apache License 2.0](LICENSE).

## Brand and known limitations

ClipRiva's name and icon policy is defined in [TRADEMARK.md](TRADEMARK.md). The code remains
licensed under [Apache License 2.0](LICENSE); the code license does not grant use of the ClipRiva
name or logos.

Known current limitations:

- macOS 13+ is the only supported platform; Windows and Linux are not v1 commitments.
- Public signed and notarized binaries are not available yet.
- The manual compatibility matrix and large-history Quick Paste latency work are incomplete.
- ClipRiva Labs is experimental, disabled by default, and has no cross-release stability promise.
- Local Link is experimental and disabled by default. Its native pairing and text-delivery path is
  implemented, but it remains a Release Candidate / public-release No-Go until Keychain,
  packet/privacy, sleep/network recovery and two-real-Mac acceptance gates pass.
