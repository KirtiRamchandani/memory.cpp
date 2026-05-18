use serde::{Deserialize, Serialize};

use crate::{MemoryEngine, RecallQuery, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    pub query: String,
    pub expected: String,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub query: String,
    pub expected: String,
    pub hit: bool,
    pub rank: Option<usize>,
    pub top_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub cases: usize,
    pub hits: usize,
    pub recall_at_k: f32,
    pub mean_reciprocal_rank: f32,
    pub results: Vec<EvalResult>,
}

pub fn evaluate(engine: &MemoryEngine, cases: &[EvalCase], limit: usize) -> Result<EvalReport> {
    let mut results = Vec::new();
    let mut hits = 0;
    let mut reciprocal_rank_sum = 0.0;

    for case in cases {
        let mut query = RecallQuery::new(&case.query).limit(limit.max(1));
        if let Some(scope) = &case.scope {
            query = query.scope(scope);
        }

        let recalled = engine.recall(query)?;
        let expected = case.expected.to_ascii_lowercase();
        let rank = recalled.iter().position(|item| {
            item.memory.summary.to_ascii_lowercase().contains(&expected)
                || item.memory.content.to_ascii_lowercase().contains(&expected)
        });

        if let Some(rank) = rank {
            hits += 1;
            reciprocal_rank_sum += 1.0 / (rank + 1) as f32;
        }

        results.push(EvalResult {
            query: case.query.clone(),
            expected: case.expected.clone(),
            hit: rank.is_some(),
            rank: rank.map(|value| value + 1),
            top_score: recalled.first().map(|item| item.score),
        });
    }

    let cases_len = cases.len().max(1);
    Ok(EvalReport {
        cases: cases.len(),
        hits,
        recall_at_k: hits as f32 / cases_len as f32,
        mean_reciprocal_rank: reciprocal_rank_sum / cases_len as f32,
        results,
    })
}
