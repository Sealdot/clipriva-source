# ClipRiva v1.3 Design QA

Date: 2026-07-30

Reference source: maintainer-supplied `ClipRiva-v1.3-产品优化方案（含配图）.docx` and its embedded Quick Paste, Devices, and pairing mockups.

Implementation reviewed: `http://127.0.0.1:1420/` with the browser-only `local-link-ready` visual fixture. This fixture is presentation evidence only and is not physical LAN or two-Mac evidence.

## Source truth and evidence

| Surface | Reference | Post-fix implementation | Viewport / state |
| --- | --- | --- | --- |
| Quick Paste send | DOCX embedded image 6 (`/tmp/clipriva-v13-review/media/image6.png`) | `/tmp/clipriva-v13-qa/quick-paste-1600x1000-final.png` | 1600 × 1000, selected text, sidecar open, most-recent sendable device selected |
| Device center | DOCX embedded image 7 (`/tmp/clipriva-v13-review/media/image7.png`) | `/tmp/clipriva-v13-qa/devices-1600x1000-final.png` | 1600 × 1000, ready, trusted-online device selected, pending card collapsed |
| Pairing | DOCX embedded image 8 (`/tmp/clipriva-v13-review/media/image8.png`) | `/tmp/clipriva-v13-qa/pairing-1600x1000-final.png` | 1600 × 1000, step 2 of 3, matching SAS/fingerprints, 30-day trust selected |
| Narrow desktop | v1.3 UI-02 acceptance case | `/tmp/clipriva-v13-qa/devices-760x520-final.png` and `/tmp/clipriva-v13-qa/quick-paste-760x480.png` | 760 × 520 / 760 × 480, no horizontal overflow |

All three source/implementation pairs were viewed together after the final fixes. Source and desktop implementation comparisons used the same 1600 × 1000 CSS viewport at DPR 1. The narrow checks measured `documentElement.scrollWidth === innerWidth`.

## Comparison summary

| Surface | Visual and interaction check | Result |
| --- | --- | --- |
| Readiness | One ordered conclusion appears before device management. Ready, action-required, and offline states use the existing mint/amber/neutral system; only the first blocker exposes one action. Five checks preserve the requested permission → enablement → discovery → trust → authenticated-presence order. | Pass |
| Devices | The final layout matches the reference hierarchy: readiness first, then a left device master list and right selected-device detail. Trusted-online is selected first; offline and blocked devices remain visible but are not sendable. Fingerprint, trust duration, activity, transfer, and trust actions fit without collision. | Pass |
| Quick Paste | The selected clip remains visible while the right sidecar opens. Device choices show sendable, offline, and pair-first states. The final sidecar is vertically centered and 510 px wide at the reference viewport; one Enter / Send now action creates the request, while Escape cancels. | Pass |
| Pairing | Centered modal, dimmed background, three-step progress, shared six-digit SAS, both fingerprints, 60-second timer, three trust durations, explicit mismatch, cancel, local confirm, and remote waiting states are visible and keyboard reachable. | Pass |
| Pending receive | Devices uses an inline pending request. Escape/X collapses without discarding; only Reject discards. Copy and Save remain explicit, mutually exclusive destination choices. | Pass |
| Diagnostics | Preview is user-triggered and appears before Download JSON. The UI states the 50-record maximum and excluded content/identity/network fields. No automatic download was observed. | Pass |

## Typography, spacing, color, assets, and copy

- Typography and spacing follow the existing ClipRiva shell. Headings, eyebrow labels, status metadata, SAS digits, and fingerprints remain distinct at 1600 × 1000 and the supported narrow sizes.
- Card borders, radii, and padding align with the supplied dark, restrained reference. Mint is reserved for ready/trusted/primary states; amber/red remain attention/failure signals; blue is informational.
- The implementation uses the repository's existing Lucide icon family. No reference image, generated art, font, or third-party runtime asset was copied into the product.
- Copy keeps the safety boundary explicit: online is not trust; send is explicit; receive chooses Copy, Save, or Reject; diagnostics and activity exclude clipboard text.
- Focusable controls retain visible focus rings, dialogs have accessible names, state is not conveyed by color alone, and reduced-motion behavior remains unchanged.

## Iteration record

1. Initial 920 × 620 review found a P1 horizontal overflow/top-bar collision and the first-value guide overlapping the Devices task. Responsive navigation/search rules were tightened and the guide is now suppressed on Devices.
2. The first post-fix Devices view still placed recent sends and management ahead of the reference's master-detail device view. A dedicated device master-detail surface was added above recent activity.
3. Browser verification then found the blocked online discovery record selected ahead of the trusted-online device. Device priority now selects trusted + online first, then trusted offline, re-pairing, and blocked records.
4. The first 1600 × 1000 Quick Paste comparison stretched the sidecar through nearly the entire viewport and made it too narrow. It is now vertically centered and widened to 510 px, matching the reference proportions while remaining bounded on narrow windows.
5. Source and implementation were compared together again after each fix. The final 1600 × 1000 Devices, Quick Paste, and pairing states have no remaining P0/P1/P2 fidelity issue.

## Runtime checks and remaining scope

- Navigation, device selection, pending collapse, pairing start/cancel, Quick Paste sidecar open, diagnostics preview, and the keyboard-oriented send state were exercised.
- Final browser logs contained no `error`, `warning`, or `warn` entries on the Devices/pairing and Quick Paste tabs.
- P0: none.
- P1: none.
- P2: none.
- This pass validates implementation fidelity, browser interaction behavior, and fail-closed presentation. Bonjour discovery, bilateral trust, encrypted delivery, timing, sleep/wake, and leak checks still require the documented physical two-Mac Beta Gate.

final result: passed

---

# ClipRiva 2.0 Design QA — Native Library Workspace

Date: 2026-08-12

Selected direction: option 2, a native library workspace with a source rail, a focused History
list and a stable inspector. The comparison reference and final captures are retained in the local
design-QA artifact bundle as `exec-426347fe-3e48-4ba2-af82-bd03cd5ca303.png`,
`implemented-passed-1280x720.png` and `implemented-minimum-720x480.png`. They are review evidence,
not runtime product assets, and are intentionally not copied into the repository.

## Review matrix

| Surface | Final behavior | Result |
| --- | --- | --- |
| Main workspace | History, Saved and Collections form the primary source rail; the center list owns search and structured filters; the right inspector stays visually stable at normal desktop sizes. | Pass |
| Visual hierarchy | The active source, selected card and primary inspector Copy action have distinct emphasis. Secondary actions live in one More panel instead of competing across each card. | Pass |
| Quick Paste | The footer exposes exactly Navigate, Restore and More. Type filters and secondary actions are available without eight persistent chips, top-level Send or row-level Pin controls. | Pass |
| Daily reuse | Existing pins/tags are presented as Saved/Collections. Filter combinations run before pagination; Collection membership and Saved state remain local and independently editable. | Pass |
| Settings | General, Privacy and Shortcuts are the only primary tabs. Advanced local tools and diagnostics remain available in collapsed secondary sections. | Pass |
| Responsive behavior | At 1280 × 720 all three workspace columns are visible. At 720 × 480 the inspector collapses, search remains usable and `scrollWidth === innerWidth`. | Pass |
| Privacy Cover | When enabled, the protected workspace is not mounted in the DOM; only the cover screen and its disable action remain. Native content protection is applied best-effort. | Pass |
| Accessibility | Search scope is named, active filters are removable, keyboard Action Panel behavior is covered, focus rings remain visible and ordinary metadata is at least 11 px with checked contrast. | Pass |

## Interaction exercise

The final browser pass exercised History search, Saved and Collection navigation, structured type
filtering and chip removal, card selection, inspector Copy, Quick Paste More, the three-item Quick
Paste footer, Settings tab navigation, Privacy Activity empty state and Privacy Cover enable/disable.
The first-value guide was completed through Quick Paste and then disappeared as intended.

The normal and minimum-width captures were reviewed against the selected visual reference after the
final primary-Copy adjustment. No P0, P1 or P2 visual fidelity defect remains in the synthetic
browser fixture. Native Vision, window-protection behavior and Local Link real-device reliability
remain governed by their native/manual evidence gates rather than this browser QA.

final result: passed
