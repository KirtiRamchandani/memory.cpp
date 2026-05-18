use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct RankerConfig {
    pub similarity_weight: f32,
    pub keyword_weight: f32,
    pub graph_weight: f32,
    pub importance_weight: f32,
    pub recency_weight: f32,
    pub confidence_weight: f32,
    pub redundancy_penalty_weight: f32,
    pub sensitivity_penalty_weight: f32,
    pub recency_half_life_hours: f32,
}

impl Default for RankerConfig {
    fn default() -> Self {
        Self {
            similarity_weight: 0.35,
            keyword_weight: 0.20,
            graph_weight: 0.15,
            importance_weight: 0.15,
            recency_weight: 0.10,
            confidence_weight: 0.05,
            redundancy_penalty_weight: 0.05,
            sensitivity_penalty_weight: 0.05,
            recency_half_life_hours: 24.0 * 21.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ranker {
    config: RankerConfig,
}

impl Ranker {
    pub fn new(config: RankerConfig) -> Self {
        Self { config }
    }

    pub fn score(&self, features: RankFeatures) -> (f32, String) {
        let similarity = ((features.similarity + 1.0) / 2.0).clamp(0.0, 1.0);
        let keyword = features.keyword_score.clamp(0.0, 1.0);
        let graph = features.entity_score.clamp(0.0, 1.0);
        let importance = features.importance.clamp(0.0, 1.0);
        let recency = recency_score(features.created_at, self.config.recency_half_life_hours);
        let confidence = features.confidence.clamp(0.0, 1.0);
        let redundancy_penalty = redundancy_penalty(features.access_count);
        let sensitivity_penalty = if features.is_sensitive { 1.0 } else { 0.0 };

        let score = similarity * self.config.similarity_weight
            + keyword * self.config.keyword_weight
            + graph * self.config.graph_weight
            + importance * self.config.importance_weight
            + recency * self.config.recency_weight
            + confidence * self.config.confidence_weight
            - redundancy_penalty * self.config.redundancy_penalty_weight
            - sensitivity_penalty * self.config.sensitivity_penalty_weight;

        let reason = format!(
            "semantic={similarity:.3}, keyword={keyword:.3}, entity={graph:.3}, importance={importance:.3}, recency={recency:.3}, confidence={confidence:.3}, redundancy_penalty={redundancy_penalty:.3}, sensitivity_penalty={sensitivity_penalty:.3}"
        );

        (score.clamp(0.0, 1.0), reason)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RankFeatures {
    pub similarity: f32,
    pub keyword_score: f32,
    pub entity_score: f32,
    pub importance: f32,
    pub confidence: f32,
    pub created_at: DateTime<Utc>,
    pub access_count: u64,
    pub is_sensitive: bool,
}

fn recency_score(created_at: DateTime<Utc>, half_life_hours: f32) -> f32 {
    let age_hours = (Utc::now() - created_at).num_hours().max(0) as f32;
    1.0 / (1.0 + age_hours / half_life_hours.max(1.0))
}

fn redundancy_penalty(access_count: u64) -> f32 {
    ((access_count as f32).ln_1p() / 8.0).clamp(0.0, 1.0)
}
