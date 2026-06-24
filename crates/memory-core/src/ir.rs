//! MemoryIR — a formal intermediate representation between raw repository
//! memory and the final compiled prompt pack.
//!
//! The compiler pipeline is:
//!
//! ```text
//! repo/files/git/traces/memories
//!   -> MemoryIR            (this module)
//!   -> optimizer passes    (optimizer.rs)
//!   -> stable prefix + fresh suffix (prefix.rs)
//!   -> provider-specific prompt
//!   -> cache manifest      (cache_manifest.rs)
//!   -> provenance ledger   (ledger.rs)
//!   -> optional lockfile   (lockfile.rs)
//! ```
//!
//! Representing context as typed IR blocks (instead of opaque strings) is what
//! lets every later pass reason about volatility, cache role, freshness, trust,
//! and token cost in a uniform way.

use serde::{Deserialize, Serialize};

use crate::prefix::content_hash;
use crate::types::{MemoryKind, MemoryStatus, RetrievedMemory};

/// The semantic category of an IR block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrKind {
    Fact,
    Invariant,
    Decision,
    Bug,
    Fix,
    ApiContract,
    FileSummary,
    TraceSummary,
    TestRecipe,
    NegativeMemory,
    FreshnessProof,
    SecurityWarning,
    RehydrationHandle,
    CacheRegion,
}

impl IrKind {
    pub fn as_str(self) -> &'static str {
        match self {
            IrKind::Fact => "fact",
            IrKind::Invariant => "invariant",
            IrKind::Decision => "decision",
            IrKind::Bug => "bug",
            IrKind::Fix => "fix",
            IrKind::ApiContract => "api_contract",
            IrKind::FileSummary => "file_summary",
            IrKind::TraceSummary => "trace_summary",
            IrKind::TestRecipe => "test_recipe",
            IrKind::NegativeMemory => "negative_memory",
            IrKind::FreshnessProof => "freshness_proof",
            IrKind::SecurityWarning => "security_warning",
            IrKind::RehydrationHandle => "rehydration_handle",
            IrKind::CacheRegion => "cache_region",
        }
    }
}

/// Where a block should land in the compiled pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheRole {
    /// Durable, low-volatility material safe to reuse byte-for-byte.
    StablePrefix,
    /// Volatile, per-request material.
    FreshSuffix,
    /// Deliberately excluded (see [`IrBlock::exclusion_reason`]).
    Excluded,
}

impl CacheRole {
    pub fn as_str(self) -> &'static str {
        match self {
            CacheRole::StablePrefix => "stable_prefix",
            CacheRole::FreshSuffix => "fresh_suffix",
            CacheRole::Excluded => "excluded",
        }
    }
}

/// How quickly a block's content is expected to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Volatility {
    Static,
    Low,
    Medium,
    High,
    Volatile,
}

impl Volatility {
    /// A 0.0 (never changes) .. 1.0 (changes every request) score.
    pub fn score(self) -> f32 {
        match self {
            Volatility::Static => 0.0,
            Volatility::Low => 0.25,
            Volatility::Medium => 0.5,
            Volatility::High => 0.75,
            Volatility::Volatile => 1.0,
        }
    }
}

/// One unit of compiled context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrBlock {
    pub id: String,
    pub text: String,
    pub kind: IrKind,
    pub source_memory_id: Option<String>,
    pub source_file: Option<String>,
    pub source_line_range: Option<(u64, u64)>,
    pub source_commit: Option<String>,
    pub content_hash: String,
    pub token_count: usize,
    /// Retrieval relevance to the current task (from the recall score).
    pub relevance_score: f32,
    pub utility_score: f32,
    pub freshness_score: f32,
    pub trust_score: f32,
    pub sensitivity_score: f32,
    pub volatility: Volatility,
    pub volatility_score: f32,
    pub cache_role: CacheRole,
    pub evidence_ids: Vec<String>,
    pub exclusion_reason: Option<String>,
}

/// A bundle of IR blocks plus convenience views.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryIr {
    pub blocks: Vec<IrBlock>,
}

/// Char/4 token approximation. Replaced by the tokenizer module for
/// provider-aware counts; kept here as a dependency-free default.
fn approx_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    if chars == 0 {
        0
    } else {
        chars.div_ceil(4)
    }
}

/// Map a stored memory's kind/tags/text to an IR kind.
fn classify_ir_kind(kind: MemoryKind, tags: &[String], text: &str) -> IrKind {
    let lower = text.to_ascii_lowercase();
    let has_tag = |needle: &str| tags.iter().any(|t| t.eq_ignore_ascii_case(needle));

    if has_tag("security") || lower.contains("vulnerab") || lower.contains("secret") {
        return IrKind::SecurityWarning;
    }
    if has_tag("invariant")
        || lower.contains("must always")
        || lower.contains("must be deterministic")
    {
        return IrKind::Invariant;
    }
    if has_tag("api") || has_tag("contract") || lower.contains("api contract") {
        return IrKind::ApiContract;
    }
    if has_tag("negative") || has_tag("do_not_touch") || lower.contains("do not send") {
        return IrKind::NegativeMemory;
    }
    if has_tag("test") || lower.contains("cargo test") || lower.contains("test command") {
        return IrKind::TestRecipe;
    }
    if has_tag("trace") || has_tag("tool_trace_summary") {
        return IrKind::TraceSummary;
    }
    if has_tag("fix") || lower.starts_with("fixed ") || lower.contains("the fix was") {
        return IrKind::Fix;
    }
    if has_tag("file_summary") {
        return IrKind::FileSummary;
    }
    match kind {
        MemoryKind::Decision => IrKind::Decision,
        MemoryKind::Bug => IrKind::Bug,
        MemoryKind::Code => IrKind::FileSummary,
        MemoryKind::Summary => IrKind::FileSummary,
        _ => IrKind::Fact,
    }
}

/// Default volatility for an IR kind.
fn volatility_for(kind: IrKind) -> Volatility {
    match kind {
        IrKind::Invariant | IrKind::ApiContract => Volatility::Static,
        IrKind::Fact
        | IrKind::Decision
        | IrKind::SecurityWarning
        | IrKind::NegativeMemory
        | IrKind::TestRecipe
        | IrKind::RehydrationHandle
        | IrKind::FreshnessProof
        | IrKind::CacheRegion => Volatility::Low,
        IrKind::FileSummary | IrKind::Fix => Volatility::Medium,
        IrKind::Bug | IrKind::TraceSummary => Volatility::High,
    }
}

impl IrBlock {
    /// Build an IR block from a retrieved memory, assigning kind, scores, and
    /// cache role. Stale/superseded/contradicted memories are marked excluded.
    pub fn from_retrieved(item: &RetrievedMemory) -> Self {
        let memory = &item.memory;
        let text = if memory.summary.trim().is_empty() {
            memory.content.clone()
        } else {
            memory.summary.clone()
        };
        let kind = classify_ir_kind(memory.kind, &memory.attributes.tags, &text);
        let volatility = volatility_for(kind);

        let exclusion_reason = match memory.attributes.status {
            MemoryStatus::Superseded => Some("superseded".to_string()),
            MemoryStatus::Contradicted => Some("contradicted".to_string()),
            MemoryStatus::Archived => Some("archived".to_string()),
            MemoryStatus::Forgotten => Some("forgotten".to_string()),
            _ if memory.attributes.tags.iter().any(|t| t == "stale") => Some("stale".to_string()),
            _ => None,
        };

        let cache_role = if exclusion_reason.is_some() {
            CacheRole::Excluded
        } else if matches!(volatility, Volatility::Static | Volatility::Low) {
            CacheRole::StablePrefix
        } else {
            CacheRole::FreshSuffix
        };

        let source = memory.attributes.source.as_ref();
        let source_file = source.and_then(|s| s.source_file.clone());
        let source_commit = source.and_then(|s| s.source_commit.clone());
        let source_line_range = source.and_then(|s| s.source_line).map(|line| (line, line));

        let hash = content_hash("blk", text.as_bytes());
        let id = format!("ir:{}", &hash[hash.len() - 16..]);
        IrBlock {
            id,
            content_hash: hash,
            token_count: approx_tokens(&text),
            relevance_score: item.score,
            kind,
            source_memory_id: Some(memory.id.clone()),
            source_file,
            source_line_range,
            source_commit,
            utility_score: memory.derived.usefulness,
            freshness_score: memory.derived.freshness,
            trust_score: memory.derived.trust,
            sensitivity_score: memory.derived.sensitivity,
            volatility,
            volatility_score: volatility.score(),
            cache_role,
            evidence_ids: source.and_then(|s| s.source.clone()).into_iter().collect(),
            exclusion_reason,
            text,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

impl MemoryIr {
    pub fn from_retrieved(items: &[RetrievedMemory]) -> Self {
        MemoryIr {
            blocks: items.iter().map(IrBlock::from_retrieved).collect(),
        }
    }

    pub fn stable_blocks(&self) -> impl Iterator<Item = &IrBlock> {
        self.blocks
            .iter()
            .filter(|b| b.cache_role == CacheRole::StablePrefix)
    }

    pub fn fresh_blocks(&self) -> impl Iterator<Item = &IrBlock> {
        self.blocks
            .iter()
            .filter(|b| b.cache_role == CacheRole::FreshSuffix)
    }

    pub fn excluded_blocks(&self) -> impl Iterator<Item = &IrBlock> {
        self.blocks
            .iter()
            .filter(|b| b.cache_role == CacheRole::Excluded)
    }

    pub fn total_tokens(&self) -> usize {
        self.blocks.iter().map(|b| b.token_count).sum()
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "block_count": self.blocks.len(),
            "total_tokens": self.total_tokens(),
            "stable_blocks": self.stable_blocks().count(),
            "fresh_blocks": self.fresh_blocks().count(),
            "excluded_blocks": self.excluded_blocks().count(),
            "blocks": self.blocks.iter().map(|b| b.to_json()).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MemoryAttributes, MemoryDerivedScores, StoredMemory};
    use chrono::Utc;

    fn retrieved(
        kind: MemoryKind,
        text: &str,
        status: MemoryStatus,
        tags: Vec<String>,
    ) -> RetrievedMemory {
        let now = Utc::now();
        RetrievedMemory {
            memory: StoredMemory {
                id: format!("mem-{text}"),
                kind,
                scope: "repo".into(),
                content: text.into(),
                summary: text.into(),
                metadata: serde_json::Value::Null,
                importance: 0.7,
                created_at: now,
                updated_at: now,
                last_accessed_at: None,
                access_count: 0,
                attributes: MemoryAttributes {
                    tags,
                    status,
                    ..Default::default()
                },
                derived: MemoryDerivedScores::default(),
            },
            score: 0.9,
            similarity: 0.8,
            keyword_score: 0.5,
            entity_score: 0.0,
            confidence_score: 0.8,
            reason: "test".into(),
        }
    }

    #[test]
    fn durable_memory_goes_to_stable_prefix() {
        let ir = MemoryIr::from_retrieved(&[retrieved(
            MemoryKind::Decision,
            "ADR-004: use server-side sessions",
            MemoryStatus::Active,
            vec![],
        )]);
        assert_eq!(ir.stable_blocks().count(), 1);
        assert_eq!(ir.blocks[0].kind, IrKind::Decision);
    }

    #[test]
    fn volatile_memory_goes_to_fresh_suffix() {
        let ir = MemoryIr::from_retrieved(&[retrieved(
            MemoryKind::Bug,
            "checkout panics on empty cart",
            MemoryStatus::Active,
            vec![],
        )]);
        assert_eq!(ir.fresh_blocks().count(), 1);
    }

    #[test]
    fn superseded_memory_is_excluded_with_reason() {
        let ir = MemoryIr::from_retrieved(&[retrieved(
            MemoryKind::Fact,
            "use npm to install",
            MemoryStatus::Superseded,
            vec![],
        )]);
        assert_eq!(ir.excluded_blocks().count(), 1);
        assert_eq!(ir.blocks[0].exclusion_reason.as_deref(), Some("superseded"));
    }

    #[test]
    fn invariant_is_static_and_stable() {
        let ir = MemoryIr::from_retrieved(&[retrieved(
            MemoryKind::Fact,
            "Billing exports must be deterministic",
            MemoryStatus::Active,
            vec!["invariant".into()],
        )]);
        assert_eq!(ir.blocks[0].kind, IrKind::Invariant);
        assert_eq!(ir.blocks[0].volatility, Volatility::Static);
        assert_eq!(ir.blocks[0].cache_role, CacheRole::StablePrefix);
    }

    #[test]
    fn ir_json_has_counts() {
        let ir = MemoryIr::from_retrieved(&[
            retrieved(
                MemoryKind::Decision,
                "a durable decision",
                MemoryStatus::Active,
                vec![],
            ),
            retrieved(MemoryKind::Bug, "a fresh bug", MemoryStatus::Active, vec![]),
        ]);
        let json = ir.to_json();
        assert_eq!(json["block_count"], 2);
        assert!(json["total_tokens"].as_u64().unwrap() > 0);
    }
}
