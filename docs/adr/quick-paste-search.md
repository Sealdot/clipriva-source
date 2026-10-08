# Quick Paste local search decision for alpha.2

**Status:** implemented for CR-03, performance acceptance pending. **Decision date:** 2026-09-26.

## Context

The prior Quick Paste path fetched only the newest 10,000 rows by creation time and ranked them in Rust. A retained older Item could never be returned. The main History FTS index uses SQLite `unicode61`; Quick Paste must also handle continuous Chinese substrings, short punctuation, URLs, Notes and manually extracted image text. Query text and results remain on device.

## Options considered

| Option | Recall and behavior | Cost and risk |
| --- | --- | --- |
| Keep newest-10k Rust scan | Existing exact/prefix/contains ranking stays simple. | Misses older retained Items, so fails 50k recall. |
| Route Quick Paste only through existing FTS | Uses existing local index for many words. | Token matching does not cover the required short and punctuation substrings; would change existing ranking. |
| Add a new n-gram or trigram index | Could make broad substring search faster after a successful migration. | Adds index bytes, write cost and migration/rollback work to every Item lifecycle. Needs measured design and CR-08 migration evidence first. |
| Parameterized SQLite anchor prefilter, then Rust rank | Searches the full retained set for a required literal term, retains exact/prefix/contains semantics and stable tie breaks, uses no new stored field. | Broad terms still hydrate/rank many Items; SQLite `lower()` does not provide complete Unicode case folding. |

## Decision and boundary

Use the last option for this reversible alpha.2 increment. A nonempty query is whitespace-normalized and its longest term becomes a bound SQLite `instr(lower(...), ?)` parameter. The prefilter includes content, file/search text, current and historical source app, tags, Notes and manual image text. Rust checks the complete normalized query and chooses the best field and tier. Only the visible result page receives complete representation data. Zero-input suggestions remain bounded and now consider recent use/update before original creation time.

No automatic OCR, remote provider, telemetry, cloud index, new permission, schema migration or new stored data type is introduced. OCR text is not serialized in Quick Paste results; only `imageText` match provenance crosses the IPC boundary. Notes retain the same content-free provenance rule.

## Measured evidence and remaining decision

A 50,001-Item synthetic in-memory fixture with a 100,000-Item retention setting found the older unique match. One macOS 14 Apple M1 debug-build sample after the retention fast-path optimization took approximately 62 ms for `unique sample` and 557 ms for broad `filler`. These are single observations, not release P95 or native UI latency. The broad query exceeds the planned 50k search P95 ≤350 ms target in that debug sample. The existing release-mode 10k benchmark remains ignored by ordinary `cargo test`. Before CR-03 acceptance, run at least 100 hot native/release samples for both rare and broad queries with raw measurements, index size and write-cost comparison. If the broad path misses target, optimize candidate hydration or propose a measured additive index through CR-08 review; do not change schema merely to hide the current risk.

The current anchor prefilter is complete for the required Chinese, ASCII case and literal-symbol fixtures. SQLite's built-in lowercase operation may not match Rust's full Unicode lowercase behavior for some non-ASCII cased scripts; this is a documented residual recall risk pending a focused fixture and index decision. React Query uses the full query in its cache key and does not retain a previous query's data as placeholder; an out-of-order native request test remains open.
