# ClipRiva

[简体中文](README.zh-CN.md) | English

**Find and reuse something you copied earlier.** ClipRiva is a macOS clipboard-history tool:
copy two pieces of text, open Quick Paste, search for the earlier one, and press Enter to put it
back on the system clipboard. Then paste it in your original app with `Command + V`.

The core workflow keeps History on your Mac and needs no account or remote service. You can also
browse copied text, images, rich text and file references in the History workspace.

## Public source preview

This repository, [Sealdot/clipriva-source](https://github.com/Sealdot/clipriva-source), publishes
**source code and source packages**, not a stable app or a signed and notarized installer.

- [Browse the current source](https://github.com/Sealdot/clipriva-source/tree/main) or use the
  development paths below. `main` includes updates made after the frozen preview.
- [Download Source Development Preview 1](https://github.com/Sealdot/clipriva-source/releases/tag/v2.0.0-alpha.1-source-preview.1):
  application version `2.0.0-alpha.1`, source commit `e4366664163ad51d2deb3276fb28ef4e1114b5dd`.
  The Release links its source CI, download verification, checksums and reproduction instructions.
- Native installation/UI, the broader real-device macOS compatibility matrix, signing/notarization and real
  two-Mac Local Link acceptance remain unverified. Source checks do not establish those outcomes.

macOS 13+ is the native target. The frozen preview’s source CI used macOS 14 on Apple Silicon;
see the [support policy](docs/macos-support.md) for the limits of that evidence.

## Interface examples

**Browser preview · synthetic sample data · not proof of native operation.** Both images are
unmodified, project-owned captures of the actual React UI already published in this repository.
They do not demonstrate system clipboard capture, global shortcuts, focus return or two-Mac delivery.

**Quick Paste — search earlier clips and choose Copy to clipboard.**

![Quick Paste with synthetic clips, search and Copy to clipboard footer](docs/release/evidence/alpha2-cr02/screenshots/quick-light-browser-mock.png)

**History workspace — browse and organize what you want to reuse.**

![History workspace with synthetic clips, Saved and Collections](docs/release/evidence/alpha2-cr05/screenshots/workspace-list-browser-mock.png)

Capture context: [Quick Paste](docs/release/evidence/alpha2-cr02/README.md),
[workspace](docs/release/evidence/alpha2-cr05/README.md), and
[asset provenance](THIRD_PARTY_NOTICES.md#assets-and-system-components). These historical visual
records are separate from current release acceptance.

## Three ways it helps

- **Recover the earlier copy.** Quick Paste search and keyboard navigation help you find a clip
  after another copy has replaced it on the system clipboard.
- **Keep useful material close.** History, Saved and Collections let you organize snippets for
  later reuse; images, rich text and file references are also supported by the native adapters.
- **Choose what stays on this Mac.** Pause capture, exclude applications, adjust retention and
  review cleanup before removing History. Local storage still contains data; review
  [PRIVACY.md](PRIVACY.md) before using sensitive content.

## First use: recover one earlier text

After starting the native development app using the macOS path below:

1. Complete onboarding, keep capture active and use the default clipboard restore behavior.
2. In a text editor, copy `Sample note: bring a notebook`, then `Sample note: confirm the room`.
   Confirm both ordinary text samples appear in History.
3. Press `Command + Shift + Space` (the configurable default) to open Quick Paste. Search for
   `notebook` and select the earlier sample with the arrow keys.
4. Press Enter. **Expected result:** the earlier sample is restored to the system clipboard.
   Return to your editor and press `Command + V` to paste it yourself.

Enter performs Copy/restore. **Direct Paste is an optional separate action** that requires macOS
Accessibility permission and attempts to return focus and send a paste keystroke. The target app
may not accept the keystroke; permission or target failures can fall back to clipboard restoration.
Clipboard restore itself does not need Accessibility permission.

If a sample is missing, check capture pause, the excluded-app list and the search term. The
[first-use guide and FAQ (Chinese)](docs/help/v1.5-getting-started.md) cover further troubleshooting.

### Get source and choose a development path

For the current public branch, run from a directory where you keep source checkouts:

```sh
git clone https://github.com/Sealdot/clipriva-source.git
cd clipriva-source
```

For the frozen source package, download all five attachments from the
[preview Release](https://github.com/Sealdot/clipriva-source/releases/tag/v2.0.0-alpha.1-source-preview.1)
into one directory: `ClipRiva-source.tar.gz`, `SOURCE_RECORD.json`, `SHA256SUMS`,
`REPRODUCE.md` and `CI-evidence.zip`. In that download directory:

```sh
shasum -a 256 -c SHA256SUMS
tar -xzf ClipRiva-source.tar.gz
cd ClipRiva-source
```

Run subsequent commands in the source root containing `package.json` and `src-tauri/`.
Use the toolchain pins **in the checkout you selected**: `.nvmrc`, `package.json`'s
`packageManager`, `rust-toolchain.toml`, `pnpm-lock.yaml` and `src-tauri/Cargo.lock`.
Current `main` pins Node.js **24.15.0**, pnpm **11.9.0** and Rust **1.97.1**. The frozen
October 8 package predates later dependency/runtime updates; follow its included `REPRODUCE.md`
and pins rather than substituting current versions. Use `pnpm install --frozen-lockfile`,
not `npm install`. No application `.env` configuration is required.

**Path A — browser UI preview.** With the selected Node.js and pnpm versions installed:

```sh
pnpm install --frozen-lockfile
pnpm dev
```

Open [History at 127.0.0.1:1420](http://127.0.0.1:1420/) or
[Quick Paste preview](http://127.0.0.1:1420/?quick-paste-preview) yourself.
Expected result: the real interface backed by in-memory **synthetic sample data**, reset on reload.
It does not monitor your system clipboard or register global shortcuts. Copy can request a browser
clipboard write, subject to browser permission; simulated native and Local Link states are not
macOS or two-device validation. Rust and Xcode are not needed for this path.

**Path B — macOS native development.** On macOS 13+, install Xcode Command Line Tools
(`xcode-select --install` if absent), the selected Node.js/pnpm versions, and Rust via rustup.
`rust-toolchain.toml` selects Rust, rustfmt and Clippy when Cargo runs from the source root.

```sh
pnpm install --frozen-lockfile
pnpm desktop:dev
```

Expected result: Tauri compiles and starts the local development app with native clipboard
adapters. This command opens app windows; it is not an installer or signing/notarization check.
Use ordinary synthetic text for the first-use exercise. Unsigned bundle instructions and full
checks belong to the [development guide](CONTRIBUTING.md) and [release guide](docs/release.md).

## Experiments and data-handling limits

- **Labs and local automation are independently off by default.** Optional local summaries,
  retrieval, text filters and CLI/Shortcuts capabilities are experiments. Manual image OCR uses
  macOS Vision. Implementation boundaries are in [the architecture](docs/architecture.md) and
  [module-boundary document](docs/open-source/module-boundary.md).
- **Local Link is off by default and remains Preview / No-Go.** Explicit enable starts local
  network discovery/transport. It is an explicit **single-text handoff** to a paired Mac: the
  receiver chooses Copy (clipboard only), Save (History only) or Reject. It is not automatic
  clipboard mirroring or History sync, has no cloud relay or offline delivery queue, and has not
  completed real two-Mac validation. See [the evidence gate](docs/release/local-link-ga-evidence-gate.md).
- **Local does not mean nothing is recorded.** History and related metadata live in local
  SQLite/blobs; encryption at rest is not claimed. Optional diagnostics are off by default and
  retain bounded event/result metadata, day and app version when enabled, excluding clipboard
  bodies and identifying fields. Privacy Activity separately retains bounded content-free capture
  reasons and may include a source-app label. Review [data handling](PRIVACY.md) and
  [data flows](docs/privacy/data-flow.md).
- **Sensitive-content detection is limited.** Detected high-confidence secrets are skipped before
  storage; Strict mode also pauses subsequent capture until you resume it. This is not a guarantee
  that every secret will be detected. App exclusions do not erase existing History. Privacy Cover
  hides the interface while History remains stored and capture continues unless paused; it is
  neither disk encryption nor a system-wide recording block.

## Detailed documentation

| Topic | Where to read |
| --- | --- |
| Development, checks and contribution rules | [Contributing](CONTRIBUTING.md), [change rules](docs/development/open-source-change-rules.md) |
| Core, Labs and Local Link implementation | [Architecture](docs/architecture.md), [module boundaries](docs/open-source/module-boundary.md), [Local Link protocol](docs/local-link/PROTOCOL-v1.5.md) |
| Privacy and security reporting | [Privacy](PRIVACY.md), [Security](SECURITY.md) |
| Source verification and historical audits | [Public preview record](docs/audit/source-public-preview-2026-10-08.md), [SBOM/license review](docs/audit/sbom-license-review.md) |
| Native support and release gates | [macOS support](docs/macos-support.md), [release checklist](docs/release/release-checklist.md), [Local Link evidence](docs/release/local-link-ga-evidence-gate.md) |
| Product direction and ordinary bugs | [Roadmap](ROADMAP.md), [public issues](https://github.com/Sealdot/clipriva-source/issues) |

Code is licensed under [Apache-2.0](LICENSE). See [third-party notices](THIRD_PARTY_NOTICES.md)
and [the ClipRiva name/logo policy](TRADEMARK.md) for asset and brand terms.
