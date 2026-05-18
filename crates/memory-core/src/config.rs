use crate::{CompressionConfig, RankerConfig};

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub embedding_dim: usize,
    pub max_candidate_pool: usize,
    pub context_token_budget: usize,
    pub compaction_max_chars: usize,
    pub compression: CompressionConfig,
    pub ranker: RankerConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 384,
            max_candidate_pool: 256,
            context_token_budget: 1_200,
            compaction_max_chars: 8_000,
            compression: CompressionConfig::default(),
            ranker: RankerConfig::default(),
        }
    }
}
