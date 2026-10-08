//! Local, deterministic context enrichment for clipboard text.
//!
//! This module deliberately has no database, network, or model dependency.  The
//! returned [`ContextEnrichment`] is a versioned value that callers can persist
//! alongside a clipboard record and re-create when the pipeline version changes.
//! Redacted derivatives are safe defaults for a future explicit cloud-action
//! preview; the original clipboard content remains owned by the caller.

pub mod semantic;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// Version of the serialized enrichment document.
///
/// Increment this only when the shape or semantics of [`ContextEnrichment`]
/// change in a way that requires persisted enrichments to be reprocessed.
pub const CONTEXT_ENRICHMENT_SCHEMA_VERSION: u32 = 1;

/// Stable identifier for this built-in, offline enrichment implementation.
pub const LOCAL_CONTEXT_ENRICHER_ID: &str = "clipriva.local-context";

/// Version of the deterministic rules implemented in this module.
pub const LOCAL_CONTEXT_ENRICHER_VERSION: &str = "1.0.0";

const SUMMARY_MAX_CHARS: usize = 180;
const MAX_TAGS: usize = 8;

/// Runs ClipRiva's built-in local enrichment rules.
///
/// The type has no state so the same input always produces the same output.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalContextPipeline;

impl LocalContextPipeline {
    pub fn enrich(&self, content: &str) -> ContextEnrichment {
        enrich_text(content)
    }
}

/// Enrich text using only deterministic local rules.
pub fn enrich_text(content: &str) -> ContextEnrichment {
    let redaction = build_redaction_preview(content);
    let classification = classify_content(content);
    let entities = extract_entities(content, &redaction);
    let summary = summarize(&redaction.redacted_content, classification.kind);
    let tags = extract_tags(
        &redaction.redacted_content,
        &classification,
        &entities,
        &redaction,
    );

    ContextEnrichment {
        metadata: EnrichmentMetadata {
            schema_version: CONTEXT_ENRICHMENT_SCHEMA_VERSION,
            enricher_id: LOCAL_CONTEXT_ENRICHER_ID.to_owned(),
            enricher_version: LOCAL_CONTEXT_ENRICHER_VERSION.to_owned(),
        },
        classification,
        summary,
        entities,
        tags,
        redaction,
    }
}

/// A fully serializable derivative of one clipboard text payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextEnrichment {
    pub metadata: EnrichmentMetadata,
    pub classification: ContentClassification,
    pub summary: LocalSummary,
    pub entities: Vec<ContextEntity>,
    pub tags: Vec<ContextTag>,
    pub redaction: RedactionPreview,
}

/// Identifies the enrichment schema and exact local rule set that produced it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentMetadata {
    pub schema_version: u32,
    pub enricher_id: String,
    pub enricher_version: String,
}

/// The primary content category and the rules that selected it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContentClassification {
    pub kind: ContentKind,
    pub matched_rules: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Text,
    Url,
    Code,
    Color,
    Json,
}

/// A short, local-only summary. Its text is always generated from redacted input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalSummary {
    pub text: String,
    pub is_truncated: bool,
}

/// A detected entity. `preview` never contains a value that the redaction preview masks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextEntity {
    pub kind: EntityKind,
    /// Byte offsets into the original UTF-8 source text. They are suitable for
    /// highlighting only after the caller verifies it still has that exact text.
    pub source_range: TextRange,
    pub preview: String,
    pub is_redacted: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Url,
    EmailAddress,
    PhoneNumber,
    PaymentCardNumber,
    ApiKey,
    SecretValue,
    FilePath,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

/// A stable tag suitable for filtering. Topic tags are generated from redacted text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct ContextTag {
    pub id: String,
    pub label: String,
}

/// A preview to show before content crosses a cloud, sync, or agent boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RedactionPreview {
    /// The original content with locally detected sensitive spans replaced by
    /// stable placeholders such as `[REDACTED:email_address]`.
    pub redacted_content: String,
    pub requires_explicit_consent: bool,
    /// Contains type and position only; never stores the sensitive value itself.
    pub matches: Vec<RedactionMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RedactionMatch {
    pub kind: RedactionKind,
    /// Byte offsets into the original UTF-8 source text.
    pub source_range: TextRange,
    pub replacement: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RedactionKind {
    EmailAddress,
    PhoneNumber,
    PaymentCardNumber,
    ApiKey,
    SecretValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SensitiveSpan {
    kind: RedactionKind,
    start: usize,
    end: usize,
}

impl SensitiveSpan {
    fn replacement(self) -> String {
        format!("[REDACTED:{}]", redaction_kind_id(self.kind))
    }
}

fn classify_content(content: &str) -> ContentClassification {
    let value = content.trim();

    if is_hex_color(value) {
        return ContentClassification {
            kind: ContentKind::Color,
            matched_rules: vec!["content.color.hex".to_owned()],
        };
    }

    if is_single_http_url(content) {
        return ContentClassification {
            kind: ContentKind::Url,
            matched_rules: vec!["content.url.http".to_owned()],
        };
    }

    if looks_like_code(value) {
        return ContentClassification {
            kind: ContentKind::Code,
            matched_rules: vec!["content.code.markers".to_owned()],
        };
    }

    if looks_like_json(value) {
        return ContentClassification {
            kind: ContentKind::Json,
            matched_rules: vec!["content.json.delimiters".to_owned()],
        };
    }

    ContentClassification {
        kind: ContentKind::Text,
        matched_rules: vec!["content.text.default".to_owned()],
    }
}

fn is_hex_color(value: &str) -> bool {
    matches!(value.len(), 4 | 7 | 9)
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn is_single_http_url(content: &str) -> bool {
    let trimmed_start = content.len() - content.trim_start().len();
    let trimmed = content.trim();
    let urls = find_http_urls(content);
    let Some(url) = urls.first() else {
        return false;
    };

    url.0 == trimmed_start && url.1 == trimmed_start + trimmed.len()
}

fn looks_like_code(value: &str) -> bool {
    const CODE_MARKERS: [&str; 14] = [
        "const ",
        "let ",
        "fn ",
        "def ",
        "class ",
        "import ",
        "export ",
        "public ",
        "SELECT ",
        "#include",
        "function ",
        "interface ",
        "impl ",
        "use ",
    ];

    CODE_MARKERS.iter().any(|marker| value.contains(marker))
        || (value.contains('\n') && (value.contains('{') || value.contains("=>")))
}

fn looks_like_json(value: &str) -> bool {
    (value.starts_with('{') && value.ends_with('}'))
        || (value.starts_with('[') && value.ends_with(']'))
}

fn build_redaction_preview(content: &str) -> RedactionPreview {
    let spans = sensitive_spans(content);
    let matches = spans
        .iter()
        .map(|span| RedactionMatch {
            kind: span.kind,
            source_range: TextRange {
                start: span.start,
                end: span.end,
            },
            replacement: span.replacement(),
        })
        .collect::<Vec<_>>();

    RedactionPreview {
        redacted_content: redact_range(content, 0, content.len(), &spans),
        requires_explicit_consent: !matches.is_empty(),
        matches,
    }
}

fn sensitive_spans(content: &str) -> Vec<SensitiveSpan> {
    let mut candidates = Vec::new();
    candidates.extend(find_email_spans(content));
    candidates.extend(find_payment_card_spans(content));
    candidates.extend(find_api_key_spans(content));
    candidates.extend(find_secret_assignment_spans(content));
    candidates.extend(find_phone_spans(content));

    // Resolve overlaps by location, then by the more specific/redaction-safe
    // category. This produces a stable non-overlapping replacement plan.
    candidates.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| redaction_priority(right.kind).cmp(&redaction_priority(left.kind)))
            .then_with(|| (right.end - right.start).cmp(&(left.end - left.start)))
            .then_with(|| left.end.cmp(&right.end))
    });

    let mut accepted = Vec::new();
    for candidate in candidates {
        if accepted
            .iter()
            .all(|existing: &SensitiveSpan| !ranges_overlap(existing, &candidate))
        {
            accepted.push(candidate);
        }
    }

    accepted.sort_by_key(|span| span.start);
    accepted
}

fn redaction_priority(kind: RedactionKind) -> u8 {
    match kind {
        RedactionKind::ApiKey => 5,
        RedactionKind::SecretValue => 4,
        RedactionKind::PaymentCardNumber => 3,
        RedactionKind::EmailAddress => 2,
        RedactionKind::PhoneNumber => 1,
    }
}

fn ranges_overlap(left: &SensitiveSpan, right: &SensitiveSpan) -> bool {
    left.start < right.end && right.start < left.end
}

fn find_email_spans(content: &str) -> Vec<SensitiveSpan> {
    let bytes = content.as_bytes();
    let mut spans = Vec::new();

    for (at, byte) in bytes.iter().enumerate() {
        if *byte != b'@' || at == 0 || at + 1 >= bytes.len() {
            continue;
        }

        let mut start = at;
        while start > 0 && is_email_local_byte(bytes[start - 1]) {
            start -= 1;
        }

        let mut end = at + 1;
        while end < bytes.len() && is_email_domain_byte(bytes[end]) {
            end += 1;
        }
        while end > at + 1 && bytes[end - 1] == b'.' {
            end -= 1;
        }

        let domain = &content[at + 1..end];
        if start < at && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
        {
            spans.push(SensitiveSpan {
                kind: RedactionKind::EmailAddress,
                start,
                end,
            });
        }
    }

    spans
}

fn is_email_local_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-')
}

fn is_email_domain_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')
}

fn find_payment_card_spans(content: &str) -> Vec<SensitiveSpan> {
    find_digit_sequences(content)
        .into_iter()
        .filter_map(|(start, end, digits)| {
            ((13..=19).contains(&digits.len()) && passes_luhn(&digits)).then_some(SensitiveSpan {
                kind: RedactionKind::PaymentCardNumber,
                start,
                end,
            })
        })
        .collect()
}

fn find_phone_spans(content: &str) -> Vec<SensitiveSpan> {
    find_digit_sequences(content)
        .into_iter()
        .filter_map(|(start, end, digits)| {
            ((10..=15).contains(&digits.len()) && !passes_luhn(&digits)).then_some(SensitiveSpan {
                kind: RedactionKind::PhoneNumber,
                start,
                end,
            })
        })
        .collect()
}

/// Finds digit runs with common phone/card separators. The result preserves the
/// source span and supplies normalized digits only for local validation.
fn find_digit_sequences(content: &str) -> Vec<(usize, usize, String)> {
    let bytes = content.as_bytes();
    let mut values = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let can_start = bytes[cursor].is_ascii_digit()
            || (bytes[cursor] == b'+'
                && cursor + 1 < bytes.len()
                && bytes[cursor + 1].is_ascii_digit());
        let preceded_by_word = cursor > 0 && is_word_byte(bytes[cursor - 1]);
        if !can_start || preceded_by_word {
            cursor += 1;
            continue;
        }

        let start = cursor;
        let mut end = cursor;
        let mut digits = String::new();
        while end < bytes.len() && is_digit_separator_byte(bytes[end]) {
            if bytes[end].is_ascii_digit() {
                digits.push(bytes[end] as char);
            }
            end += 1;
        }

        // A separator cannot be the final character in an entity.
        while end > start && !bytes[end - 1].is_ascii_digit() {
            end -= 1;
        }

        let followed_by_word = end < bytes.len() && is_word_byte(bytes[end]);
        if !followed_by_word && !digits.is_empty() {
            values.push((start, end, digits));
        }
        cursor = end.max(cursor + 1);
    }

    values
}

fn is_digit_separator_byte(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b' ' | b'(' | b')' | b'.')
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn passes_luhn(digits: &str) -> bool {
    let mut sum = 0_u32;
    for (index, digit) in digits.bytes().rev().enumerate() {
        let mut value = u32::from(digit - b'0');
        if index % 2 == 1 {
            value *= 2;
            if value > 9 {
                value -= 9;
            }
        }
        sum += value;
    }
    sum % 10 == 0
}

fn find_api_key_spans(content: &str) -> Vec<SensitiveSpan> {
    const PREFIXES: [(&str, usize); 3] = [("sk-", 20), ("ghp_", 20), ("github_pat_", 20)];
    let lower = content.to_ascii_lowercase();
    let bytes = content.as_bytes();
    let mut spans = Vec::new();

    for (prefix, minimum_length) in PREFIXES {
        for (start, _) in lower.match_indices(prefix) {
            if start > 0 && is_word_byte(bytes[start - 1]) {
                continue;
            }

            let end = consume_secret_token(bytes, start);
            if end - start >= minimum_length {
                spans.push(SensitiveSpan {
                    kind: RedactionKind::ApiKey,
                    start,
                    end,
                });
            }
        }
    }

    // AWS access-key IDs are exactly 20 upper-case alpha-numeric characters.
    for (start, _) in content.match_indices("AKIA") {
        let end = start + 20;
        if end <= bytes.len()
            && (start == 0 || !is_word_byte(bytes[start - 1]))
            && (end == bytes.len() || !is_word_byte(bytes[end]))
            && bytes[start..end]
                .iter()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            spans.push(SensitiveSpan {
                kind: RedactionKind::ApiKey,
                start,
                end,
            });
        }
    }

    spans
}

fn find_secret_assignment_spans(content: &str) -> Vec<SensitiveSpan> {
    const SECRET_KEYS: [&str; 8] = [
        "access_token",
        "api_key",
        "apikey",
        "authorization",
        "password",
        "secret",
        "token",
        "passwd",
    ];

    let lower = content.to_ascii_lowercase();
    let bytes = content.as_bytes();
    let mut spans = Vec::new();

    for key in SECRET_KEYS {
        for (key_start, _) in lower.match_indices(key) {
            let key_end = key_start + key.len();
            if (key_start > 0 && is_word_byte(bytes[key_start - 1]))
                || (key_end < bytes.len() && is_word_byte(bytes[key_end]))
            {
                continue;
            }

            let mut value_start = key_end;
            while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
                value_start += 1;
            }
            if value_start >= bytes.len() || !matches!(bytes[value_start], b':' | b'=') {
                continue;
            }
            value_start += 1;
            while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
                value_start += 1;
            }

            if lower[value_start..].starts_with("bearer ") {
                value_start += "bearer ".len();
            }
            let quote = bytes
                .get(value_start)
                .copied()
                .filter(|byte| matches!(byte, b'\'' | b'\"'));
            if quote.is_some() {
                value_start += 1;
            }
            let mut value_end = value_start;
            while value_end < bytes.len()
                && is_secret_value_byte(bytes[value_end])
                && quote.is_none_or(|delimiter| bytes[value_end] != delimiter)
            {
                value_end += 1;
            }

            if value_end.saturating_sub(value_start) >= 6 {
                spans.push(SensitiveSpan {
                    kind: if key == "authorization" {
                        RedactionKind::ApiKey
                    } else {
                        RedactionKind::SecretValue
                    },
                    start: value_start,
                    end: value_end,
                });
            }
        }
    }

    spans
}

fn is_secret_value_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/' | b'+' | b'=')
}

fn consume_secret_token(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < bytes.len() && is_secret_value_byte(bytes[end]) {
        end += 1;
    }
    end
}

fn extract_entities(content: &str, redaction: &RedactionPreview) -> Vec<ContextEntity> {
    let spans = redaction_spans(redaction);
    let mut candidates = BTreeSet::new();

    for (start, end) in find_http_urls(content) {
        candidates.insert((start, end, EntityKind::Url));
    }
    for SensitiveSpan { kind, start, end } in &spans {
        candidates.insert((*start, *end, entity_kind_for_redaction(*kind)));
    }
    for (start, end) in find_file_paths(content) {
        candidates.insert((start, end, EntityKind::FilePath));
    }

    candidates
        .into_iter()
        .map(|(start, end, kind)| {
            let is_redacted = spans
                .iter()
                .any(|span| span.start < end && start < span.end);
            ContextEntity {
                kind,
                source_range: TextRange { start, end },
                preview: redact_range(content, start, end, &spans),
                is_redacted,
            }
        })
        .collect()
}

fn redaction_spans(redaction: &RedactionPreview) -> Vec<SensitiveSpan> {
    redaction
        .matches
        .iter()
        .map(|item| SensitiveSpan {
            kind: item.kind,
            start: item.source_range.start,
            end: item.source_range.end,
        })
        .collect()
}

fn entity_kind_for_redaction(kind: RedactionKind) -> EntityKind {
    match kind {
        RedactionKind::EmailAddress => EntityKind::EmailAddress,
        RedactionKind::PhoneNumber => EntityKind::PhoneNumber,
        RedactionKind::PaymentCardNumber => EntityKind::PaymentCardNumber,
        RedactionKind::ApiKey => EntityKind::ApiKey,
        RedactionKind::SecretValue => EntityKind::SecretValue,
    }
}

fn find_http_urls(content: &str) -> Vec<(usize, usize)> {
    let lower = content.to_ascii_lowercase();
    let bytes = content.as_bytes();
    let mut values = Vec::new();

    for scheme in ["https://", "http://"] {
        for (start, _) in lower.match_indices(scheme) {
            if start > 0 && is_word_byte(bytes[start - 1]) {
                continue;
            }

            let mut end = start + scheme.len();
            while end < bytes.len() && !is_url_terminator(bytes[end]) {
                end += 1;
            }
            while end > start + scheme.len() && is_trailing_url_punctuation(bytes[end - 1]) {
                end -= 1;
            }
            if end > start + scheme.len() {
                values.push((start, end));
            }
        }
    }

    values.sort_unstable();
    values.dedup();
    values
}

fn is_url_terminator(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'<' | b'>' | b'\"' | b'\'')
}

fn is_trailing_url_punctuation(byte: u8) -> bool {
    matches!(
        byte,
        b'.' | b',' | b';' | b':' | b'!' | b'?' | b')' | b']' | b'}'
    )
}

fn find_file_paths(content: &str) -> Vec<(usize, usize)> {
    let bytes = content.as_bytes();
    let mut values = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let starts_absolute = bytes[cursor] == b'/'
            && (cursor == 0 || (!is_word_byte(bytes[cursor - 1]) && bytes[cursor - 1] != b'/'))
            && cursor + 1 < bytes.len()
            && !matches!(bytes[cursor + 1], b'/' | b' ');
        let starts_home = bytes[cursor] == b'~'
            && cursor + 2 < bytes.len()
            && bytes[cursor + 1] == b'/'
            && (cursor == 0 || !is_word_byte(bytes[cursor - 1]));
        if !starts_absolute && !starts_home {
            cursor += 1;
            continue;
        }

        let start = cursor;
        let mut end = cursor;
        while end < bytes.len() && is_path_byte(bytes[end]) {
            end += 1;
        }
        while end > start && is_trailing_url_punctuation(bytes[end - 1]) {
            end -= 1;
        }
        if end > start + 1 {
            values.push((start, end));
        }
        cursor = end.max(cursor + 1);
    }

    values
}

fn is_path_byte(byte: u8) -> bool {
    !byte.is_ascii_whitespace() && !matches!(byte, b'<' | b'>' | b'\"' | b'\'' | b'`')
}

fn redact_range(content: &str, start: usize, end: usize, spans: &[SensitiveSpan]) -> String {
    let mut output = String::new();
    let mut cursor = start;

    for span in spans {
        if span.end <= start || span.start >= end {
            continue;
        }
        // All emitted spans originate from ASCII recognizers and are fully
        // contained in entity ranges when relevant.
        let replacement_start = span.start.max(start);
        let replacement_end = span.end.min(end);
        output.push_str(&content[cursor..replacement_start]);
        output.push_str(&span.replacement());
        cursor = replacement_end;
    }
    output.push_str(&content[cursor..end]);
    output
}

fn summarize(redacted_content: &str, kind: ContentKind) -> LocalSummary {
    let compact = redacted_content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if compact.is_empty() {
        return LocalSummary {
            text: "Empty clipboard text".to_owned(),
            is_truncated: false,
        };
    }

    let (prefix, candidate) = match kind {
        ContentKind::Url => ("Link: ", compact.as_str()),
        ContentKind::Color => ("Color: ", compact.as_str()),
        ContentKind::Code => {
            let first_line = redacted_content
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or(compact.as_str());
            ("Code: ", first_line)
        }
        ContentKind::Json => ("Structured data: ", compact.as_str()),
        ContentKind::Text => ("", first_sentence(&compact)),
    };

    let available_chars = SUMMARY_MAX_CHARS.saturating_sub(prefix.chars().count());
    let (body, shortened) = truncate_chars(candidate, available_chars);
    let omitted_after_sentence = kind == ContentKind::Text && candidate.len() < compact.len();
    LocalSummary {
        text: format!("{prefix}{body}"),
        is_truncated: shortened || omitted_after_sentence,
    }
}

fn first_sentence(content: &str) -> &str {
    for (index, character) in content.char_indices() {
        if matches!(character, '.' | '!' | '?') {
            return &content[..index + character.len_utf8()];
        }
    }
    content
}

fn truncate_chars(value: &str, maximum: usize) -> (String, bool) {
    let count = value.chars().count();
    if count <= maximum {
        return (value.to_owned(), false);
    }

    let mut truncated = value
        .chars()
        .take(maximum.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    (truncated, true)
}

fn extract_tags(
    redacted_content: &str,
    classification: &ContentClassification,
    entities: &[ContextEntity],
    redaction: &RedactionPreview,
) -> Vec<ContextTag> {
    let mut tag_ids = BTreeSet::new();
    tag_ids.insert(format!("content:{}", content_kind_id(classification.kind)));

    for entity in entities {
        tag_ids.insert(format!("entity:{}", entity_kind_id(entity.kind)));
    }
    if redaction.requires_explicit_consent {
        tag_ids.insert("safety:redacted".to_owned());
    }
    if classification.kind == ContentKind::Code {
        if let Some(language) = detect_code_language(redacted_content) {
            tag_ids.insert(format!("language:{language}"));
        }
    }

    let occupied = tag_ids.len();
    for keyword in keyword_tags(redacted_content, MAX_TAGS.saturating_sub(occupied)) {
        tag_ids.insert(format!("topic:{keyword}"));
    }

    tag_ids
        .into_iter()
        .take(MAX_TAGS)
        .map(|id| ContextTag {
            label: tag_label(&id),
            id,
        })
        .collect()
}

fn content_kind_id(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::Text => "text",
        ContentKind::Url => "url",
        ContentKind::Code => "code",
        ContentKind::Color => "color",
        ContentKind::Json => "json",
    }
}

fn entity_kind_id(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Url => "url",
        EntityKind::EmailAddress => "email_address",
        EntityKind::PhoneNumber => "phone_number",
        EntityKind::PaymentCardNumber => "payment_card_number",
        EntityKind::ApiKey => "api_key",
        EntityKind::SecretValue => "secret_value",
        EntityKind::FilePath => "file_path",
    }
}

fn redaction_kind_id(kind: RedactionKind) -> &'static str {
    match kind {
        RedactionKind::EmailAddress => "email_address",
        RedactionKind::PhoneNumber => "phone_number",
        RedactionKind::PaymentCardNumber => "payment_card_number",
        RedactionKind::ApiKey => "api_key",
        RedactionKind::SecretValue => "secret_value",
    }
}

fn detect_code_language(content: &str) -> Option<&'static str> {
    let trimmed = content.trim();
    if trimmed.contains("fn ") || trimmed.contains("impl ") || trimmed.contains("use ") {
        Some("rust")
    } else if trimmed.contains("const ")
        || trimmed.contains("let ")
        || trimmed.contains("=>")
        || trimmed.contains("export ")
    {
        Some("javascript")
    } else if trimmed.contains("def ") || trimmed.contains("import ") {
        Some("python")
    } else if trimmed.contains("SELECT ") || trimmed.contains("FROM ") {
        Some("sql")
    } else {
        None
    }
}

fn keyword_tags(content: &str, maximum: usize) -> Vec<String> {
    const STOP_WORDS: [&str; 38] = [
        "about", "address", "after", "again", "also", "and", "api", "are", "been", "before", "but",
        "com", "email", "for", "from", "have", "https", "http", "into", "its", "key", "not", "org",
        "redacted", "that", "the", "their", "then", "this", "to", "with", "www", "you", "your",
        "value", "secret", "token", "password",
    ];
    let stop_words = STOP_WORDS.into_iter().collect::<BTreeSet<_>>();
    let mut counts = BTreeMap::<String, usize>::new();

    for word in content.split(|character: char| !character.is_alphanumeric()) {
        let normalized = word.to_ascii_lowercase();
        let length = normalized.chars().count();
        if !(4..=32).contains(&length) || stop_words.contains(normalized.as_str()) {
            continue;
        }
        *counts.entry(normalized).or_default() += 1;
    }

    let mut values = counts.into_iter().collect::<Vec<_>>();
    values.sort_by(|(left_word, left_count), (right_word, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_word.cmp(right_word))
    });
    values
        .into_iter()
        .take(maximum)
        .map(|(word, _)| word)
        .collect()
}

fn tag_label(id: &str) -> String {
    match id {
        "content:text" => "Text".to_owned(),
        "content:url" => "URL".to_owned(),
        "content:code" => "Code".to_owned(),
        "content:color" => "Color".to_owned(),
        "content:json" => "JSON".to_owned(),
        "entity:url" => "Contains URL".to_owned(),
        "entity:email_address" => "Contains email".to_owned(),
        "entity:phone_number" => "Contains phone number".to_owned(),
        "entity:payment_card_number" => "Contains payment card".to_owned(),
        "entity:api_key" => "Contains API key".to_owned(),
        "entity:secret_value" => "Contains secret".to_owned(),
        "entity:file_path" => "Contains file path".to_owned(),
        "safety:redacted" => "Sensitive data redacted".to_owned(),
        _ if id.starts_with("language:") => {
            format!("{} code", title_case(&id["language:".len()..]))
        }
        _ if id.starts_with("topic:") => title_case(&id["topic:".len()..]),
        _ => id.to_owned(),
    }
}

fn title_case(value: &str) -> String {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    first.to_uppercase().chain(characters).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        enrich_text, ContentKind, EntityKind, RedactionKind, CONTEXT_ENRICHMENT_SCHEMA_VERSION,
        LOCAL_CONTEXT_ENRICHER_ID, LOCAL_CONTEXT_ENRICHER_VERSION,
    };

    #[test]
    fn produces_stable_versioned_output_for_the_same_text() {
        let content = "Read the ClipRiva release plan at https://clipriva.example/docs.";
        let first = enrich_text(content);
        let second = enrich_text(content);

        assert_eq!(first, second);
        assert_eq!(
            first.metadata.schema_version,
            CONTEXT_ENRICHMENT_SCHEMA_VERSION
        );
        assert_eq!(first.metadata.enricher_id, LOCAL_CONTEXT_ENRICHER_ID);
        assert_eq!(
            first.metadata.enricher_version,
            LOCAL_CONTEXT_ENRICHER_VERSION
        );
        assert_eq!(first.classification.kind, ContentKind::Text);
        assert!(first
            .entities
            .iter()
            .any(|entity| entity.kind == EntityKind::Url));
        assert_eq!(
            serde_json::to_string(&first).expect("enrichment serializes"),
            serde_json::to_string(&second).expect("enrichment serializes")
        );
    }

    #[test]
    fn classifies_color_url_code_and_json_with_traceable_rules() {
        let color = enrich_text("#6EE7B7");
        let url = enrich_text("https://clipriva.example/quick-paste");
        let code = enrich_text("fn main() { println!(\"ClipRiva\"); }");
        let json = enrich_text("{\"mode\":\"local\"}");

        assert_eq!(color.classification.kind, ContentKind::Color);
        assert_eq!(url.classification.kind, ContentKind::Url);
        assert_eq!(code.classification.kind, ContentKind::Code);
        assert_eq!(json.classification.kind, ContentKind::Json);
        assert_eq!(code.classification.matched_rules, ["content.code.markers"]);
        assert!(code.tags.iter().any(|tag| tag.id == "language:rust"));
    }

    #[test]
    fn redacts_sensitive_values_from_every_derived_text_field() {
        let email = "alice@example.com";
        let card = "4242 4242 4242 4242";
        let key = "sk-1234567890abcdefghijklmnop";
        let source = format!("Contact {email}; card {card}; api_key={key}");
        let enrichment = enrich_text(&source);
        let serialized = serde_json::to_string(&enrichment).expect("enrichment serializes");

        assert!(enrichment.redaction.requires_explicit_consent);
        assert!(!enrichment.redaction.redacted_content.contains(email));
        assert!(!enrichment.redaction.redacted_content.contains(card));
        assert!(!enrichment.redaction.redacted_content.contains(key));
        assert!(!enrichment.summary.text.contains(email));
        assert!(!serialized.contains(email));
        assert!(!serialized.contains(card));
        assert!(!serialized.contains(key));
        assert!(enrichment
            .redaction
            .matches
            .iter()
            .any(|item| item.kind == RedactionKind::EmailAddress));
        assert!(enrichment
            .redaction
            .matches
            .iter()
            .any(|item| item.kind == RedactionKind::PaymentCardNumber));
        assert!(enrichment
            .redaction
            .matches
            .iter()
            .any(|item| item.kind == RedactionKind::ApiKey));
        assert!(enrichment.entities.iter().any(|entity| {
            entity.kind == EntityKind::EmailAddress
                && entity.is_redacted
                && entity.preview == "[REDACTED:email_address]"
        }));
    }

    #[test]
    fn emits_stable_topic_tags_from_redacted_text_only() {
        let enrichment = enrich_text(
            "ClipRiva retrieval makes clipboard retrieval faster. token=secret-value-12345",
        );
        let tag_ids = enrichment
            .tags
            .iter()
            .map(|tag| tag.id.as_str())
            .collect::<Vec<_>>();

        assert!(tag_ids.contains(&"topic:clipriva"));
        assert!(tag_ids.contains(&"topic:retrieval"));
        assert!(tag_ids.contains(&"safety:redacted"));
        assert!(!tag_ids.iter().any(|id| id.contains("secret-value")));
        assert!(tag_ids.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn summarizes_unicode_text_on_character_boundaries() {
        let enrichment = enrich_text("Image · 1 × 1. 下一条说明仍在本地。");

        assert_eq!(enrichment.summary.text, "Image · 1 × 1.");
    }
}
