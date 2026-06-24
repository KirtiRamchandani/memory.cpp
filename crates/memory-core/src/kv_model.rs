//! KV-cache footprint estimator.
//!
//! This module estimates how many bytes of key/value cache a sequence of a
//! given length would occupy for a given model profile. It is an **estimate**,
//! not a measurement: real runtimes apply paging, quantization, and other
//! optimizations. memory.cpp does not compress a provider's KV cache — it
//! reduces KV *pressure* by sending fewer tokens. This estimator quantifies
//! that "fewer tokens" effect in bytes so the savings are legible.
//!
//! Approximate formula:
//!
//! ```text
//! kv_bytes ≈ seq_len × layers × kv_heads × head_dim × 2 (K and V) × bytes_per_element × quant_factor
//! ```

use serde::{Deserialize, Serialize};

/// A model's KV-relevant shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelProfile {
    pub model: String,
    pub layers: u32,
    pub kv_heads: u32,
    pub head_dim: u32,
    pub dtype_bytes: u32,
    pub context_length: u32,
    /// Multiplier for KV quantization (1.0 = none, 0.5 = fp8-ish, 0.25 = int4).
    #[serde(default = "default_quant")]
    pub quantization_factor: f32,
}

fn default_quant() -> f32 {
    1.0
}

impl ModelProfile {
    /// Built-in profiles for common open models (public architecture specs).
    pub fn preset(name: &str) -> Option<ModelProfile> {
        let p = match name.trim().to_ascii_lowercase().as_str() {
            "llama-3.1-8b" | "llama-3-8b" | "llama-8b" => ModelProfile {
                model: "llama-3.1-8b".into(),
                layers: 32,
                kv_heads: 8,
                head_dim: 128,
                dtype_bytes: 2,
                context_length: 131072,
                quantization_factor: 1.0,
            },
            "llama-3.1-70b" | "llama-70b" => ModelProfile {
                model: "llama-3.1-70b".into(),
                layers: 80,
                kv_heads: 8,
                head_dim: 128,
                dtype_bytes: 2,
                context_length: 131072,
                quantization_factor: 1.0,
            },
            "mistral-7b" => ModelProfile {
                model: "mistral-7b".into(),
                layers: 32,
                kv_heads: 8,
                head_dim: 128,
                dtype_bytes: 2,
                context_length: 32768,
                quantization_factor: 1.0,
            },
            "qwen2.5-7b" | "qwen-7b" => ModelProfile {
                model: "qwen2.5-7b".into(),
                layers: 28,
                kv_heads: 4,
                head_dim: 128,
                dtype_bytes: 2,
                context_length: 131072,
                quantization_factor: 1.0,
            },
            "generic-7b" | "generic" => ModelProfile {
                model: "generic-7b".into(),
                layers: 32,
                kv_heads: 8,
                head_dim: 128,
                dtype_bytes: 2,
                context_length: 32768,
                quantization_factor: 1.0,
            },
            _ => return None,
        };
        Some(p)
    }

    /// Apply a runtime's typical default (e.g. vLLM/SGLang often run fp16 KV;
    /// llama.cpp/ollama commonly use quantized KV). These only set the
    /// quantization factor on top of a base profile.
    pub fn with_runtime(mut self, runtime: &str) -> ModelProfile {
        self.quantization_factor = match runtime.trim().to_ascii_lowercase().as_str() {
            "llama.cpp" | "ollama" => 0.5, // commonly q8_0 KV
            "vllm" | "sglang" => 1.0,      // commonly fp16 KV
            _ => self.quantization_factor,
        };
        self
    }

    /// Estimated KV bytes for a sequence of `seq_len` tokens.
    pub fn kv_bytes(&self, seq_len: usize) -> u64 {
        let per_token = (self.layers as u64)
            * (self.kv_heads as u64)
            * (self.head_dim as u64)
            * 2 // K and V
            * (self.dtype_bytes as u64);
        let raw = per_token.saturating_mul(seq_len as u64);
        (raw as f64 * self.quantization_factor as f64) as u64
    }
}

/// Side-by-side raw vs compiled KV estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvComparison {
    pub model: String,
    pub raw_tokens: usize,
    pub compiled_tokens: usize,
    pub stable_prefix_tokens: usize,
    pub fresh_suffix_tokens: usize,
    pub estimated_kv_bytes_raw: u64,
    pub estimated_kv_bytes_compiled: u64,
    pub estimated_kv_bytes_avoided: u64,
    pub estimated_kv_percent_reduction: f32,
    pub note: String,
}

impl KvComparison {
    pub fn new(
        profile: &ModelProfile,
        raw_tokens: usize,
        compiled_tokens: usize,
        stable_prefix_tokens: usize,
        fresh_suffix_tokens: usize,
    ) -> Self {
        let raw = profile.kv_bytes(raw_tokens);
        let compiled = profile.kv_bytes(compiled_tokens);
        let avoided = raw.saturating_sub(compiled);
        let pct = if raw == 0 {
            0.0
        } else {
            (avoided as f32 / raw as f32) * 100.0
        };
        KvComparison {
            model: profile.model.clone(),
            raw_tokens,
            compiled_tokens,
            stable_prefix_tokens,
            fresh_suffix_tokens,
            estimated_kv_bytes_raw: raw,
            estimated_kv_bytes_compiled: compiled,
            estimated_kv_bytes_avoided: avoided,
            estimated_kv_percent_reduction: pct,
            note: "Estimate only. memory.cpp reduces KV pressure by sending fewer tokens; it does not compress a provider's KV cache.".to_string(),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Human-friendly MiB rendering.
pub fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_known_and_unknown() {
        assert!(ModelProfile::preset("llama-3.1-8b").is_some());
        assert!(ModelProfile::preset("not-a-model").is_none());
    }

    #[test]
    fn kv_bytes_scale_linearly_with_tokens() {
        let p = ModelProfile::preset("llama-3.1-8b").unwrap();
        assert_eq!(p.kv_bytes(2000), p.kv_bytes(1000) * 2);
    }

    #[test]
    fn quantization_reduces_estimate() {
        let p = ModelProfile::preset("llama-3.1-8b")
            .unwrap()
            .with_runtime("llama.cpp");
        let f = ModelProfile::preset("llama-3.1-8b").unwrap();
        assert!(p.kv_bytes(1000) < f.kv_bytes(1000));
    }

    #[test]
    fn comparison_reports_savings() {
        let p = ModelProfile::preset("llama-3.1-8b").unwrap();
        let c = KvComparison::new(&p, 84000, 1420, 1100, 320);
        assert!(c.estimated_kv_bytes_avoided > 0);
        assert!(c.estimated_kv_percent_reduction > 90.0);
        assert!(c.to_json()["model"].is_string());
    }
}
