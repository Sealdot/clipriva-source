# macOS support policy

ClipRiva targets macOS Ventura 13 and every newer major macOS release. The public support claim is
bounded by the evidence levels below; a successful build on a newer host is not treated as runtime
proof for an older system.

Apple's [macOS version list](https://support.apple.com/109033) is the source for current major and
patch versions. Apple's [Tahoe compatibility list](https://support.apple.com/122867) defines which
Apple Silicon and remaining Intel models can run macOS 26.

Apple's current [security release list](https://support.apple.com/100100) shows ongoing macOS
releases for Tahoe 26, Sequoia 15 and Sonoma 14. Ventura 13 remains ClipRiva's compatibility floor
for existing hardware, not a recommendation to stay on an older OS; users should install the newest
security release their Mac supports.

## Supported version and architecture matrix

| System | Apple Silicon | Intel | Release position |
| --- | --- | --- | --- |
| macOS Ventura 13 | Required real-device minimum-version test | Required real-device minimum-version test | Supported minimum after its release row passes |
| macOS Sonoma 14 | GitHub compile gate + required release smoke test | Universal artifact + release smoke test where hardware is available | Supported after its release row passes |
| macOS Sequoia 15 | GitHub compile gate + required release smoke test | GitHub compile gate + required release smoke test | Supported after its release row passes |
| macOS Tahoe 26 | GitHub compile gate + required release smoke test | GitHub compile gate + required release smoke test on an Apple-supported Intel model | Supported after its release row passes |

GitHub's standard hosted runner set currently provides Apple Silicon runners for macOS 14, 15 and
26, plus Intel runners for macOS 15 and 26. It does not provide a standard macOS 13 runner. The
automated workflow therefore cannot replace the Ventura 13 real-device test.

macOS Monterey 12 and earlier are outside the support boundary. The installer declares macOS 13.0,
so those systems must fail before launch instead of running an untested partial feature set.

## Evidence levels

1. **Source compatible:** frontend production build and native link succeed on that OS/architecture.
2. **Artifact compatible:** the Universal executable contains `arm64` and `x86_64`, both Mach-O
   slices declare macOS 13.0, and the DMG verifies.
3. **Runtime verified:** the exact candidate passes the release row on a real machine or clean VM.
4. **Distribution verified:** the signed and notarized DMG installs through Gatekeeper on a clean
   machine and passes upgrade/relaunch checks.

Only level 3 supports a runtime claim for a system row. Only level 4 supports normal external
distribution. Missing evidence remains `未测`; it is never inferred from a different macOS version.

## Per-version release smoke test

Use synthetic clipboard content and record the candidate commit, DMG SHA-256, OS patch, hardware
model, architecture and result in `docs/beta-validation-matrix.md`.

1. Install, first launch, update over an older ClipRiva build, quit, and reopen from the Dock.
2. Capture and restore text, images, RTF/HTML and file references; verify excluded apps and sensitive
   content fail closed.
3. Exercise Quick Paste by keyboard, Direct Paste permission allow/deny/revoke, clipboard restore,
   launch at login and sleep/wake.
4. Run manual image-text extraction and deletion, including a sensitive-result rejection.
5. With Local Link off, confirm there is no listener or discovery traffic. If the Preview is being
   evaluated, run the separate two-device Local Link gate on the same OS pair.
6. Check VoiceOver, keyboard focus, Reduce Motion, light/dark appearance and the system's current
   permission wording.

The oldest supported release catches availability and WebKit regressions; the latest release catches
new permission, Gatekeeper and UI behavior; Intel rows catch architecture-specific native linking.
