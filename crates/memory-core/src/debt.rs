//! Context debt score.
//!
//! A single 0–100 number (higher = worse) summarizing how unhealthy a repo's
//! AI context is: stale memories, unresolved contradictions, duplicates,
//! unverified high-impact facts, secret-risk memories, prefix instability,
//! trace bloat, and outdated test recipes. Comes with the top causes and a
//! concrete fix plan.

use serde::{Deserialize, Serialize};

/// Raw counts/rates feeding the debt score.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DebtFactors {
    pub stale_memories: usize,
    pub unresolved_contradictions: usize,
    pub duplicate_memories: usize,
    pub unverified_high_impact: usize,
    pub secret_risk_memories: usize,
    /// 0.0 (always stable) .. 1.0 (changes every run).
    pub unstable_prefix_rate: f32,
    pub trace_bloat_blocks: usize,
    pub outdated_test_recipes: usize,
    /// Total memories, used to normalize counts.
    pub total_memories: usize,
}

/// The computed debt report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtReport {
    pub score: u32,
    pub grade: String,
    pub top_causes: Vec<String>,
    pub fix_plan: Vec<String>,
    pub factors: DebtFactors,
}

impl DebtReport {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

fn grade_for(score: u32) -> &'static str {
    match score {
        0..=19 => "A (healthy)",
        20..=39 => "B (minor debt)",
        40..=59 => "C (notable debt)",
        60..=79 => "D (high debt)",
        _ => "F (critical debt)",
    }
}

/// Compute the debt score. Each factor contributes capped, weighted points.
pub fn score(factors: DebtFactors) -> DebtReport {
    let denom = factors.total_memories.max(1) as f32;
    // Severity-weighted contributions (each capped so one factor can't dominate
    // beyond its weight).
    let stale = ((factors.stale_memories as f32 / denom) * 100.0).min(20.0);
    let contradictions = (factors.unresolved_contradictions as f32 * 6.0).min(20.0);
    let duplicates = ((factors.duplicate_memories as f32 / denom) * 60.0).min(12.0);
    let unverified = (factors.unverified_high_impact as f32 * 3.0).min(15.0);
    let secrets = (factors.secret_risk_memories as f32 * 8.0).min(20.0);
    let prefix = (factors.unstable_prefix_rate.clamp(0.0, 1.0) * 15.0).min(15.0);
    let bloat = (factors.trace_bloat_blocks as f32 * 2.0).min(8.0);
    let tests = (factors.outdated_test_recipes as f32 * 2.0).min(10.0);

    let raw = stale + contradictions + duplicates + unverified + secrets + prefix + bloat + tests;
    let score = raw.round().clamp(0.0, 100.0) as u32;

    let mut causes: Vec<(f32, String)> = vec![
        (stale, format!("{} stale memories", factors.stale_memories)),
        (
            contradictions,
            format!(
                "{} unresolved contradictions",
                factors.unresolved_contradictions
            ),
        ),
        (
            secrets,
            format!("{} secret-risk memories", factors.secret_risk_memories),
        ),
        (
            unverified,
            format!(
                "{} unverified high-impact facts",
                factors.unverified_high_impact
            ),
        ),
        (
            prefix,
            format!(
                "stable prefix instability {:.0}%",
                factors.unstable_prefix_rate * 100.0
            ),
        ),
        (
            duplicates,
            format!("{} duplicate memories", factors.duplicate_memories),
        ),
    ];
    causes.retain(|(weight, _)| *weight > 0.5);
    causes.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let top_causes: Vec<String> = causes.into_iter().take(4).map(|(_, c)| c).collect();

    let mut fix_plan = Vec::new();
    if factors.stale_memories > 0 || factors.outdated_test_recipes > 0 {
        fix_plan.push("run 'memory prove' and 'memory audit-stale', then clean stale".into());
    }
    if factors.unresolved_contradictions > 0 {
        fix_plan.push("run 'memory contradictions resolve' (prefer newer/human-confirmed)".into());
    }
    if factors.secret_risk_memories > 0 {
        fix_plan.push("quarantine secret-risk memories via the write firewall".into());
    }
    if factors.unstable_prefix_rate > 0.2 {
        fix_plan.push("promote durable invariants and keep task text out of the prefix".into());
    }
    if factors.duplicate_memories > 0 {
        fix_plan.push("run 'memory gc' to compact duplicate summaries".into());
    }
    if fix_plan.is_empty() {
        fix_plan.push("context is healthy — keep locking packs to preserve it".into());
    }

    DebtReport {
        score,
        grade: grade_for(score).to_string(),
        top_causes,
        fix_plan,
        factors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_repo_scores_low() {
        let r = score(DebtFactors {
            total_memories: 50,
            ..Default::default()
        });
        assert!(r.score < 20);
        assert!(r.grade.starts_with('A'));
    }

    #[test]
    fn unhealthy_repo_scores_high() {
        let r = score(DebtFactors {
            stale_memories: 37,
            unresolved_contradictions: 9,
            unverified_high_impact: 14,
            secret_risk_memories: 3,
            unstable_prefix_rate: 1.0,
            total_memories: 60,
            ..Default::default()
        });
        assert!(r.score >= 60, "score was {}", r.score);
        assert!(!r.top_causes.is_empty());
        assert!(!r.fix_plan.is_empty());
    }

    #[test]
    fn score_is_bounded() {
        let r = score(DebtFactors {
            stale_memories: 10_000,
            unresolved_contradictions: 10_000,
            secret_risk_memories: 10_000,
            unstable_prefix_rate: 5.0,
            total_memories: 1,
            ..Default::default()
        });
        assert!(r.score <= 100);
    }
}
