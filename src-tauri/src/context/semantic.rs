//! Replaceable semantic-search contract and a deterministic local fallback.
//!
//! The fallback intentionally performs lexical ranking rather than pretending to
//! provide embeddings. It gives the application a useful local implementation
//! now, while preserving the same boundary that an embedding-backed adapter will
//! implement later.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::TextRange;

/// Version of the serialized semantic-search request and result shapes.
pub const SEMANTIC_SEARCH_SCHEMA_VERSION: u32 = 1;

/// Stable identifier for the built-in local fallback adapter.
pub const LOCAL_LEXICAL_FALLBACK_ADAPTER_ID: &str = "clipriva.local-lexical";

/// Version of the local fallback ranking rules.
pub const LOCAL_LEXICAL_FALLBACK_ADAPTER_VERSION: &str = "1.0.0";

/// A replaceable interface for local or opt-in external semantic retrieval.
///
/// Adapters receive caller-owned documents. They do not read application data or
/// make privacy decisions themselves; callers choose whether supplied text is
/// original or redacted before invoking an adapter.
pub trait SemanticSearchAdapter {
    fn descriptor(&self) -> SemanticAdapterDescriptor;

    /// Creates vectors for stable document IDs. Adapters that do not expose an
    /// embedding capability should return [`SemanticSearchError::EmbeddingsUnsupported`].
    fn embed(&self, inputs: &[SemanticEmbeddingInput]) -> SemanticResult<Vec<SemanticVector>>;

    /// Ranks caller-provided documents for one query.
    fn search(
        &self,
        query: &SemanticSearchQuery,
        documents: &[SemanticSearchDocument],
    ) -> SemanticResult<SemanticSearchResponse>;
}

pub type SemanticResult<T> = Result<T, SemanticSearchError>;

/// Metadata that lets persisted results identify the adapter that produced them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticAdapterDescriptor {
    pub schema_version: u32,
    pub adapter_id: String,
    pub adapter_version: String,
    pub supports_embeddings: bool,
}

/// Text and stable ID to supply to an adapter when creating an embedding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticEmbeddingInput {
    pub document_id: String,
    pub text: String,
}

/// A versioned vector belonging to one document.
///
/// The vector is deliberately self-describing so a future model/provider change
/// cannot silently mix incompatible dimensions in a persisted index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticVector {
    pub schema_version: u32,
    pub document_id: String,
    pub adapter: SemanticAdapterDescriptor,
    pub values: Vec<f32>,
}

/// A document to rank. `vector` is optional because a fallback may operate on
/// text, and an embedding adapter may choose to generate a missing vector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchDocument {
    pub document_id: String,
    pub text: String,
    pub vector: Option<SemanticVector>,
}

/// Search parameters shared by every adapter implementation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchQuery {
    pub schema_version: u32,
    pub text: String,
    pub limit: usize,
    /// Inclusive normalized relevance threshold between 0.0 and 1.0.
    pub minimum_score: Option<f32>,
}

/// One ranked item. All `highlight_ranges` are byte offsets into that item's
/// original UTF-8 `SemanticSearchDocument::text` value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchResult {
    pub document_id: String,
    pub score: f32,
    pub matched_terms: Vec<String>,
    pub highlight_ranges: Vec<TextRange>,
}

/// A complete, serializable response from a semantic-search adapter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchResponse {
    pub schema_version: u32,
    pub adapter: SemanticAdapterDescriptor,
    pub mode: SemanticSearchMode,
    pub results: Vec<SemanticSearchResult>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticSearchMode {
    Vector,
    Hybrid,
    LexicalFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticSearchError {
    EmptyQuery,
    InvalidMinimumScore,
    EmbeddingsUnsupported,
}

impl fmt::Display for SemanticSearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyQuery => formatter.write_str("semantic search query cannot be empty"),
            Self::InvalidMinimumScore => {
                formatter.write_str("semantic search minimum score must be between 0.0 and 1.0")
            }
            Self::EmbeddingsUnsupported => formatter
                .write_str("the selected semantic-search adapter does not create embeddings"),
        }
    }
}

impl Error for SemanticSearchError {}

/// An offline, deterministic fallback that ranks documents by token-set overlap.
///
/// It does not create vectors and never sends text anywhere. Scores are the
/// Jaccard similarity of normalized query and document token sets, in `[0.0, 1.0]`.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalLexicalFallbackAdapter;

impl SemanticSearchAdapter for LocalLexicalFallbackAdapter {
    fn descriptor(&self) -> SemanticAdapterDescriptor {
        SemanticAdapterDescriptor {
            schema_version: SEMANTIC_SEARCH_SCHEMA_VERSION,
            adapter_id: LOCAL_LEXICAL_FALLBACK_ADAPTER_ID.to_owned(),
            adapter_version: LOCAL_LEXICAL_FALLBACK_ADAPTER_VERSION.to_owned(),
            supports_embeddings: false,
        }
    }

    fn embed(&self, _inputs: &[SemanticEmbeddingInput]) -> SemanticResult<Vec<SemanticVector>> {
        Err(SemanticSearchError::EmbeddingsUnsupported)
    }

    fn search(
        &self,
        query: &SemanticSearchQuery,
        documents: &[SemanticSearchDocument],
    ) -> SemanticResult<SemanticSearchResponse> {
        let query_tokens = tokenize(&query.text);
        if query_tokens.is_empty() {
            return Err(SemanticSearchError::EmptyQuery);
        }
        validate_minimum_score(query.minimum_score)?;

        let query_terms = query_tokens
            .iter()
            .map(|token| token.normalized.as_str())
            .collect::<BTreeSet<_>>();
        let minimum_score = query.minimum_score.unwrap_or(0.0);
        let mut results = documents
            .iter()
            .filter_map(|document| lexical_result(document, &query_terms, minimum_score))
            .collect::<Vec<_>>();

        results.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.document_id.cmp(&right.document_id))
        });
        results.truncate(query.limit);

        Ok(SemanticSearchResponse {
            schema_version: SEMANTIC_SEARCH_SCHEMA_VERSION,
            adapter: self.descriptor(),
            mode: SemanticSearchMode::LexicalFallback,
            results,
        })
    }
}

#[derive(Debug)]
struct Token {
    normalized: String,
    source_range: TextRange,
}

fn lexical_result(
    document: &SemanticSearchDocument,
    query_terms: &BTreeSet<&str>,
    minimum_score: f32,
) -> Option<SemanticSearchResult> {
    let document_tokens = tokenize(&document.text);
    let document_terms = document_tokens
        .iter()
        .map(|token| token.normalized.as_str())
        .collect::<BTreeSet<_>>();
    let matched_terms = query_terms
        .intersection(&document_terms)
        .map(|term| (*term).to_owned())
        .collect::<Vec<_>>();
    if matched_terms.is_empty() {
        return None;
    }

    let intersection_size = matched_terms.len();
    let union_size = query_terms.len() + document_terms.len() - intersection_size;
    let score = intersection_size as f32 / union_size as f32;
    if score < minimum_score {
        return None;
    }

    let matched_term_set = matched_terms
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let highlight_ranges = document_tokens
        .iter()
        .filter(|token| matched_term_set.contains(token.normalized.as_str()))
        .map(|token| token.source_range.clone())
        .collect();

    Some(SemanticSearchResult {
        document_id: document.document_id.clone(),
        score,
        matched_terms,
        highlight_ranges,
    })
}

fn validate_minimum_score(minimum_score: Option<f32>) -> SemanticResult<()> {
    if minimum_score.is_some_and(|score| !(0.0..=1.0).contains(&score)) {
        return Err(SemanticSearchError::InvalidMinimumScore);
    }
    Ok(())
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut token_start = None;

    for (index, character) in text.char_indices() {
        if character.is_alphanumeric() {
            token_start.get_or_insert(index);
            continue;
        }
        if let Some(start) = token_start.take() {
            push_token(text, start, index, &mut tokens);
        }
    }
    if let Some(start) = token_start {
        push_token(text, start, text.len(), &mut tokens);
    }

    tokens
}

fn push_token(text: &str, start: usize, end: usize, tokens: &mut Vec<Token>) {
    let normalized = text[start..end].to_lowercase();
    if !normalized.is_empty() {
        tokens.push(Token {
            normalized,
            source_range: TextRange { start, end },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LocalLexicalFallbackAdapter, SemanticEmbeddingInput, SemanticSearchAdapter,
        SemanticSearchDocument, SemanticSearchError, SemanticSearchMode, SemanticSearchQuery,
        SemanticVector, LOCAL_LEXICAL_FALLBACK_ADAPTER_ID, SEMANTIC_SEARCH_SCHEMA_VERSION,
    };

    fn query(text: &str, limit: usize) -> SemanticSearchQuery {
        SemanticSearchQuery {
            schema_version: SEMANTIC_SEARCH_SCHEMA_VERSION,
            text: text.to_owned(),
            limit,
            minimum_score: None,
        }
    }

    fn document(id: &str, text: &str) -> SemanticSearchDocument {
        SemanticSearchDocument {
            document_id: id.to_owned(),
            text: text.to_owned(),
            vector: None,
        }
    }

    #[test]
    fn ranks_local_documents_deterministically_and_returns_highlights() {
        let adapter = LocalLexicalFallbackAdapter;
        let documents = [
            document("partial", "Quick links for the team"),
            document("exact", "Quick paste"),
            document("unrelated", "A color palette"),
        ];

        let response = adapter
            .search(&query("quick paste", 5), &documents)
            .expect("lexical fallback search succeeds");

        assert_eq!(response.mode, SemanticSearchMode::LexicalFallback);
        assert_eq!(
            response.adapter.adapter_id,
            LOCAL_LEXICAL_FALLBACK_ADAPTER_ID
        );
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].document_id, "exact");
        assert_eq!(response.results[0].score, 1.0);
        assert_eq!(response.results[0].matched_terms, ["paste", "quick"]);
        assert_eq!(response.results[0].highlight_ranges[0].start, 0);
        assert_eq!(response.results[0].highlight_ranges[0].end, 5);
        assert_eq!(response.results[0].highlight_ranges[1].start, 6);
        assert_eq!(response.results[0].highlight_ranges[1].end, 11);
    }

    #[test]
    fn uses_document_id_to_break_equal_score_ties_and_honors_limit() {
        let adapter = LocalLexicalFallbackAdapter;
        let documents = [
            document("zeta", "clipriva history"),
            document("alpha", "clipriva history"),
        ];

        let response = adapter
            .search(&query("clipriva", 1), &documents)
            .expect("lexical fallback search succeeds");

        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].document_id, "alpha");
    }

    #[test]
    fn validates_queries_and_reports_missing_embedding_capability() {
        let adapter = LocalLexicalFallbackAdapter;
        let document = document("one", "ClipRiva history");
        let mut invalid_score = query("clipriva", 3);
        invalid_score.minimum_score = Some(1.1);

        assert_eq!(
            adapter.search(&query("   ", 3), std::slice::from_ref(&document)),
            Err(SemanticSearchError::EmptyQuery)
        );
        assert_eq!(
            adapter.search(&invalid_score, &[document]),
            Err(SemanticSearchError::InvalidMinimumScore)
        );
        assert_eq!(
            adapter.embed(&[SemanticEmbeddingInput {
                document_id: "one".to_owned(),
                text: "ClipRiva history".to_owned(),
            }]),
            Err(SemanticSearchError::EmbeddingsUnsupported)
        );
    }

    #[test]
    fn keeps_vector_and_result_shapes_serializable() {
        let adapter = LocalLexicalFallbackAdapter;
        let vector = SemanticVector {
            schema_version: SEMANTIC_SEARCH_SCHEMA_VERSION,
            document_id: "item-1".to_owned(),
            adapter: adapter.descriptor(),
            values: vec![0.25, -0.5],
        };
        let response = adapter
            .search(
                &query("clipriva", 3),
                &[document("item-1", "ClipRiva history")],
            )
            .expect("search succeeds");
        let vector_json = serde_json::to_value(&vector).expect("vector serializes");
        let response_json = serde_json::to_value(&response).expect("response serializes");

        assert_eq!(vector_json["schemaVersion"], SEMANTIC_SEARCH_SCHEMA_VERSION);
        assert_eq!(vector_json["adapter"]["supportsEmbeddings"], false);
        assert_eq!(response_json["mode"], "lexical_fallback");
        assert_eq!(
            serde_json::from_value::<SemanticVector>(vector_json).expect("vector deserializes"),
            vector
        );
    }
}
