#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextControlOptions {
    pub task: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_budget")]
    pub budget: usize,
    #[serde(default)]
    pub context: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePlanOptions {
    pub task: String,
    #[serde(default = "default_runtime")]
    pub runtime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchPlanOptions {
    #[serde(default)]
    pub requests: Vec<String>,
    #[serde(default = "default_provider")]
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecordOptions {
    pub text: String,
    #[serde(default = "default_scope")]
    pub scope: String,
    #[serde(default = "default_memory_type")]
    pub memory_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextControlReport {
    pub compiled_prompt: String,
    pub stable_prefix: String,
    pub fresh_suffix: String,
    pub context_pack: String,
    pub cache_plan: String,
    pub kv_report: KvPressureReport,
    pub prefill_report: PrefillReport,
    pub signal_density: SignalDensityReport,
    pub token_firewall_report: TokenFirewallReport,
    pub warnings: Vec<String>,
    pub evidence: Vec<String>,
    pub omitted: Vec<String>,
    pub files_written: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvPressureReport {
    pub raw_context_tokens: usize,
    pub compiled_context_tokens: usize,
    pub estimated_kv_positions_avoided: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefillReport {
    pub raw_prompt_tokens: usize,
    pub compiled_prompt_tokens: usize,
    pub cacheable_prefix_tokens: usize,
    pub fresh_suffix_tokens: usize,
    pub estimated_prefill_reduction_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalDensityReport {
    pub useful_context_tokens: usize,
    pub duplicate_tokens: usize,
    pub stale_tokens: usize,
    pub low_relevance_tokens: usize,
    pub signal_density_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenFirewallReport {
    pub duplicate_context_tokens_blocked: usize,
    pub stale_context_tokens_blocked: usize,
    pub tool_trace_tokens_compressed: usize,
    pub secret_like_strings_blocked: usize,
    pub prompt_injection_warnings: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheAuditReport {
    pub provider: String,
    pub cache_hit_risk: String,
    pub problems: Vec<String>,
    pub fixes: Vec<String>,
    pub stable_prefix_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePlanReport {
    pub runtime: String,
    pub recommended_context_budget: usize,
    pub prefix_reuse_hint: String,
    pub kv_quantization_hint: String,
    pub speculative_decoding_hint: String,
    pub batching_hint: String,
    pub dynamic_suffix_placement: String,
    pub warning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchPlanReport {
    pub provider: String,
    pub batch_groups: Vec<BatchGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchGroup {
    pub group_id: String,
    pub shared_stable_prefix_token_count: usize,
    pub per_request_fresh_suffix_tokens: Vec<usize>,
    pub cache_strategy: String,
    pub estimated_repeated_tokens_avoided: usize,
}

fn default_provider() -> String {
    "generic".to_string()
}

fn default_runtime() -> String {
    "generic".to_string()
}

fn default_budget() -> usize {
    1500
}

fn default_scope() -> String {
    "repo".to_string()
}

fn default_memory_type() -> String {
    "fact".to_string()
}

fn estimate_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    if chars == 0 {
        0
    } else {
        chars.div_ceil(4)
    }
}

fn compact_context(options: &ContextControlOptions) -> (String, Vec<String>) {
    let mut seen = std::collections::BTreeSet::new();
    let mut omitted = Vec::new();
    let mut kept = Vec::new();
    for item in &options.context {
        let normalized = item
            .to_ascii_lowercase()
            .split_whitespace()
            .take(24)
            .collect::<Vec<_>>()
            .join(" ");
        if normalized.contains("password")
            || normalized.contains("secret")
            || normalized.contains("token")
        {
            omitted.push("secret-like context omitted".to_string());
            continue;
        }
        if !seen.insert(normalized) {
            omitted.push("duplicate context omitted".to_string());
            continue;
        }
        kept.push(item.trim().to_string());
    }
    let mut body = kept.join("\n- ");
    if !body.is_empty() {
        body.insert_str(0, "- ");
    }
    (body, omitted)
}

fn build_report(options: ContextControlOptions) -> ContextControlReport {
    let (memory_pack, omitted) = compact_context(&options);
    let stable_prefix = format!(
        "memory.cpp stable prefix\nProvider: {}\nTask: {}\nRules:\n- Local-first.\n- Do not include secrets.\n- Prefer durable memories over raw logs.",
        options.provider, options.task
    );
    let fresh_suffix = format!("Fresh task request:\n{}", options.task);
    let context_pack = if memory_pack.is_empty() {
        "No local context supplied.".to_string()
    } else {
        memory_pack.clone()
    };
    let compiled_prompt =
        format!("{stable_prefix}\n\nContext pack:\n{context_pack}\n\n{fresh_suffix}");
    let raw_context_tokens = options
        .context
        .iter()
        .map(|item| estimate_tokens(item))
        .sum::<usize>()
        .saturating_add(estimate_tokens(&options.task));
    let compiled_context_tokens = estimate_tokens(&compiled_prompt).min(options.budget);
    let duplicate_tokens = omitted
        .iter()
        .filter(|item| item.contains("duplicate"))
        .count()
        * 24;
    let secret_like = omitted
        .iter()
        .filter(|item| item.contains("secret-like"))
        .count();
    let avoided = raw_context_tokens.saturating_sub(compiled_context_tokens);
    let reduction = if raw_context_tokens == 0 {
        0.0
    } else {
        (avoided as f32 / raw_context_tokens as f32) * 100.0
    };
    ContextControlReport {
        compiled_prompt,
        stable_prefix: stable_prefix.clone(),
        fresh_suffix: fresh_suffix.clone(),
        context_pack,
        cache_plan: planProviderCache(options.clone()).stable_prefix_hash,
        kv_report: KvPressureReport {
            raw_context_tokens,
            compiled_context_tokens,
            estimated_kv_positions_avoided: avoided,
        },
        prefill_report: PrefillReport {
            raw_prompt_tokens: raw_context_tokens,
            compiled_prompt_tokens: compiled_context_tokens,
            cacheable_prefix_tokens: estimate_tokens(&stable_prefix),
            fresh_suffix_tokens: estimate_tokens(&fresh_suffix),
            estimated_prefill_reduction_percent: reduction,
        },
        signal_density: SignalDensityReport {
            useful_context_tokens: compiled_context_tokens,
            duplicate_tokens,
            stale_tokens: 0,
            low_relevance_tokens: avoided.saturating_sub(duplicate_tokens),
            signal_density_score: if compiled_context_tokens == 0 {
                0.0
            } else {
                raw_context_tokens.max(1) as f32 / compiled_context_tokens as f32
            },
        },
        token_firewall_report: TokenFirewallReport {
            duplicate_context_tokens_blocked: duplicate_tokens,
            stale_context_tokens_blocked: 0,
            tool_trace_tokens_compressed: 0,
            secret_like_strings_blocked: secret_like,
            prompt_injection_warnings: 0,
        },
        warnings: vec![
            "Estimates are approximate unless connected to runtime/provider metrics.".to_string(),
            "memory.cpp reduces KV pressure by reducing prompt tokens before inference."
                .to_string(),
        ],
        evidence: Vec::new(),
        omitted,
        files_written: Vec::new(),
    }
}

pub fn compileContext(options: ContextControlOptions) -> ContextControlReport {
    build_report(options)
}

pub fn createContextPack(options: ContextControlOptions) -> ContextControlReport {
    build_report(options)
}

pub fn doctor(options: ContextControlOptions) -> ContextControlReport {
    build_report(options)
}

pub fn estimatePrefill(options: ContextControlOptions) -> PrefillReport {
    build_report(options).prefill_report
}

pub fn estimateKvPressure(options: ContextControlOptions) -> KvPressureReport {
    build_report(options).kv_report
}

pub fn calculateSignalDensity(options: ContextControlOptions) -> SignalDensityReport {
    build_report(options).signal_density
}

pub fn planProviderCache(options: ContextControlOptions) -> CacheAuditReport {
    let prefix = format!("{}:{}", options.provider, options.task);
    CacheAuditReport {
        provider: options.provider,
        cache_hit_risk: "low when stable prefix is byte-for-byte reused".to_string(),
        problems: Vec::new(),
        fixes: vec!["put dynamic text after stable memory blocks".to_string()],
        stable_prefix_hash: format!("{:016x}", stable_hash(prefix.as_bytes())),
    }
}

pub fn auditProviderCache(options: ContextControlOptions) -> CacheAuditReport {
    let mut report = planProviderCache(options.clone());
    let has_timestamp = options.context.iter().any(|item| item.contains("202"));
    if has_timestamp {
        report.cache_hit_risk = "medium".to_string();
        report
            .problems
            .push("timestamp-like text appears in cacheable context".to_string());
        report
            .fixes
            .push("move timestamps into the fresh suffix".to_string());
    }
    report
}

pub fn compressToolTrace(options: ContextControlOptions) -> ContextControlReport {
    build_report(options)
}

pub fn rollupTrace(options: ContextControlOptions) -> ContextControlReport {
    build_report(options)
}

pub fn recordMemory(options: MemoryRecordOptions) -> MemoryRecordOptions {
    options
}

pub fn recordMistake(options: MemoryRecordOptions) -> MemoryRecordOptions {
    options
}

pub fn attachProvider(options: ContextControlOptions) -> ContextControlReport {
    build_report(options)
}

pub fn generateRuntimePlan(options: RuntimePlanOptions) -> RuntimePlanReport {
    let budget = match options.runtime.as_str() {
        "llama.cpp" | "ollama" => 4096,
        "vllm" | "sglang" => 8192,
        _ => 4096,
    };
    RuntimePlanReport {
        runtime: options.runtime,
        recommended_context_budget: budget,
        prefix_reuse_hint: "keep stable memory before the fresh suffix".to_string(),
        kv_quantization_hint: "enable in your runtime separately if supported".to_string(),
        speculative_decoding_hint: "shorter compiled prompts reduce prompt-side noise".to_string(),
        batching_hint: "group requests that share the same stable prefix".to_string(),
        dynamic_suffix_placement: "place latest user input and tool output last".to_string(),
        warning: "memory.cpp does not implement low-level kernels by default".to_string(),
    }
}

pub fn generateBatchPlan(options: BatchPlanOptions) -> BatchPlanReport {
    let prefix_tokens = options
        .requests
        .first()
        .map(|request| estimate_tokens(request).min(256))
        .unwrap_or(0);
    let suffixes = options
        .requests
        .iter()
        .map(|request| estimate_tokens(request).saturating_sub(prefix_tokens))
        .collect::<Vec<_>>();
    let avoided = prefix_tokens.saturating_mul(options.requests.len().saturating_sub(1));
    BatchPlanReport {
        provider: options.provider,
        batch_groups: vec![BatchGroup {
            group_id: "shared-prefix-1".to_string(),
            shared_stable_prefix_token_count: prefix_tokens,
            per_request_fresh_suffix_tokens: suffixes,
            cache_strategy: "reuse stable prefix, vary fresh suffix".to_string(),
            estimated_repeated_tokens_avoided: avoided,
        }],
    }
}

pub fn askMemory(options: ContextControlOptions) -> ContextControlReport {
    build_report(options)
}

pub fn testMemory(options: ContextControlOptions) -> ContextControlReport {
    build_report(options)
}

pub fn scoreAgentReadiness(options: ContextControlOptions) -> ContextControlReport {
    build_report(options)
}

fn stable_hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_reduces_duplicate_context() {
        let options = ContextControlOptions {
            task: "fix billing export".to_string(),
            provider: "generic".to_string(),
            budget: 1500,
            context: vec![
                "Use cargo test before pushing. ".repeat(200),
                "Use cargo test before pushing. ".repeat(200),
                "Never include secret tokens".to_string(),
            ],
        };
        let report = compileContext(options);
        assert!(report.kv_report.estimated_kv_positions_avoided > 0);
        assert!(
            report
                .token_firewall_report
                .duplicate_context_tokens_blocked
                > 0
        );
        assert!(report.token_firewall_report.secret_like_strings_blocked > 0);
    }
}
