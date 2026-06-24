//! KV-aware context optimizer.
//!
//! Context selection should not be relevance-only. A block that is relevant but
//! stale, low-trust, huge, or highly volatile can cost more (in tokens, cache
//! instability, and KV pressure) than it is worth. The optimizer scores every
//! IR block by *utility per cost* and selects a minimal, high-value subset
//! under a token (and optional KV) budget.
//!
//! ```text
//! utility = relevance × trust × freshness × importance
//! cost    = tokens × kv_cost × volatility_penalty
//! score   = utility / cost
//! ```

use serde::{Deserialize, Serialize};

use crate::ir::{CacheRole, IrBlock, IrKind};

/// Optimization objective. Each tweaks how utility and cost are weighted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    MaxQuality,
    MinTokens,
    KvEfficiency,
    CacheHitRate,
    FreshnessFirst,
    SecurityFirst,
    MinimalSufficient,
}

impl Objective {
    pub fn parse(s: &str) -> Option<Objective> {
        Some(
            match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
                "max_quality" | "quality" => Objective::MaxQuality,
                "min_tokens" | "tokens" => Objective::MinTokens,
                "kv_efficiency" | "kv" => Objective::KvEfficiency,
                "cache_hit_rate" | "cache" => Objective::CacheHitRate,
                "freshness_first" | "freshness" => Objective::FreshnessFirst,
                "security_first" | "security" => Objective::SecurityFirst,
                "minimal_sufficient" | "minimal" => Objective::MinimalSufficient,
                _ => return None,
            },
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Objective::MaxQuality => "max_quality",
            Objective::MinTokens => "min_tokens",
            Objective::KvEfficiency => "kv_efficiency",
            Objective::CacheHitRate => "cache_hit_rate",
            Objective::FreshnessFirst => "freshness_first",
            Objective::SecurityFirst => "security_first",
            Objective::MinimalSufficient => "minimal_sufficient",
        }
    }
}

/// Compiler-style optimization level (`-O0`.. `-Oz`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    O0,
    O1,
    O2,
    O3,
    Oz,
    OSafe,
    OCache,
}

impl OptLevel {
    pub fn parse(flag: &str) -> Option<OptLevel> {
        Some(match flag.trim() {
            "-O0" | "O0" => OptLevel::O0,
            "-O1" | "O1" => OptLevel::O1,
            "-O2" | "O2" => OptLevel::O2,
            "-O3" | "O3" => OptLevel::O3,
            "-Oz" | "Oz" => OptLevel::Oz,
            "-Osafe" | "Osafe" => OptLevel::OSafe,
            "-Ocache" | "Ocache" => OptLevel::OCache,
            _ => return None,
        })
    }

    pub fn objective(self) -> Objective {
        match self {
            OptLevel::O0 => Objective::MaxQuality,
            OptLevel::O1 => Objective::MaxQuality,
            OptLevel::O2 => Objective::FreshnessFirst,
            OptLevel::O3 => Objective::KvEfficiency,
            OptLevel::Oz => Objective::MinimalSufficient,
            OptLevel::OSafe => Objective::SecurityFirst,
            OptLevel::OCache => Objective::CacheHitRate,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            OptLevel::O0 => "-O0",
            OptLevel::O1 => "-O1",
            OptLevel::O2 => "-O2",
            OptLevel::O3 => "-O3",
            OptLevel::Oz => "-Oz",
            OptLevel::OSafe => "-Osafe",
            OptLevel::OCache => "-Ocache",
        }
    }
}

/// Optimizer configuration.
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    pub objective: Objective,
    pub token_budget: usize,
    /// Optional KV byte ceiling. When set, selection also stops once the
    /// estimated KV bytes would exceed this.
    pub max_kv_bytes: Option<u64>,
    /// Estimated KV bytes per token (from a [`crate::kv_model::ModelProfile`]).
    pub kv_bytes_per_token: u64,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        OptimizerConfig {
            objective: Objective::MinimalSufficient,
            token_budget: 1500,
            max_kv_bytes: None,
            kv_bytes_per_token: 0,
        }
    }
}

/// A block plus its score and inclusion decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredBlock {
    pub id: String,
    pub kind: String,
    pub cache_role: String,
    pub tokens: usize,
    pub score: f64,
    pub included: bool,
    pub reason: String,
}

/// Result of an optimization pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub objective: String,
    pub token_budget: usize,
    pub selected_tokens: usize,
    pub selected: Vec<ScoredBlock>,
    pub excluded: Vec<ScoredBlock>,
}

impl OptimizationResult {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// A block that must always be included regardless of budget.
fn is_mandatory(block: &IrBlock, objective: Objective) -> bool {
    match block.kind {
        IrKind::Invariant | IrKind::SecurityWarning => true,
        IrKind::NegativeMemory => true,
        _ => objective == Objective::SecurityFirst && block.kind == IrKind::ApiContract,
    }
}

/// Compute the utility-per-cost score for a block under an objective.
fn score_block(block: &IrBlock, cfg: &OptimizerConfig) -> f64 {
    let relevance = block.relevance_score.clamp(0.0, 1.0) as f64;
    let trust = block.trust_score.clamp(0.0, 1.0) as f64;
    let freshness = block.freshness_score.clamp(0.0, 1.0) as f64;
    let importance = block.utility_score.clamp(0.0, 1.0) as f64;

    // Base utility, with a small floor so a zero factor doesn't nuke the score.
    let mut utility = (0.1 + relevance) * (0.1 + trust) * (0.1 + freshness) * (0.1 + importance);

    // Sensitivity always hurts utility a little; security-first hurts a lot.
    let sensitivity = block.sensitivity_score.clamp(0.0, 1.0) as f64;
    utility *= 1.0 - 0.5 * sensitivity;

    let volatility = block.volatility_score.clamp(0.0, 1.0) as f64;
    let tokens = block.token_count.max(1) as f64;

    // Cost grows with tokens and volatility (volatile blocks also add KV churn).
    let mut cost = tokens * (1.0 + volatility);

    match cfg.objective {
        Objective::MaxQuality => {
            // De-emphasize cost; prioritize raw utility.
            cost = cost.powf(0.4);
        }
        Objective::MinTokens | Objective::MinimalSufficient => {
            // Strongly penalize tokens.
            cost = tokens * (1.0 + volatility);
        }
        Objective::KvEfficiency => {
            // Penalize volatility (volatile blocks break cache and add KV churn).
            cost = tokens * (1.0 + 2.0 * volatility);
        }
        Objective::CacheHitRate => {
            // Reward stable-prefix blocks heavily.
            if block.cache_role == CacheRole::StablePrefix {
                utility *= 1.75;
            } else {
                cost *= 1.5;
            }
        }
        Objective::FreshnessFirst => {
            utility *= 0.5 + freshness;
        }
        Objective::SecurityFirst => {
            utility *= 1.0 - 0.8 * sensitivity;
        }
    }

    utility / cost.max(1.0)
}

/// Run the optimizer over a set of IR blocks.
pub fn optimize(blocks: &[IrBlock], cfg: &OptimizerConfig) -> OptimizationResult {
    let mut scored: Vec<(f64, &IrBlock, bool)> = blocks
        .iter()
        .filter(|b| b.cache_role != CacheRole::Excluded)
        .map(|b| (score_block(b, cfg), b, is_mandatory(b, cfg.objective)))
        .collect();

    // Mandatory blocks first, then by score descending.
    scored.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then(b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut selected = Vec::new();
    let mut excluded = Vec::new();
    let mut used_tokens = 0usize;
    let mut used_kv: u64 = 0;

    for (score, block, mandatory) in scored {
        let next_tokens = used_tokens + block.token_count;
        let next_kv = used_kv + cfg.kv_bytes_per_token * block.token_count as u64;
        let over_tokens = next_tokens > cfg.token_budget;
        let over_kv = cfg.max_kv_bytes.map(|max| next_kv > max).unwrap_or(false);

        let entry = ScoredBlock {
            id: block.id.clone(),
            kind: block.kind.as_str().to_string(),
            cache_role: block.cache_role.as_str().to_string(),
            tokens: block.token_count,
            score: (score * 10000.0).round() / 10000.0,
            included: false,
            reason: String::new(),
        };

        if mandatory {
            let mut e = entry;
            e.included = true;
            e.reason = "mandatory (invariant/security/negative)".to_string();
            used_tokens = next_tokens;
            used_kv = next_kv;
            selected.push(e);
        } else if over_tokens {
            let mut e = entry;
            e.reason = "exceeds token budget".to_string();
            excluded.push(e);
        } else if over_kv {
            let mut e = entry;
            e.reason = "exceeds KV budget".to_string();
            excluded.push(e);
        } else {
            let mut e = entry;
            e.included = true;
            e.reason = "selected by utility/cost".to_string();
            used_tokens = next_tokens;
            used_kv = next_kv;
            selected.push(e);
        }
    }

    // Note excluded blocks (those filtered as Excluded earlier).
    for block in blocks
        .iter()
        .filter(|b| b.cache_role == CacheRole::Excluded)
    {
        excluded.push(ScoredBlock {
            id: block.id.clone(),
            kind: block.kind.as_str().to_string(),
            cache_role: "excluded".to_string(),
            tokens: block.token_count,
            score: 0.0,
            included: false,
            reason: block
                .exclusion_reason
                .clone()
                .unwrap_or_else(|| "excluded".to_string()),
        });
    }

    OptimizationResult {
        objective: cfg.objective.as_str().to_string(),
        token_budget: cfg.token_budget,
        selected_tokens: used_tokens,
        selected,
        excluded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::MemoryIr;
    use crate::types::{
        MemoryAttributes, MemoryDerivedScores, MemoryKind, RetrievedMemory, StoredMemory,
    };
    use chrono::Utc;

    fn mem(
        text: &str,
        kind: MemoryKind,
        tags: Vec<String>,
        score: f32,
        tokens_pad: usize,
    ) -> RetrievedMemory {
        let now = Utc::now();
        let content = format!("{text} {}", "x".repeat(tokens_pad));
        RetrievedMemory {
            memory: StoredMemory {
                id: format!("m-{text}"),
                kind,
                scope: "repo".into(),
                content: content.clone(),
                summary: content,
                metadata: serde_json::Value::Null,
                importance: 0.7,
                created_at: now,
                updated_at: now,
                last_accessed_at: None,
                access_count: 0,
                attributes: MemoryAttributes {
                    tags,
                    ..Default::default()
                },
                derived: MemoryDerivedScores {
                    freshness: 0.9,
                    usefulness: 0.8,
                    trust: 0.8,
                    ..Default::default()
                },
            },
            score,
            similarity: score,
            keyword_score: 0.0,
            entity_score: 0.0,
            confidence_score: 0.8,
            reason: "t".into(),
        }
    }

    #[test]
    fn budget_limits_selection() {
        let ir = MemoryIr::from_retrieved(&[
            mem("a relevant decision", MemoryKind::Decision, vec![], 0.9, 0),
            mem(
                "a giant low-value note",
                MemoryKind::Note,
                vec![],
                0.2,
                4000,
            ),
        ]);
        let result = optimize(
            &ir.blocks,
            &OptimizerConfig {
                token_budget: 50,
                ..Default::default()
            },
        );
        assert!(result.selected_tokens <= 50);
        assert!(result.excluded.iter().any(|b| b.reason.contains("budget")));
    }

    #[test]
    fn invariants_are_always_included() {
        let ir = MemoryIr::from_retrieved(&[mem(
            "Billing exports must be deterministic",
            MemoryKind::Fact,
            vec!["invariant".into()],
            0.1,
            5000,
        )]);
        let result = optimize(
            &ir.blocks,
            &OptimizerConfig {
                token_budget: 5,
                ..Default::default()
            },
        );
        assert!(result
            .selected
            .iter()
            .any(|b| b.reason.contains("mandatory")));
    }

    #[test]
    fn cache_hit_objective_prefers_stable_blocks() {
        let ir = MemoryIr::from_retrieved(&[
            mem("durable decision", MemoryKind::Decision, vec![], 0.6, 10),
            mem("volatile bug trace", MemoryKind::Bug, vec![], 0.6, 10),
        ]);
        let result = optimize(
            &ir.blocks,
            &OptimizerConfig {
                objective: Objective::CacheHitRate,
                token_budget: 1000,
                ..Default::default()
            },
        );
        let first = &result.selected[0];
        assert_eq!(first.cache_role, "stable_prefix");
    }

    #[test]
    fn opt_levels_map_to_objectives() {
        assert_eq!(
            OptLevel::parse("-Oz").unwrap().objective(),
            Objective::MinimalSufficient
        );
        assert_eq!(
            OptLevel::parse("-Ocache").unwrap().objective(),
            Objective::CacheHitRate
        );
        assert!(OptLevel::parse("-Onope").is_none());
    }
}
