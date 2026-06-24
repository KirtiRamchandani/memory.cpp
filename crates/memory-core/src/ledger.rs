//! Prompt provenance ledger.
//!
//! Every compiled pack should leave an audit trail: which memories were
//! included and excluded (and why), which freshness proofs passed or failed,
//! the token-budget tradeoffs, the cache regions, the provider, any files
//! written, and the pack hash. The ledger is an append-only JSONL log so runs
//! can be compared and reproduced.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::prefix::content_hash;

/// One excluded memory with the reason it was dropped.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExcludedRecord {
    pub id: String,
    pub reason: String,
}

/// One ledger entry for a single compiled pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub pack_id: String,
    pub recorded_at: DateTime<Utc>,
    pub provider: String,
    pub task_class: String,
    pub repo_state_hash: String,
    pub included_memories: Vec<String>,
    pub excluded_memories: Vec<ExcludedRecord>,
    pub proofs_passed: usize,
    pub proofs_failed: usize,
    pub stable_prefix_hash: String,
    pub fresh_suffix_hash: String,
    pub stable_prefix_tokens: usize,
    pub fresh_suffix_tokens: usize,
    pub compiled_tokens: usize,
    pub budget: usize,
    pub cache_regions: Vec<String>,
    pub generated_files: Vec<String>,
    pub pack_hash: String,
}

impl LedgerEntry {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Serialize to a single JSONL line.
    pub fn to_jsonl(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Parse a JSONL ledger into entries, skipping malformed lines.
    pub fn parse_log(raw: &str) -> Vec<LedgerEntry> {
        raw.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str::<LedgerEntry>(l).ok())
            .collect()
    }
}

/// Content checksums embedded in a pack so it can be verified and reproduced.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackChecksums {
    pub pack_id: String,
    pub stable_prefix_id: String,
    pub fresh_suffix_id: String,
    pub memory_ir_id: String,
    pub negative_memory_id: String,
    pub proof_set_id: String,
}

impl PackChecksums {
    /// Build checksums from the constituent artifacts.
    pub fn build(
        pack_id: &str,
        stable_prefix: &str,
        fresh_suffix: &str,
        memory_ir_json: &str,
        negative_memory_json: &str,
        proof_set_json: &str,
    ) -> Self {
        PackChecksums {
            pack_id: pack_id.to_string(),
            stable_prefix_id: content_hash("prefix", stable_prefix.as_bytes()),
            fresh_suffix_id: content_hash("suffix", fresh_suffix.as_bytes()),
            memory_ir_id: content_hash("ir", memory_ir_json.as_bytes()),
            negative_memory_id: content_hash("neg", negative_memory_json.as_bytes()),
            proof_set_id: content_hash("proofs", proof_set_json.as_bytes()),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Render as a `[checksums]` block for the Context ABI.
    pub fn to_abi_block(&self) -> String {
        format!(
            "[checksums]\npack_id = {}\nstable_prefix_id = {}\nfresh_suffix_id = {}\nmemory_ir_id = {}\nnegative_memory_id = {}\nproof_set_id = {}\n",
            self.pack_id,
            self.stable_prefix_id,
            self.fresh_suffix_id,
            self.memory_ir_id,
            self.negative_memory_id,
            self.proof_set_id,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> LedgerEntry {
        LedgerEntry {
            pack_id: "mempack:v2:blake3:abc".into(),
            recorded_at: Utc::now(),
            provider: "anthropic".into(),
            task_class: "bugfix".into(),
            repo_state_hash: "git:abc".into(),
            included_memories: vec!["m1".into(), "m2".into()],
            excluded_memories: vec![ExcludedRecord {
                id: "m3".into(),
                reason: "stale".into(),
            }],
            proofs_passed: 2,
            proofs_failed: 1,
            stable_prefix_hash: "memprefix:v1:blake3:p".into(),
            fresh_suffix_hash: "suffix:blake3:s".into(),
            stable_prefix_tokens: 100,
            fresh_suffix_tokens: 50,
            compiled_tokens: 150,
            budget: 1500,
            cache_regions: vec!["stable_repo_prefix".into(), "fresh_task_suffix".into()],
            generated_files: vec![],
            pack_hash: "pack:blake3:h".into(),
        }
    }

    #[test]
    fn jsonl_roundtrips() {
        let e = entry();
        let line = e.to_jsonl();
        let parsed = LedgerEntry::parse_log(&format!("{line}\n\ngarbage\n{line}"));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].pack_id, e.pack_id);
        assert_eq!(parsed[0].excluded_memories[0].reason, "stale");
    }

    #[test]
    fn checksums_are_deterministic_and_tagged() {
        let a = PackChecksums::build("p", "prefix", "suffix", "{}", "{}", "[]");
        let b = PackChecksums::build("p", "prefix", "suffix", "{}", "{}", "[]");
        assert_eq!(a, b);
        assert!(a.stable_prefix_id.starts_with("prefix:blake3:"));
        assert!(a.to_abi_block().contains("[checksums]"));
    }

    #[test]
    fn changing_content_changes_checksum() {
        let a = PackChecksums::build("p", "prefix", "suffix", "{}", "{}", "[]");
        let b = PackChecksums::build("p", "prefix CHANGED", "suffix", "{}", "{}", "[]");
        assert_ne!(a.stable_prefix_id, b.stable_prefix_id);
    }
}
