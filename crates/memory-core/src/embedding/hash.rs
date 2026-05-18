use blake3::Hasher;

use super::Embedder;
use crate::{vector::l2_normalize, Result};

#[derive(Debug, Clone)]
pub struct HashEmbedder {
    dimensions: usize,
}

impl HashEmbedder {
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions: dimensions.max(32),
        }
    }
}

impl Default for HashEmbedder {
    fn default() -> Self {
        Self::new(384)
    }
}

impl Embedder for HashEmbedder {
    fn name(&self) -> &'static str {
        "hash"
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let tokens = tokenize(text);
        let mut vector = vec![0.0; self.dimensions];

        for token in &tokens {
            add_feature(&mut vector, token, 1.0);
        }

        for pair in tokens.windows(2) {
            let feature = format!("{}:{}", pair[0], pair[1]);
            add_feature(&mut vector, &feature, 0.65);
        }

        l2_normalize(&mut vector);
        Ok(vector)
    }
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '#') {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn add_feature(vector: &mut [f32], feature: &str, weight: f32) {
    let mut hasher = Hasher::new();
    hasher.update(feature.as_bytes());
    let hash = hasher.finalize();
    let bytes = hash.as_bytes();

    let raw = u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);

    let index = raw as usize % vector.len();
    let sign = if bytes[8] & 1 == 0 { 1.0 } else { -1.0 };
    let length_boost = 1.0 + (feature.len().min(16) as f32 / 32.0);

    vector[index] += sign * weight * length_boost;
}
