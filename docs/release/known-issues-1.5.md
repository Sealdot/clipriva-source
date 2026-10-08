# ClipRiva 1.5.0 release-candidate known issues

- **Status:** private validation; public distribution and Local Link promotion are **No-Go**.
- **Version:** `1.5.0`.
- **Scope:** candidate limitations that remain after the v1.5 engineering implementation.

## Open release blockers

1. Local Link has not completed the required two-real-Mac evidence: 50 pairing ceremonies, 200 text
   transfers, Copy/Save/Reject coverage, physical sleep/wake, lock/user switch, network changes,
   packet inspection, or SQLite/WAL/log marker scans.
2. The unsigned Universal Intel/Apple Silicon artifact has passed binary metadata and DMG checks,
   but Developer ID signing, Apple notarization, Gatekeeper assessment, clean installation, upgrade
   and uninstall checks are not complete.
3. The macOS 13, 14, 15 and 26 compatibility matrix, multi-run Quick Paste release measurements,
   VoiceOver checks and the four-week maintainer daily-use gate remain incomplete.
4. Public download instructions, signed artifact checksums and a supported public contact channel do not
   exist yet.

## Completed candidate evidence

- Candidate source was frozen at `cfa2ead91bffb90bcd754e5c6356590b26cf2f0a`.
- A detached clean clone passed the frozen install, browser start, `release:verify`, the 10k-item
  release benchmark and `release:bundle:unsigned` on macOS 14.2.1 / Apple Silicon.
- The unsigned internal-test archive SHA-256 is
  `f824faed1e306f683ea3b75d007f928f6e79c5c80a87b36f600b0de670a1efc9`.
- A desktop launch under an isolated macOS test user was not available and remains part of the clean
  installation gate; the unsigned bundle is not approved for distribution.

## Required behavior while blockers remain

- Keep Local Link default off and labeled Preview/Alpha; do not describe loopback, browser, unit or
  single-Mac evidence as real-device evidence.
- Treat any plaintext fallback, incorrect trust, duplicate Copy/Save, post-revocation access,
  persisted undecided body, or failed transfer reported as delivered as an absolute No-Go.
- Do not publish, tag, sign, notarize or distribute this candidate without the release owner's
  separate approval and the completed release checklist.
