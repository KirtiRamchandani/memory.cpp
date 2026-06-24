//! Machine-readable cache manifest.
//!
//! A [`CacheManifest`] describes how a compiled context pack should be split
//! into cacheable and non-cacheable regions so that providers and local
//! runtimes can reuse prefill / KV state for the stable prefix.
//!
//! Important honesty note: memory.cpp does **not** directly compress a closed
//! provider's KV cache. It reduces KV *pressure* by keeping unnecessary, stale,
//! or duplicated tokens out of the prompt, and by canonicalizing a stable
//! prefix that providers can cache. The numbers reported here are estimates of
//! *token positions avoided*, not measured runtime speedups.

use serde::{Deserialize, Serialize};

use crate::prefix::{pack_id, stable_prefix_hash};

/// The compiler version stamped into manifests and lockfiles.
pub fn compiler_version() -> String {
    format!("memory.cpp-compiler/{}", env!("CARGO_PKG_VERSION"))
}

/// A contiguous token region of the compiled prompt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheRegion {
    pub name: String,
    pub start_token: usize,
    pub end_token: usize,
    pub cacheable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_hint: Option<String>,
}

/// Full cache manifest for one compiled pack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheManifest {
    pub pack_id: String,
    pub stable_prefix_hash: String,
    pub repo_state_hash: String,
    pub task_class: String,
    pub provider: String,
    pub compiler_version: String,
    pub stable_prefix_token_count: usize,
    pub fresh_suffix_token_count: usize,
    pub cache_regions: Vec<CacheRegion>,
    pub cache_breaker_warnings: Vec<String>,
    pub suggested_provider_strategy: String,
    pub estimated_repeated_tokens_avoided: usize,
    pub estimated_kv_positions_avoided: usize,
}

/// Inputs required to build a manifest.
pub struct CacheManifestInput<'a> {
    pub stable_prefix: &'a str,
    pub fresh_suffix: &'a str,
    pub provider: &'a str,
    pub task_class: &'a str,
    pub repo_state_hash: &'a str,
    pub stable_prefix_tokens: usize,
    pub fresh_suffix_tokens: usize,
    /// How many times we expect the stable prefix to be reused before it
    /// changes (used only to estimate repeated tokens avoided).
    pub expected_reuse: usize,
    pub cache_breaker_warnings: Vec<String>,
}

/// Provider-specific cache strategy guidance.
fn provider_strategy(provider: &str) -> String {
    match provider.to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => {
            "Place the stable prefix in a cache_control breakpoint; keep the fresh suffix outside the cached block."
        }
        "openai" | "codex" => {
            "Send the byte-stable prefix first so automatic prefix caching applies; append the fresh suffix last."
        }
        "gemini" => {
            "Register the stable prefix as cachedContent (branch TTL); send the fresh suffix as the live request."
        }
        _ => {
            "Reuse the stable prefix across requests where your runtime supports prefix/KV reuse; vary only the fresh suffix."
        }
    }
    .to_string()
}

/// Suggested time-to-live for the cacheable region.
fn ttl_hint(provider: &str) -> String {
    match provider.to_ascii_lowercase().as_str() {
        "gemini" => "branch (explicit cachedContent TTL)".to_string(),
        _ => "branch".to_string(),
    }
}

impl CacheManifest {
    pub fn build(input: CacheManifestInput<'_>) -> Self {
        let stable_tokens = input.stable_prefix_tokens;
        let fresh_tokens = input.fresh_suffix_tokens;
        let regions = vec![
            CacheRegion {
                name: "stable_repo_prefix".to_string(),
                start_token: 0,
                end_token: stable_tokens,
                cacheable: true,
                ttl_hint: Some(ttl_hint(input.provider)),
            },
            CacheRegion {
                name: "fresh_task_suffix".to_string(),
                start_token: stable_tokens,
                end_token: stable_tokens.saturating_add(fresh_tokens),
                cacheable: false,
                ttl_hint: None,
            },
        ];
        // Repeated tokens avoided: every reuse after the first saves a full
        // reprocess of the stable prefix.
        let estimated_repeated_tokens_avoided =
            stable_tokens.saturating_mul(input.expected_reuse.saturating_sub(1));

        CacheManifest {
            pack_id: pack_id(
                input.stable_prefix,
                input.fresh_suffix,
                input.provider,
                input.task_class,
            ),
            stable_prefix_hash: stable_prefix_hash(input.stable_prefix),
            repo_state_hash: input.repo_state_hash.to_string(),
            task_class: input.task_class.to_string(),
            provider: input.provider.to_string(),
            compiler_version: compiler_version(),
            stable_prefix_token_count: stable_tokens,
            fresh_suffix_token_count: fresh_tokens,
            cache_regions: regions,
            cache_breaker_warnings: input.cache_breaker_warnings,
            suggested_provider_strategy: provider_strategy(input.provider),
            estimated_repeated_tokens_avoided,
            // KV positions scale with cached prefix length: each reused position
            // is a position the runtime does not need to recompute prefill for.
            estimated_kv_positions_avoided: estimated_repeated_tokens_avoided,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefix::{
        canonicalize_stable_prefix, render_fresh_suffix, FreshSuffixInput, StablePrefixInput,
    };

    fn manifest_for(task: &str) -> CacheManifest {
        let prefix = canonicalize_stable_prefix(&StablePrefixInput {
            repo_rules: vec!["Use pnpm, never npm.".into()],
            ..Default::default()
        });
        let suffix = render_fresh_suffix(&FreshSuffixInput {
            task: task.into(),
            ..Default::default()
        });
        CacheManifest::build(CacheManifestInput {
            stable_prefix: &prefix,
            fresh_suffix: &suffix,
            provider: "anthropic",
            task_class: "bugfix",
            repo_state_hash: "git:abc123",
            stable_prefix_tokens: 4200,
            fresh_suffix_tokens: 900,
            expected_reuse: 10,
            cache_breaker_warnings: vec![],
        })
    }

    #[test]
    fn manifest_has_two_regions_with_correct_offsets() {
        let m = manifest_for("fix checkout bug");
        assert_eq!(m.cache_regions.len(), 2);
        assert!(m.cache_regions[0].cacheable);
        assert!(!m.cache_regions[1].cacheable);
        assert_eq!(m.cache_regions[0].end_token, 4200);
        assert_eq!(m.cache_regions[1].start_token, 4200);
        assert_eq!(m.cache_regions[1].end_token, 5100);
    }

    #[test]
    fn stable_prefix_hash_is_task_independent() {
        let a = manifest_for("fix checkout bug");
        let b = manifest_for("totally different task string");
        assert_eq!(a.stable_prefix_hash, b.stable_prefix_hash);
        // But the pack id covers the fresh suffix, so it must differ.
        assert_ne!(a.pack_id, b.pack_id);
    }

    #[test]
    fn estimates_repeated_tokens_avoided() {
        let m = manifest_for("fix checkout bug");
        assert_eq!(m.estimated_repeated_tokens_avoided, 4200 * 9);
        assert!(m.to_json()["pack_id"].is_string());
    }
}
