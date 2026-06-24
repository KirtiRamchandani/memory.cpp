//! Agent failure autopsy.
//!
//! Given the context a coding agent was handed (as MemoryIR) plus the budget it
//! ran under, explain the most likely *context-side* reasons a run could fail:
//! stale context included, contradictions, missing invariants, secret/injection
//! risk, over-budget truncation, or an unstable prefix. The autopsy is a ranked
//! list of likely causes with concrete fixes.

use serde::{Deserialize, Serialize};

use crate::ir::{CacheRole, IrBlock, IrKind};

/// A single likely cause with a suggested fix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub category: String,
    pub severity: String,
    pub detail: String,
    pub fix: String,
}

/// The autopsy result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Autopsy {
    pub likely_failure: Option<String>,
    pub findings: Vec<Finding>,
}

impl Autopsy {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Inputs the autopsy reasons over.
pub struct AutopsyInput<'a> {
    pub blocks: &'a [IrBlock],
    pub budget: usize,
    pub compiled_tokens: usize,
    /// Essential memory ids the caller expected but that are absent.
    pub missing_essentials: &'a [String],
}

/// Run the autopsy.
pub fn analyze(input: &AutopsyInput<'_>) -> Autopsy {
    let mut findings = Vec::new();

    let included: Vec<&IrBlock> = input
        .blocks
        .iter()
        .filter(|b| b.cache_role != CacheRole::Excluded)
        .collect();

    // Stale context that slipped in (should be excluded, but a no-freshness
    // build or a manual pack might include it).
    let stale = input
        .blocks
        .iter()
        .filter(|b| b.exclusion_reason.is_some() && b.cache_role != CacheRole::Excluded)
        .count();
    if stale > 0 {
        findings.push(Finding {
            category: "stale_context_included".into(),
            severity: "high".into(),
            detail: format!("{stale} stale/superseded blocks are still active"),
            fix: "run 'memory audit-stale' and exclude them via negative memory".into(),
        });
    }

    // Missing essentials.
    if !input.missing_essentials.is_empty() {
        findings.push(Finding {
            category: "missing_context".into(),
            severity: "high".into(),
            detail: format!(
                "{} expected memories were not in the pack: {}",
                input.missing_essentials.len(),
                input.missing_essentials.join(", ")
            ),
            fix: "promote them (pin/raise confidence) or widen retrieval".into(),
        });
    }

    // No invariant present at all.
    let has_invariant = included.iter().any(|b| b.kind == IrKind::Invariant);
    if !has_invariant {
        findings.push(Finding {
            category: "invariant_missing".into(),
            severity: "medium".into(),
            detail: "no invariant block was included".into(),
            fix: "run 'memory invariants extract' or tag a durable rule as invariant".into(),
        });
    }

    // Sensitive content.
    let secrets = included
        .iter()
        .filter(|b| b.sensitivity_score > 0.6)
        .count();
    if secrets > 0 {
        findings.push(Finding {
            category: "prompt_injection_risk".into(),
            severity: "critical".into(),
            detail: format!("{secrets} sensitive blocks present in the pack"),
            fix: "enable the token/write firewall so secrets never compile in".into(),
        });
    }

    // Over budget / truncation.
    if input.compiled_tokens > input.budget {
        findings.push(Finding {
            category: "insufficient_budget".into(),
            severity: "medium".into(),
            detail: format!(
                "compiled {} tokens over a {} budget",
                input.compiled_tokens, input.budget
            ),
            fix: "raise the budget or compile with -Oz (minimal sufficient)".into(),
        });
    }

    // Unstable prefix: volatile blocks landed in the stable prefix.
    let unstable = included
        .iter()
        .filter(|b| b.cache_role == CacheRole::StablePrefix && b.volatility_score >= 0.5)
        .count();
    if unstable > 0 {
        findings.push(Finding {
            category: "unstable_prefix".into(),
            severity: "medium".into(),
            detail: format!("{unstable} volatile blocks are in the stable prefix"),
            fix: "move volatile blocks to the fresh suffix to keep the prefix cacheable".into(),
        });
    }

    // Rank by severity.
    let rank = |s: &str| match s {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        _ => 3,
    };
    findings.sort_by_key(|f| rank(&f.severity));

    let likely_failure = findings.first().map(|f| f.detail.clone());
    Autopsy {
        likely_failure,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::MemoryIr;
    use crate::types::{
        MemoryAttributes, MemoryDerivedScores, MemoryKind, MemoryStatus, RetrievedMemory,
        StoredMemory,
    };
    use chrono::Utc;

    fn rm(
        id: &str,
        text: &str,
        kind: MemoryKind,
        status: MemoryStatus,
        sens: f32,
    ) -> RetrievedMemory {
        let now = Utc::now();
        RetrievedMemory {
            memory: StoredMemory {
                id: id.into(),
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
                    status,
                    ..Default::default()
                },
                derived: MemoryDerivedScores {
                    sensitivity: sens,
                    ..Default::default()
                },
            },
            score: 0.8,
            similarity: 0.8,
            keyword_score: 0.0,
            entity_score: 0.0,
            confidence_score: 0.8,
            reason: "t".into(),
        }
    }

    #[test]
    fn flags_missing_invariant_and_budget() {
        let ir = MemoryIr::from_retrieved(&[rm(
            "a",
            "a fact",
            MemoryKind::Fact,
            MemoryStatus::Active,
            0.0,
        )]);
        let a = analyze(&AutopsyInput {
            blocks: &ir.blocks,
            budget: 100,
            compiled_tokens: 250,
            missing_essentials: &[],
        });
        assert!(a.findings.iter().any(|f| f.category == "invariant_missing"));
        assert!(a
            .findings
            .iter()
            .any(|f| f.category == "insufficient_budget"));
    }

    #[test]
    fn secrets_are_critical_and_ranked_first() {
        let ir = MemoryIr::from_retrieved(&[rm(
            "s",
            "AWS secret key value",
            MemoryKind::Note,
            MemoryStatus::Active,
            0.9,
        )]);
        let a = analyze(&AutopsyInput {
            blocks: &ir.blocks,
            budget: 5000,
            compiled_tokens: 100,
            missing_essentials: &[],
        });
        assert_eq!(a.findings[0].severity, "critical");
    }

    #[test]
    fn reports_missing_essentials() {
        let ir = MemoryIr::from_retrieved(&[rm(
            "a",
            "Billing must be deterministic",
            MemoryKind::Fact,
            MemoryStatus::Active,
            0.0,
        )]);
        let a = analyze(&AutopsyInput {
            blocks: &ir.blocks,
            budget: 5000,
            compiled_tokens: 100,
            missing_essentials: &["adr-004".to_string()],
        });
        assert!(a.findings.iter().any(|f| f.category == "missing_context"));
    }
}
