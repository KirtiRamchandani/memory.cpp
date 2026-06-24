//! Minimal sufficient context search.
//!
//! Given a set of candidate context blocks and an *oracle* that decides whether
//! a candidate subset still "solves" the task, find the smallest subset that
//! keeps the oracle satisfied. This is delta-debugging applied to prompts: the
//! paper-grade question "what is the least context that still works?".
//!
//! The oracle is a pluggable predicate so the same search works whether the
//! oracle is "tests pass", "expected file edited", or "answer contains fact".

use serde::{Deserialize, Serialize};

/// A minimizable unit of context.
#[derive(Debug, Clone)]
pub struct MinBlock {
    pub id: String,
    pub text: String,
    pub tokens: usize,
}

/// Strategy for shrinking the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinStrategy {
    /// Try removing one block at a time, keep removals that stay satisfied.
    GreedyRemoval,
    /// Delta-debugging: try removing halves, then quarters, etc.
    DeltaDebug,
}

impl MinStrategy {
    pub fn parse(s: &str) -> MinStrategy {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "delta" | "delta_debug" | "binary" => MinStrategy::DeltaDebug,
            _ => MinStrategy::GreedyRemoval,
        }
    }
}

/// Outcome of a minimization run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimizeResult {
    pub raw_blocks: usize,
    pub raw_tokens: usize,
    pub minimized_blocks: usize,
    pub minimized_tokens: usize,
    pub essential_ids: Vec<String>,
    pub removed_ids: Vec<String>,
    pub oracle_satisfied: bool,
}

impl MinimizeResult {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

fn tokens_of(blocks: &[MinBlock], keep: &[bool]) -> usize {
    blocks
        .iter()
        .zip(keep)
        .filter(|(_, k)| **k)
        .map(|(b, _)| b.tokens)
        .sum()
}

/// Find a minimal sufficient subset. `oracle` receives the kept blocks and
/// returns whether the task is still satisfied.
pub fn minimize<F>(blocks: &[MinBlock], strategy: MinStrategy, oracle: F) -> MinimizeResult
where
    F: Fn(&[&MinBlock]) -> bool,
{
    let raw_tokens: usize = blocks.iter().map(|b| b.tokens).sum();
    let satisfied_with = |keep: &[bool]| {
        let kept: Vec<&MinBlock> = blocks
            .iter()
            .zip(keep)
            .filter(|(_, k)| **k)
            .map(|(b, _)| b)
            .collect();
        oracle(&kept)
    };

    let mut keep = vec![true; blocks.len()];

    // If the full set doesn't satisfy the oracle, nothing to minimize.
    if !satisfied_with(&keep) {
        return MinimizeResult {
            raw_blocks: blocks.len(),
            raw_tokens,
            minimized_blocks: blocks.len(),
            minimized_tokens: raw_tokens,
            essential_ids: blocks.iter().map(|b| b.id.clone()).collect(),
            removed_ids: vec![],
            oracle_satisfied: false,
        };
    }

    match strategy {
        MinStrategy::GreedyRemoval => {
            for i in 0..blocks.len() {
                keep[i] = false;
                if !satisfied_with(&keep) {
                    keep[i] = true; // needed, put it back
                }
            }
        }
        MinStrategy::DeltaDebug => {
            let mut granularity = 2usize;
            loop {
                let indices: Vec<usize> = (0..blocks.len()).filter(|&i| keep[i]).collect();
                if indices.len() < 2 {
                    break;
                }
                let chunk = indices.len().div_ceil(granularity);
                let mut removed_any = false;
                let mut start = 0;
                while start < indices.len() {
                    let end = (start + chunk).min(indices.len());
                    let mut trial = keep.clone();
                    for &idx in &indices[start..end] {
                        trial[idx] = false;
                    }
                    if satisfied_with(&trial) {
                        keep = trial;
                        removed_any = true;
                    }
                    start += chunk;
                }
                if !removed_any {
                    if granularity >= indices.len() {
                        break;
                    }
                    granularity = (granularity * 2).min(indices.len());
                } else {
                    granularity = 2;
                }
            }
            // Final greedy pass to clean up singletons.
            for i in 0..blocks.len() {
                if keep[i] {
                    keep[i] = false;
                    if !satisfied_with(&keep) {
                        keep[i] = true;
                    }
                }
            }
        }
    }

    let essential_ids: Vec<String> = blocks
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(b, _)| b.id.clone())
        .collect();
    let removed_ids: Vec<String> = blocks
        .iter()
        .zip(&keep)
        .filter(|(_, k)| !**k)
        .map(|(b, _)| b.id.clone())
        .collect();

    MinimizeResult {
        raw_blocks: blocks.len(),
        raw_tokens,
        minimized_blocks: essential_ids.len(),
        minimized_tokens: tokens_of(blocks, &keep),
        essential_ids,
        removed_ids,
        oracle_satisfied: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks() -> Vec<MinBlock> {
        vec![
            MinBlock {
                id: "a".into(),
                text: "needed alpha".into(),
                tokens: 10,
            },
            MinBlock {
                id: "b".into(),
                text: "filler".into(),
                tokens: 10,
            },
            MinBlock {
                id: "c".into(),
                text: "needed beta".into(),
                tokens: 10,
            },
            MinBlock {
                id: "d".into(),
                text: "more filler".into(),
                tokens: 10,
            },
        ]
    }

    // Oracle: the kept set must contain both "alpha" and "beta".
    fn oracle(kept: &[&MinBlock]) -> bool {
        let joined: String = kept
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        joined.contains("alpha") && joined.contains("beta")
    }

    #[test]
    fn greedy_keeps_only_essentials() {
        let r = minimize(&blocks(), MinStrategy::GreedyRemoval, oracle);
        assert!(r.oracle_satisfied);
        assert_eq!(r.minimized_blocks, 2);
        assert!(r.essential_ids.contains(&"a".to_string()));
        assert!(r.essential_ids.contains(&"c".to_string()));
        assert_eq!(r.minimized_tokens, 20);
    }

    #[test]
    fn delta_debug_also_finds_minimal_set() {
        let r = minimize(&blocks(), MinStrategy::DeltaDebug, oracle);
        assert!(r.oracle_satisfied);
        assert_eq!(r.minimized_blocks, 2);
    }

    #[test]
    fn unsatisfiable_oracle_keeps_everything() {
        let r = minimize(&blocks(), MinStrategy::GreedyRemoval, |_| false);
        assert!(!r.oracle_satisfied);
        assert_eq!(r.minimized_blocks, 4);
    }
}
