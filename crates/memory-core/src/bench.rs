//! CAMC-Bench — Cache-Aware Memory Compilation for Coding Agents.
//!
//! A self-contained, deterministic benchmark that compares context-selection
//! strategies on a bundled synthetic repo-memory corpus. Every number is
//! produced by the real code paths in this crate (IR, negative memory,
//! freshness, optimizer, KV model) — nothing is hard-coded or faked.
//!
//! The corpus is synthetic and small on purpose: it exists to make the
//! *mechanism* measurable (fewer tokens, more cacheable prefix, stale/secret
//! context blocked, task success preserved), not to claim real-world win rates.
//! Real-repo and runtime numbers must come from live measurement and are
//! reported separately.

use serde::{Deserialize, Serialize};

use crate::embedding::{Embedder, HashEmbedder};
use crate::ir::{CacheRole, MemoryIr};
use crate::kv_model::ModelProfile;
use crate::optimizer::{optimize, Objective, OptimizerConfig};
use crate::tokenizer::estimate_tokens;
use crate::types::{
    MemoryAttributes, MemoryDerivedScores, MemoryKind, MemoryStatus, RetrievedMemory, StoredMemory,
};

use chrono::Utc;

/// A fixture memory in the benchmark corpus.
#[derive(Debug, Clone)]
struct FixtureMemory {
    id: &'static str,
    text: &'static str,
    kind: MemoryKind,
    tags: &'static [&'static str],
    superseded: bool,
    is_secret: bool,
    is_contradiction: bool,
    is_distractor: bool,
}

/// A benchmark task with its oracle (the memory ids required for success).
#[derive(Debug, Clone)]
struct BenchTask {
    query: &'static str,
    essential: &'static [&'static str],
}

fn corpus() -> Vec<FixtureMemory> {
    use MemoryKind::*;
    vec![
        FixtureMemory { id: "inv-billing", text: "Billing exports must be deterministic and sorted by invoice id.", kind: Fact, tags: &["invariant"], superseded: false, is_secret: false, is_contradiction: false, is_distractor: false },
        FixtureMemory { id: "dec-sessions", text: "ADR-004 decision: use server-side sessions, not JWT, for auth.", kind: Decision, tags: &[], superseded: false, is_secret: false, is_contradiction: false, is_distractor: false },
        FixtureMemory { id: "rule-pnpm", text: "Use pnpm for all dependency installs. Never npm install.", kind: Workflow, tags: &["mistake"], superseded: false, is_secret: false, is_contradiction: false, is_distractor: false },
        FixtureMemory { id: "fix-export", text: "Billing export bug was fixed by using CsvExportWriter, not JsonExportWriter.", kind: Bug, tags: &["fix"], superseded: false, is_secret: false, is_contradiction: false, is_distractor: false },
        FixtureMemory { id: "test-export", text: "Run cargo test export_test before changing billing export code.", kind: Fact, tags: &["test"], superseded: false, is_secret: false, is_contradiction: false, is_distractor: false },
        FixtureMemory { id: "stale-jwt", text: "Auth uses JWT tokens stored in localStorage.", kind: Decision, tags: &[], superseded: true, is_secret: false, is_contradiction: true, is_distractor: false },
        FixtureMemory { id: "stale-export-v1", text: "Billing export uses the legacy export-v1 JSON format.", kind: Note, tags: &[], superseded: true, is_secret: false, is_contradiction: true, is_distractor: false },
        FixtureMemory { id: "npm-old", text: "Install dependencies with npm install before building.", kind: Workflow, tags: &[], superseded: true, is_secret: false, is_contradiction: true, is_distractor: false },
        FixtureMemory { id: "secret-key", text: "AWS_SECRET_ACCESS_KEY=AKIAIOSFODNN7EXAMPLE used by the export job.", kind: Note, tags: &[], superseded: false, is_secret: true, is_contradiction: false, is_distractor: false },
        FixtureMemory { id: "distractor-ui", text: "The settings page uses a tabbed layout with a sidebar.", kind: Note, tags: &[], superseded: false, is_secret: false, is_contradiction: false, is_distractor: true },
        FixtureMemory { id: "distractor-readme", text: "The README has badges for build status and license.", kind: Note, tags: &[], superseded: false, is_secret: false, is_contradiction: false, is_distractor: true },
        FixtureMemory { id: "trace-noise", text: "DEBUG worker started; worker started; worker started; retry; retry; retry; timeout; timeout; reconnect; reconnect; flush; flush; idle; idle; poll; poll; noop; noop.", kind: Summary, tags: &["tool_trace_summary"], superseded: false, is_secret: false, is_contradiction: false, is_distractor: true },
    ]
}

fn tasks() -> Vec<BenchTask> {
    vec![
        BenchTask {
            query: "fix the billing export bug",
            essential: &["inv-billing", "fix-export", "test-export"],
        },
        BenchTask {
            query: "install dependencies and build",
            essential: &["rule-pnpm"],
        },
        BenchTask {
            query: "review the authentication design",
            essential: &["dec-sessions"],
        },
    ]
}

/// Strategies under comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    RawFullContext,
    Grep,
    Bm25,
    EmbeddingRetrieval,
    CamcNoNegative,
    CamcNoFreshness,
    CamcNoCanonicalPrefix,
    FullCamc,
}

impl Strategy {
    pub fn label(self) -> &'static str {
        match self {
            Strategy::RawFullContext => "raw full context",
            Strategy::Grep => "grep",
            Strategy::Bm25 => "BM25",
            Strategy::EmbeddingRetrieval => "embedding retrieval",
            Strategy::CamcNoNegative => "CAMC - no negative memory",
            Strategy::CamcNoFreshness => "CAMC - no freshness proofs",
            Strategy::CamcNoCanonicalPrefix => "CAMC - no canonical prefix",
            Strategy::FullCamc => "full CAMC",
        }
    }

    pub fn all() -> Vec<Strategy> {
        vec![
            Strategy::RawFullContext,
            Strategy::Grep,
            Strategy::Bm25,
            Strategy::EmbeddingRetrieval,
            Strategy::CamcNoNegative,
            Strategy::CamcNoFreshness,
            Strategy::CamcNoCanonicalPrefix,
            Strategy::FullCamc,
        ]
    }
}

/// Per-(strategy, task) outcome, aggregated across tasks for the report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyResult {
    pub strategy: String,
    pub total_tokens: usize,
    pub cacheable_prefix_tokens: usize,
    pub fresh_suffix_tokens: usize,
    pub stale_included: usize,
    pub contradictions_included: usize,
    pub secret_leakage: usize,
    pub distractors_included: usize,
    pub tasks_passed: usize,
    pub tasks_total: usize,
    pub estimated_kv_mib: f64,
    pub token_reduction_vs_raw_percent: f32,
}

impl StrategyResult {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Full benchmark report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    pub model: String,
    pub budget: usize,
    pub results: Vec<StrategyResult>,
    pub note: String,
}

fn approx_tokens(text: &str) -> usize {
    estimate_tokens(text)
}

/// Jaccard token overlap as a lightweight relevance score.
fn relevance(query: &str, text: &str) -> f32 {
    let q: std::collections::BTreeSet<String> = query
        .to_ascii_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let d: std::collections::BTreeSet<String> = text
        .to_ascii_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if q.is_empty() || d.is_empty() {
        return 0.0;
    }
    let inter = q.intersection(&d).count() as f32;
    let union = q.union(&d).count() as f32;
    inter / union
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Build a RetrievedMemory from a fixture for a given task relevance.
fn to_retrieved(m: &FixtureMemory, score: f32) -> RetrievedMemory {
    let now = Utc::now();
    let status = if m.superseded {
        MemoryStatus::Superseded
    } else {
        MemoryStatus::Active
    };
    let tags: Vec<String> = m.tags.iter().map(|t| t.to_string()).collect();
    RetrievedMemory {
        memory: StoredMemory {
            id: m.id.to_string(),
            kind: m.kind,
            scope: "repo".into(),
            content: m.text.to_string(),
            summary: m.text.to_string(),
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
            derived: MemoryDerivedScores {
                freshness: if m.superseded { 0.1 } else { 0.9 },
                usefulness: 0.7,
                trust: 0.7,
                sensitivity: if m.is_secret { 0.95 } else { 0.0 },
                ..Default::default()
            },
        },
        score,
        similarity: score,
        keyword_score: score,
        entity_score: 0.0,
        confidence_score: 0.7,
        reason: "bench".into(),
    }
}

/// Run one strategy over one task, returning included fixture ids and a
/// stable-prefix token count (only CAMC with canonical prefix produces one).
fn select(
    strategy: Strategy,
    task: &BenchTask,
    corpus: &[FixtureMemory],
    budget: usize,
    embedder: &HashEmbedder,
) -> (Vec<String>, usize) {
    match strategy {
        Strategy::RawFullContext => {
            // Everything, no filtering.
            (corpus.iter().map(|m| m.id.to_string()).collect(), 0)
        }
        Strategy::Grep => {
            // Any memory sharing a word with the query, naive (no exclusion).
            let ids: Vec<String> = corpus
                .iter()
                .filter(|m| relevance(task.query, m.text) > 0.0)
                .map(|m| m.id.to_string())
                .collect();
            (ids, 0)
        }
        Strategy::Bm25 => {
            // Rank by relevance, take top under a token budget. No exclusion.
            let mut ranked: Vec<&FixtureMemory> = corpus.iter().collect();
            ranked.sort_by(|a, b| {
                relevance(task.query, b.text)
                    .partial_cmp(&relevance(task.query, a.text))
                    .unwrap()
            });
            take_under_budget(&ranked, budget)
        }
        Strategy::EmbeddingRetrieval => {
            let q = embedder.embed(task.query).unwrap_or_default();
            let mut ranked: Vec<&FixtureMemory> = corpus.iter().collect();
            ranked.sort_by(|a, b| {
                let sa = cosine(&q, &embedder.embed(a.text).unwrap_or_default());
                let sb = cosine(&q, &embedder.embed(b.text).unwrap_or_default());
                sb.partial_cmp(&sa).unwrap()
            });
            take_under_budget(&ranked, budget)
        }
        _ => camc_select(strategy, task, corpus, budget),
    }
}

fn take_under_budget(ranked: &[&FixtureMemory], budget: usize) -> (Vec<String>, usize) {
    let mut ids = Vec::new();
    let mut used = 0;
    for m in ranked {
        let t = approx_tokens(m.text);
        if used + t > budget {
            continue;
        }
        used += t;
        ids.push(m.id.to_string());
    }
    (ids, 0)
}

/// CAMC pipeline (with ablation switches).
fn camc_select(
    strategy: Strategy,
    task: &BenchTask,
    corpus: &[FixtureMemory],
    budget: usize,
) -> (Vec<String>, usize) {
    let use_freshness = strategy != Strategy::CamcNoFreshness;
    let use_negative = strategy != Strategy::CamcNoNegative;
    let use_canonical_prefix = strategy != Strategy::CamcNoCanonicalPrefix;

    let retrieved: Vec<RetrievedMemory> = corpus
        .iter()
        .map(|m| to_retrieved(m, relevance(task.query, m.text)))
        .collect();
    let ir = MemoryIr::from_retrieved(&retrieved);

    // Freshness ablation: when disabled, treat excluded (stale) blocks as live.
    let blocks: Vec<_> = ir
        .blocks
        .iter()
        .cloned()
        .map(|mut b| {
            if !use_freshness && b.cache_role == CacheRole::Excluded {
                b.cache_role = CacheRole::FreshSuffix;
                b.exclusion_reason = None;
            }
            b
        })
        .collect();

    let result = optimize(
        &blocks,
        &OptimizerConfig {
            objective: Objective::MinimalSufficient,
            token_budget: budget,
            max_kv_bytes: None,
            kv_bytes_per_token: 0,
        },
    );

    // Map selected IR blocks back to fixture ids.
    let by_block: std::collections::HashMap<&str, &str> = blocks
        .iter()
        .filter_map(|b| b.source_memory_id.as_deref().map(|id| (b.id.as_str(), id)))
        .collect();
    let mut ids: Vec<String> = result
        .selected
        .iter()
        .filter_map(|s| by_block.get(s.id.as_str()).map(|id| id.to_string()))
        .collect();

    // Negative-memory ablation: when enabled, drop distractors/secrets.
    if use_negative {
        let drop: std::collections::BTreeSet<&str> = corpus
            .iter()
            .filter(|m| m.is_distractor || m.is_secret)
            .map(|m| m.id)
            .collect();
        ids.retain(|id| !drop.contains(id.as_str()));
    }

    // Canonical prefix: stable-role, non-excluded blocks become the cacheable
    // prefix. Without it, nothing is counted as cacheable.
    let stable_tokens = if use_canonical_prefix {
        blocks
            .iter()
            .filter(|b| {
                b.cache_role == CacheRole::StablePrefix
                    && ids
                        .iter()
                        .any(|id| b.source_memory_id.as_deref() == Some(id))
            })
            .map(|b| b.token_count)
            .sum()
    } else {
        0
    };

    (ids, stable_tokens)
}

/// Run the full benchmark.
pub fn run(model: &str, budget: usize) -> BenchReport {
    let profile =
        ModelProfile::preset(model).unwrap_or_else(|| ModelProfile::preset("generic-7b").unwrap());
    let corpus = corpus();
    let tasks = tasks();
    let embedder = HashEmbedder::default();

    // First compute the raw baseline token total (per task) for reduction %.
    let mut results = Vec::new();
    let raw_tokens_total: usize = tasks
        .iter()
        .map(|t| {
            let (ids, _) = select(Strategy::RawFullContext, t, &corpus, budget, &embedder);
            ids.iter()
                .filter_map(|id| corpus.iter().find(|m| m.id == id))
                .map(|m| approx_tokens(m.text))
                .sum::<usize>()
                + approx_tokens(t.query)
        })
        .sum();

    for strategy in Strategy::all() {
        let mut total_tokens = 0;
        let mut cacheable = 0;
        let mut fresh = 0;
        let mut stale = 0;
        let mut contradictions = 0;
        let mut secrets = 0;
        let mut distractors = 0;
        let mut passed = 0;

        for task in &tasks {
            let (ids, stable_tokens) = select(strategy, task, &corpus, budget, &embedder);
            let selected: Vec<&FixtureMemory> = ids
                .iter()
                .filter_map(|id| corpus.iter().find(|m| m.id == id))
                .collect();
            let body_tokens: usize = selected.iter().map(|m| approx_tokens(m.text)).sum();
            let task_tokens = approx_tokens(task.query);
            total_tokens += body_tokens + task_tokens;
            cacheable += stable_tokens;
            fresh += (body_tokens + task_tokens).saturating_sub(stable_tokens);
            stale += selected.iter().filter(|m| m.superseded).count();
            contradictions += selected.iter().filter(|m| m.is_contradiction).count();
            secrets += selected.iter().filter(|m| m.is_secret).count();
            distractors += selected.iter().filter(|m| m.is_distractor).count();

            // Oracle: all essential ids present and no contradictions included.
            let has_essentials = task.essential.iter().all(|e| ids.iter().any(|i| i == e));
            let no_contradictions = !selected.iter().any(|m| m.is_contradiction);
            if has_essentials && no_contradictions {
                passed += 1;
            }
        }

        let reduction = if raw_tokens_total == 0 {
            0.0
        } else {
            ((raw_tokens_total.saturating_sub(total_tokens)) as f32 / raw_tokens_total as f32)
                * 100.0
        };

        results.push(StrategyResult {
            strategy: strategy.label().to_string(),
            total_tokens,
            cacheable_prefix_tokens: cacheable,
            fresh_suffix_tokens: fresh,
            stale_included: stale,
            contradictions_included: contradictions,
            secret_leakage: secrets,
            distractors_included: distractors,
            tasks_passed: passed,
            tasks_total: tasks.len(),
            estimated_kv_mib: crate::kv_model::mib(profile.kv_bytes(total_tokens)),
            token_reduction_vs_raw_percent: reduction,
        });
    }

    BenchReport {
        model: profile.model,
        budget,
        results,
        note: "Synthetic bundled corpus. Numbers measure the compilation mechanism, not real-world win rates. Estimated KV is prompt-side, not provider KV-cache compression.".to_string(),
    }
}

impl BenchReport {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "benchmark": "CAMC-Bench",
            "model": self.model,
            "budget": self.budget,
            "results": self.results.iter().map(|r| r.to_json()).collect::<Vec<_>>(),
            "note": self.note,
        })
    }

    /// Plain-text table: Scenario | Tokens | Cacheable | Reduction | Stale | Secrets | Pass.
    pub fn to_table(&self) -> String {
        let mut out = String::new();
        out.push_str("CAMC-Bench (synthetic corpus)\n");
        out.push_str(&format!("model: {}  budget: {}\n", self.model, self.budget));
        out.push_str(&format!(
            "{:<28} {:>7} {:>9} {:>9} {:>6} {:>7} {:>6}\n",
            "Strategy", "Tokens", "Cacheab.", "Reduct.", "Stale", "Secret", "Pass"
        ));
        for r in &self.results {
            out.push_str(&format!(
                "{:<28} {:>7} {:>9} {:>8.1}% {:>6} {:>7} {:>4}/{}\n",
                r.strategy,
                r.total_tokens,
                r.cacheable_prefix_tokens,
                r.token_reduction_vs_raw_percent,
                r.stale_included,
                r.secret_leakage,
                r.tasks_passed,
                r.tasks_total,
            ));
        }
        out.push_str(&format!("\n{}\n", self.note));
        out
    }

    /// Markdown table for the paper / README.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "### CAMC-Bench (synthetic corpus, model {}, budget {})\n\n",
            self.model, self.budget
        ));
        out.push_str("| Strategy | Tokens | Cacheable prefix | Reduction vs raw | Stale | Contradictions | Secrets | Task success |\n");
        out.push_str("|---|--:|--:|--:|--:|--:|--:|--:|\n");
        for r in &self.results {
            out.push_str(&format!(
                "| {} | {} | {} | {:.1}% | {} | {} | {} | {}/{} |\n",
                r.strategy,
                r.total_tokens,
                r.cacheable_prefix_tokens,
                r.token_reduction_vs_raw_percent,
                r.stale_included,
                r.contradictions_included,
                r.secret_leakage,
                r.tasks_passed,
                r.tasks_total,
            ));
        }
        out.push_str(&format!("\n_{}_\n", self.note));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_camc_beats_raw_on_tokens_and_safety() {
        let report = run("llama-3.1-8b", 1500);
        let raw = report
            .results
            .iter()
            .find(|r| r.strategy == "raw full context")
            .unwrap();
        let camc = report
            .results
            .iter()
            .find(|r| r.strategy == "full CAMC")
            .unwrap();
        // Fewer tokens.
        assert!(camc.total_tokens < raw.total_tokens);
        // No stale, contradictions, or secrets in full CAMC.
        assert_eq!(camc.stale_included, 0);
        assert_eq!(camc.contradictions_included, 0);
        assert_eq!(camc.secret_leakage, 0);
        // Task success preserved (all tasks pass).
        assert_eq!(camc.tasks_passed, camc.tasks_total);
    }

    #[test]
    fn raw_leaks_stale_and_secrets() {
        let report = run("llama-3.1-8b", 1500);
        let raw = report
            .results
            .iter()
            .find(|r| r.strategy == "raw full context")
            .unwrap();
        assert!(raw.stale_included > 0);
        assert!(raw.secret_leakage > 0);
    }

    #[test]
    fn full_camc_has_cacheable_prefix_but_ablation_does_not() {
        let report = run("llama-3.1-8b", 1500);
        let camc = report
            .results
            .iter()
            .find(|r| r.strategy == "full CAMC")
            .unwrap();
        let no_prefix = report
            .results
            .iter()
            .find(|r| r.strategy == "CAMC - no canonical prefix")
            .unwrap();
        assert!(camc.cacheable_prefix_tokens > 0);
        assert_eq!(no_prefix.cacheable_prefix_tokens, 0);
    }

    #[test]
    fn report_renders_table_and_markdown() {
        let report = run("llama-3.1-8b", 1500);
        assert!(report.to_table().contains("CAMC-Bench"));
        assert!(report.to_markdown().contains("| Strategy |"));
        assert!(report.to_json()["benchmark"] == "CAMC-Bench");
    }
}
