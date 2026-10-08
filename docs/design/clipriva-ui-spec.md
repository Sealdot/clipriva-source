# ClipRiva alpha.2 interface specification

**Status:** prototype direction confirmed by the owner on 2026-09-26; native implementation and verification remain in progress. [Execution status](../planning/clipriva-2.0-execution.md) is the sole task ledger. The [standalone Quick Paste prototype](alpha2-quick-paste-prototype.html) uses synthetic content and has no clipboard or Tauri access. Its [light](../release/evidence/m0-91afcd8/screenshots/prototype-quick-light.png), [dark](../release/evidence/m0-91afcd8/screenshots/prototype-quick-dark.png), and [degraded](../release/evidence/m0-91afcd8/screenshots/prototype-quick-degraded.png) captures are design evidence only.

## Priority when references conflict

The repository's privacy/security boundary, the interaction rules below, and the semantic token table take precedence over illustrative imagery. The supplied planning document's screenshots are proposals, not a record of the current app. Reuse existing code and licensed local assets; do not fetch icons, titles, previews, or fonts over a network.

## Quick Paste

- Target default size: 600×454 logical points, suggested minimum 520×380, with the window constrained to the current screen's usable bounds and verified at 1×/2× scaling. The current 680×440 fixed native window is the before state.
- Show current search scope, one focused search input, one scrollable result list, one primary **复制到剪贴板** action, and one More entry. Default `Enter` and primary action write the selected Item to the system clipboard regardless of Accessibility permission. After successful ordinary Copy, return to the previous app for a manual paste.
- `⌘K` opens the shared More menu after checking existing shortcut conflicts. Secondary actions may include Copy as Plain Text, explicit Direct Paste, Save, add to temporary Stack, and temporary Preview only when valid for that Item and current gates. `⌘I` opens a temporary Preview. Space stays with text input. `Escape` closes an open menu or Preview first, then the window. A new invocation starts with an empty query and all History; returning from Preview within that invocation preserves query and selection.
- A row click selects the Item; it does not write the clipboard. Hover cannot silently move the active selection. Arrow keys move selection without jumping the list page. Background capture does not steal selection. Copy of highlighted text inside the search field or a selectable Preview retains the platform's native `⌘C` meaning.
- Show no permanent sidebar, large brand title, always-open inspector, or Labs/device navigation. Only show the Stack helper when it has Items; label it as temporary and at most 20 Items.

## Recovery feedback contract

| Native fact | User feedback | Selection and retry |
| --- | --- | --- |
| Clipboard write completed | 已复制到剪贴板 | Ordinary Copy may close Quick Paste. |
| Paste command dispatched to the rechecked previous app | 已发送粘贴命令 | Do not say the target accepted content; never auto-resend an uncertain dispatch. |
| Clipboard write completed but paste unavailable | 已复制，请回到原应用按 ⌘V | Keep query/selection visible for the explicit Direct Paste path; permission education is secondary. |
| Clipboard write failed or Item invalid/unavailable | 未能复制 / 原内容不可用 | Keep query and selection; offer retry or details; do not advance Stack. |

The typed native result, not an optimistic UI event, determines this text. Recheck Item validity, Accessibility and frontmost target before dispatch. Prevent rapid duplicate activation while allowing a later distinct user action. A self-write marker/change count suppresses only the app's own actual write; a later user copy of identical text must still be captured. Never equate `pasteSent` with successful receipt by another app.

## Search and content presentation

- Empty query orders by recent copy/use. Nonempty query orders by exact, prefix, then contains/relevance, with time and Item ID as stable ties. Show scope and active filters; clearing them does not delete content. Results from an older request cannot overwrite a newer query.
- Check `版本规划`, `规划`, `release`, `Release`, `user_id`, `a/b?x=1`, quotes, mixed Chinese/English, Note and manually extracted OCR. A Note hit is labeled as Note; Copy still uses the original Item. Short Chinese substrings need a stated supported strategy and a visible limit if fallback is bounded.
- Plain text is rendered as text. Rich text Preview does not execute script or fetch remote resources. Image thumbnails and OCR remain local; OCR is manual. A file reference is labeled as a local reference and failure states that the original file is unavailable. Privacy Cover omits protected content, thumbnails, paths, tooltip and accessible text nodes rather than only blurring them.

## Main workspace and optional features

Default main layout is two columns: a roughly 172-point source rail and a result area. Suggested first size is 920×620, while a saved legal user window size is kept. Primary destinations are History, Saved, and manual/Smart Collections. Inspector opens on demand; at width at least 1000 points it may become a third column, while a narrower window replaces the list with a detail view that returns to its prior position. Draft Note edits require explicit save/discard handling on navigation. Multi-select actions appear only during multi-select.

Saved is the existing `is_pinned` retention fact, not a duplicate collection of content. Manual Collection membership is organization, not automatic retention. Smart Collections are live bounded rules, not stored result snapshots. Removing any Collection membership must not delete an Item. Stack is a process-memory queue; failure stays on the current Item, and exiting clears it. Filter processes an explicitly selected text Item in one to eight allow-listed steps. Preview has no clipboard/database write; execution confirms that Item/rule versions still match, then copies the result without rewriting the original. Labs, automation and Local Link Preview stay inside their independent native gates.

## Semantic tokens

| Token | Light | Dark | Role |
| --- | --- | --- | --- |
| background / surface | `#F5F6F7` / `#FFFFFF` | `#15191D` / `#1D2329` | Window and overlays |
| panel / border | `#F0F2F4` / `#DCE1E5` | `#242C33` / `#3A4650` | Secondary surface and dividers |
| text / text-muted | `#18222C` / `#5C6670` | `#F2F5F6` / `#A6B2BC` | Body and metadata |
| accent / selected | `#167D68` / `#E4F3ED` | `#72D6B6` / `#243D35` | Primary action, selection, focus |
| on-accent | `#FFFFFF` | `#102019` | Primary label |
| warning / danger | `#9C5B00` / `#B42318` | `#F0BC6A` / `#FFB4AB` | Warning and destructive state, with icon/text |

Use the system font stack with PingFang SC fallback for Chinese. Body text targets 13 px, secondary 12 px, lowest-priority time 11 px. A workspace title is 20 px, section heading 14–16 px, control label 12–13 px. Spacing steps are 4/8/12/16/24 px; window radius 12 px, row radius 7 px, control radius 6–8 px. Quick Paste rows target 58–60 px and management rows about 72 px. Common hit targets are at least 32×32 px. The focus ring is 2 px and cannot rely only on a border-color change. Respect reduced motion; transitions may be 120–160 ms but never delay use. Normal text must meet 4.5:1 contrast and large text 3:1 in actual hovered/disabled/focused combinations, not merely in this token table.

## First use, privacy, and exceptional states

First use is one loop: explain capture/pause → copy two non-sensitive examples → open Quick Paste → recover the first → explain “copied to clipboard”. Basic Copy must not request Accessibility. Explain optional Direct Paste only after first value. Pause and excluded apps affect future capture but do not erase History. Privacy Cover affects this app's presentation and accessibility content, not disk encryption or a system-wide recording block. Privacy Activity remains at most 20 content-free events for seven days; diagnostics stay local and default off. OCR, automation, and Local Link need separate opt-in gates.

Empty results show the current scope and an explicit clear/expand option; request failure is an error, not an empty state. Permission denial/revocation yields the copied/manual-`⌘V` fallback without repeated prompts. Unknown source is “来源未知”. Missing file references disable invalid recovery. Full disk/database failure stops destructive writes and keeps prior readable content where possible. A stale Filter result requires reconfirmation. Permanent delete must state its limits and require its existing confirmation flow.

## Review checklist

Confirm the prototype's density, visible Copy label, More placement, result metadata, and degraded wording. Then compare its same-scenario captures with a real Tauri window after CR-02/04; browser fixture captures remain only UI evidence. Verify keyboard, VoiceOver, minimum size, light/dark/system theme, reduced motion, and Privacy Cover on a native build before accepting the UI work.
