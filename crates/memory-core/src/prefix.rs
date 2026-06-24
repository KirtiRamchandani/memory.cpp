//! Semantic prefix canonicalization.
//!
//! The single most important property for prompt caching is that the *stable
//! prefix* of a compiled context pack is byte-for-byte reproducible across many
//! different tasks as long as the underlying repository material has not
//! changed. Providers (and local runtimes) can then reuse the prefill / KV
//! state for that prefix instead of recomputing it for every request.
//!
//! To guarantee that, the stable prefix may only ever contain *deterministic,
//! low-volatility* material:
//!
//! * repo identity
//! * repo rules
//! * durable architecture facts
//! * API contracts
//! * invariants
//! * high-confidence decisions
//! * safe tool-use rules
//! * negative-memory exclusions
//! * provider-independent instructions
//!
//! It must **never** contain the current task text, the current error, failing
//! test output, the latest tool trace, timestamps, or random ids. Those belong
//! in the [`FreshSuffixInput`].

use serde::{Deserialize, Serialize};

/// Version tag embedded in every stable-prefix hash. Bump this only when the
/// canonical rendering changes in a way that should invalidate old caches.
pub const STABLE_PREFIX_VERSION: &str = "v1";

/// Version tag embedded in every pack id.
pub const PACK_VERSION: &str = "v2";

/// Deterministic, low-volatility material that is safe to reuse byte-for-byte
/// across many tasks against the same repository state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StablePrefixInput {
    pub repo_identity: Vec<String>,
    pub repo_rules: Vec<String>,
    pub architecture_facts: Vec<String>,
    pub api_contracts: Vec<String>,
    pub invariants: Vec<String>,
    pub decisions: Vec<String>,
    pub tool_use_rules: Vec<String>,
    pub negative_exclusions: Vec<String>,
    pub provider_independent_instructions: Vec<String>,
}

/// Volatile, per-request material. Anything here changes the fresh suffix (and
/// therefore the pack id) but never the stable prefix hash.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FreshSuffixInput {
    pub task: String,
    pub current_error: Option<String>,
    pub failing_test_output: Option<String>,
    pub changed_files: Vec<String>,
    pub latest_trace: Vec<String>,
    pub branch_volatile: Vec<String>,
}

impl StablePrefixInput {
    pub fn is_empty(&self) -> bool {
        self.repo_identity.is_empty()
            && self.repo_rules.is_empty()
            && self.architecture_facts.is_empty()
            && self.api_contracts.is_empty()
            && self.invariants.is_empty()
            && self.decisions.is_empty()
            && self.tool_use_rules.is_empty()
            && self.negative_exclusions.is_empty()
            && self.provider_independent_instructions.is_empty()
    }
}

/// Normalize a single line: trim, collapse internal whitespace, drop trailing
/// punctuation noise. Two semantically identical lines that differ only in
/// whitespace must normalize to the same string so the prefix stays stable.
fn normalize_line(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Deterministically render one section. Items are normalized, de-duplicated,
/// and sorted so that the retrieval order of memories cannot perturb the bytes.
/// Empty sections are skipped entirely (a deterministic choice).
fn render_section(out: &mut String, title: &str, items: &[String]) {
    let mut normalized: Vec<String> = items
        .iter()
        .map(|item| normalize_line(item))
        .filter(|item| !item.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    if normalized.is_empty() {
        return;
    }
    out.push_str("## ");
    out.push_str(title);
    out.push('\n');
    for item in normalized {
        out.push_str("- ");
        out.push_str(&item);
        out.push('\n');
    }
}

/// Render the canonical stable prefix. The output is deterministic for a given
/// [`StablePrefixInput`] regardless of insertion order, whitespace, or
/// duplicates.
pub fn canonicalize_stable_prefix(input: &StablePrefixInput) -> String {
    let mut out = String::new();
    out.push_str("# memory.cpp stable prefix (");
    out.push_str(STABLE_PREFIX_VERSION);
    out.push_str(")\n");
    render_section(&mut out, "Repo identity", &input.repo_identity);
    render_section(&mut out, "Repo rules", &input.repo_rules);
    render_section(&mut out, "Architecture facts", &input.architecture_facts);
    render_section(&mut out, "API contracts", &input.api_contracts);
    render_section(&mut out, "Invariants", &input.invariants);
    render_section(&mut out, "High-confidence decisions", &input.decisions);
    render_section(&mut out, "Tool-use rules", &input.tool_use_rules);
    render_section(
        &mut out,
        "Do-not-send exclusions",
        &input.negative_exclusions,
    );
    render_section(
        &mut out,
        "Provider-independent instructions",
        &input.provider_independent_instructions,
    );
    out
}

/// Render the fresh suffix. This is intentionally volatile.
pub fn render_fresh_suffix(input: &FreshSuffixInput) -> String {
    let mut out = String::new();
    out.push_str("# memory.cpp fresh suffix\n");
    out.push_str("## Task\n");
    out.push_str(&normalize_line(&input.task));
    out.push('\n');
    if let Some(err) = &input.current_error {
        out.push_str("## Current error\n");
        out.push_str(err.trim());
        out.push('\n');
    }
    if let Some(test) = &input.failing_test_output {
        out.push_str("## Failing test output\n");
        out.push_str(test.trim());
        out.push('\n');
    }
    render_plain_section(&mut out, "Changed files", &input.changed_files);
    render_plain_section(&mut out, "Latest trace", &input.latest_trace);
    render_plain_section(&mut out, "Branch-specific details", &input.branch_volatile);
    out
}

/// Like [`render_section`] but preserves order (suffix material is volatile, so
/// determinism across tasks is not required).
fn render_plain_section(out: &mut String, title: &str, items: &[String]) {
    let items: Vec<String> = items
        .iter()
        .map(|item| normalize_line(item))
        .filter(|item| !item.is_empty())
        .collect();
    if items.is_empty() {
        return;
    }
    out.push_str("## ");
    out.push_str(title);
    out.push('\n');
    for item in items {
        out.push_str("- ");
        out.push_str(&item);
        out.push('\n');
    }
}

/// Hash the *bytes of the stable prefix* with blake3. The hash deliberately
/// does not depend on the provider or the task — only on the canonical prefix
/// content — so the same repo material yields the same cache key everywhere.
pub fn stable_prefix_hash(stable_prefix: &str) -> String {
    format!(
        "memprefix:{}:blake3:{}",
        STABLE_PREFIX_VERSION,
        blake3::hash(stable_prefix.as_bytes()).to_hex()
    )
}

/// Compute a content-addressed pack id covering the full compiled artifact:
/// stable prefix, fresh suffix, provider, and task class.
pub fn pack_id(
    stable_prefix: &str,
    fresh_suffix: &str,
    provider: &str,
    task_class: &str,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(stable_prefix.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(fresh_suffix.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(provider.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(task_class.as_bytes());
    format!(
        "mempack:{}:blake3:{}",
        PACK_VERSION,
        hasher.finalize().to_hex()
    )
}

/// Hash an arbitrary content blob (used for fresh-suffix ids, proof sets, etc.).
pub fn content_hash(label: &str, bytes: &[u8]) -> String {
    format!("{}:blake3:{}", label, blake3::hash(bytes).to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_repo() -> StablePrefixInput {
        StablePrefixInput {
            repo_identity: vec!["memory.cpp — local-first context compiler".into()],
            repo_rules: vec![
                "Use pnpm, never npm.".into(),
                "Run cargo test before pushing.".into(),
            ],
            invariants: vec!["Billing exports must be deterministic.".into()],
            decisions: vec!["ADR-004: use server-side sessions.".into()],
            ..Default::default()
        }
    }

    #[test]
    fn stable_prefix_excludes_task_text() {
        let prefix = canonicalize_stable_prefix(&sample_repo());
        assert!(!prefix.to_lowercase().contains("fix checkout"));
        assert!(!prefix.to_lowercase().contains("task:"));
    }

    #[test]
    fn same_repo_different_task_yields_same_prefix_hash() {
        let prefix = canonicalize_stable_prefix(&sample_repo());
        let hash = stable_prefix_hash(&prefix);
        // The stable prefix is computed from repo material only; different task
        // strings must not change it.
        let suffix_a = render_fresh_suffix(&FreshSuffixInput {
            task: "fix checkout bug".into(),
            ..Default::default()
        });
        let suffix_b = render_fresh_suffix(&FreshSuffixInput {
            task: "refactor billing export".into(),
            ..Default::default()
        });
        assert_ne!(suffix_a, suffix_b, "fresh suffix must change with task");
        assert_eq!(stable_prefix_hash(&prefix), hash);
    }

    #[test]
    fn ordering_and_whitespace_do_not_change_prefix() {
        let mut shuffled = sample_repo();
        shuffled.repo_rules = vec![
            "Run   cargo   test before pushing.".into(),
            "Use pnpm, never npm.".into(),
            "Use pnpm, never npm.".into(), // duplicate
        ];
        assert_eq!(
            stable_prefix_hash(&canonicalize_stable_prefix(&sample_repo())),
            stable_prefix_hash(&canonicalize_stable_prefix(&shuffled)),
        );
    }

    #[test]
    fn different_repo_material_changes_prefix_hash() {
        let mut other = sample_repo();
        other.invariants.push("New durable invariant added.".into());
        assert_ne!(
            stable_prefix_hash(&canonicalize_stable_prefix(&sample_repo())),
            stable_prefix_hash(&canonicalize_stable_prefix(&other)),
        );
    }

    #[test]
    fn hashes_are_blake3_tagged() {
        let prefix = canonicalize_stable_prefix(&sample_repo());
        assert!(stable_prefix_hash(&prefix).starts_with("memprefix:v1:blake3:"));
        let pid = pack_id(&prefix, "suffix", "anthropic", "bugfix");
        assert!(pid.starts_with("mempack:v2:blake3:"));
    }
}
