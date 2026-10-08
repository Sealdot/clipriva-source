# CR-10 Local Link Core isolation source assessment

**Assessed source:** `7b29687` (`2.0.0-alpha.1`), local and unpushed. **Status:** source feasibility assessment only; no Core stable candidate, signing, network permission change or release decision. This document records no real-Mac transport acceptance.

## Source boundary

| Location | Verified source behavior | Consequence for a Core stable candidate |
| --- | --- | --- |
| `src-tauri/src/db/migrations.rs`, `MIGRATION_13`/`MIGRATION_15`; `src-tauri/src/db/mod.rs`, `migration_15_disables_network_consent_and_downgrades_preview_trust` | New data starts with Local Link off. The one-time v15 upgrade resets an older control-plane Preview `enabled`/`discovery_enabled` pair and downgrades Preview trust. | This protects new installs and the specific old schema transition. It does not reset a current schema-26 profile that has subsequently enabled real transport. |
| `src-tauri/src/lib.rs`, `setup` | Every current app startup constructs `LocalLinkService`, runs native Copy-effect recovery and installs its lifecycle monitor. Tauri registers Local Link commands in the same app. | A UI-only hide would leave the native service and IPC path in the package. Core capture/History/Quick Paste do not otherwise require Local Link, but that independence needs package and native smoke evidence. |
| `src-tauri/src/local_link.rs`, `LocalLinkService::new` and `TransportRuntime::start_inner` | A saved current `enabled=true` preference triggers lifecycle startup. Transport binds TCP on `0.0.0.0` and starts Bonjour discovery when the supervisor permits it. Disabling drops transport and clears pending state. | Prior current-version consent persists across relaunch. A proposed stable Core package cannot claim isolation solely from the default-off or v15 migration tests; it must remain off even for an upgraded profile with `enabled=true`. |
| `src-tauri/Info.plist`; `src-tauri/capabilities/default.json` | The bundle source declares `NSLocalNetworkUsageDescription` and `_clipriva._tcp`; Tauri WebViews have Core/clipboard permissions and no raw-socket capability. | Native Rust can still own its TCP listener. Built Info.plist, entitlements, capabilities and real no-listener/no-mDNS behavior must be checked on the exact frozen package. |

The current package is a combined alpha/Preview source tree, not an isolated Core stable package. The source findings do not show an unsolicited listener on a new install; they show why the existing default-off behavior alone is insufficient for the proposed stable-package exception to the Local Link gate.

## Choices requiring release-owner approval before implementation

1. **Keep one combined alpha/Preview package.** Preserve the current explicit Local Link control and defer any public stable claim. This avoids a packaging fork, but the Core stable exception is unavailable while the real two-Mac Local Link matrix remains open.
2. **Build a Core-only stable variant.** Compile out or otherwise make the Local Link transport/commands unreachable, omit unnecessary Local Network/Bonjour declarations from that variant, and retain existing History, Saved, Collections, Notes, settings and dormant Local Link records without a destructive migration. This offers the strongest source isolation, but needs build configuration, upgrade and bundle-identity review plus a clean signed package and native no-listener proof. A different bundle identifier or data-location policy would be a separate decision.
3. **Keep one binary with a stable-channel native kill switch.** It would have to reject every Local Link command that could start transport, ignore a persisted `enabled=true` at startup without erasing the user's settings, and prove no listener, Bonjour browse/advertise, session or Keychain side effect on fresh and upgraded profiles. The code and network declarations would remain in the binary, so source and package review is harder and release approval cannot rest on a hidden UI toggle.

No choice is selected here. An isolation implementation would change network/permission or release behavior and therefore requires the owner's explicit decision and the corresponding privacy/module-boundary review before code changes.

## Checks actually run

| Command | Result and limit |
| --- | --- |
| `node scripts/check-release-version.mjs` | **passed**, exit 0; package, Cargo manifest/lock and Tauri all report `2.0.0-alpha.1`. Version consistency is not a release gate result. |
| `pnpm exec vitest run src/features/clipboard/LocalLink.test.tsx src/features/clipboard/LocalLink.browserRepository.test.ts` | **passed**, exit 0; 2 files / 33 browser-fixture tests. These do not bind a native listener or use two Macs. |
| `plutil -lint src-tauri/Info.plist` and `plutil -extract NSBonjourServices json -o - src-tauri/Info.plist` | **passed**, exit 0; plist valid, source declaration is `["_clipriva._tcp"]`. The built/signed bundle was not inspected. |
| `rg -n` source inspection of migration, startup, bind and plist locations | **passed**, exit 0; exact locations are in the table above. Source inspection is not packet or permission evidence. |
| `df -h .` | **observed** about 1.5 GiB free during this audit, below the release checklist's requirement for at least 10 GiB beyond existing build output. |

`pnpm release:verify`, unsigned Tauri bundling, clean install/upgrade, native TCP/mDNS packet capture, Info.plist/entitlement inspection of a built artifact, Keychain lifecycle, signing, notarization, Gatekeeper, dual-real-Mac 50-pairing/200-transfer matrix, lifecycle/security checks and same-package hash are **blocked/not_run**. No CR-10 screenshot or recording was produced; a browser mock or historical release record would not establish native network isolation. Local Link remains independent Preview/No-Go, and this source assessment does not authorize beta, a public release, tag or push.

**Privacy/security/license impact:** documentation only; no runtime, permission, network, dependency, asset, stored-data or license change. The material risk is a current upgraded profile with saved `enabled=true` relaunching transport in a combined package. Source-assessment rollback is the evidence-document commit only; no user data was changed.
