//! Context lockfile.
//!
//! A [`ContextPackLock`] is a deterministic record of exactly what went into a
//! compiled context pack: compiler version, repo state, the memory ids and
//! their source/proof hashes, the negative-memory version, the stable-prefix
//! and fresh-suffix hashes, and token counts. Committing `.memory.cpp/context.lock`
//! lets a teammate (or CI) reproduce or verify the same pack later.

use serde::{Deserialize, Serialize};

use crate::prefix::content_hash;

/// Token counts captured in the lock.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockTokenCounts {
    pub stable_prefix: usize,
    pub fresh_suffix: usize,
    pub compiled: usize,
}

/// The lockfile contents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextPackLock {
    pub pack_id: String,
    pub compiler_version: String,
    pub repo_state_hash: String,
    pub task_class: String,
    pub provider: String,
    pub memory_ids: Vec<String>,
    pub source_hashes: Vec<String>,
    pub freshness_proof_hashes: Vec<String>,
    pub negative_memory_version: String,
    pub stable_prefix_hash: String,
    pub fresh_suffix_hash: String,
    pub token_counts: LockTokenCounts,
}

impl ContextPackLock {
    pub fn from_json(raw: &str) -> Option<ContextPackLock> {
        serde_json::from_str(raw).ok()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// A single hash over all structural fields, used as a quick lock identity.
    pub fn lock_hash(&self) -> String {
        let canonical = serde_json::to_string(self).unwrap_or_default();
        content_hash("lock", canonical.as_bytes())
    }

    /// Structural equality (the struct derives Eq; this is a readable alias and
    /// the place to relax fields later if we add non-structural metadata).
    pub fn matches(&self, other: &ContextPackLock) -> bool {
        self == other
    }

    /// Human-readable diff against another lock.
    pub fn diff(&self, other: &ContextPackLock) -> Vec<String> {
        let mut diffs = Vec::new();
        if self.stable_prefix_hash != other.stable_prefix_hash {
            diffs.push(format!(
                "stable_prefix_hash: {} -> {}",
                self.stable_prefix_hash, other.stable_prefix_hash
            ));
        }
        if self.fresh_suffix_hash != other.fresh_suffix_hash {
            diffs.push("fresh_suffix_hash changed".to_string());
        }
        if self.repo_state_hash != other.repo_state_hash {
            diffs.push(format!(
                "repo_state_hash: {} -> {}",
                self.repo_state_hash, other.repo_state_hash
            ));
        }
        let added: Vec<&String> = other
            .memory_ids
            .iter()
            .filter(|m| !self.memory_ids.contains(m))
            .collect();
        let removed: Vec<&String> = self
            .memory_ids
            .iter()
            .filter(|m| !other.memory_ids.contains(m))
            .collect();
        for m in added {
            diffs.push(format!("+ memory {m}"));
        }
        for m in removed {
            diffs.push(format!("- memory {m}"));
        }
        if self.token_counts != other.token_counts {
            diffs.push(format!(
                "tokens: compiled {} -> {}",
                self.token_counts.compiled, other.token_counts.compiled
            ));
        }
        diffs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock(prefix_hash: &str, mem: &[&str]) -> ContextPackLock {
        ContextPackLock {
            pack_id: "mempack:v2:blake3:abc".into(),
            compiler_version: "memory.cpp-compiler/0.1.0".into(),
            repo_state_hash: "git:abc".into(),
            task_class: "bugfix".into(),
            provider: "anthropic".into(),
            memory_ids: mem.iter().map(|s| s.to_string()).collect(),
            source_hashes: vec![],
            freshness_proof_hashes: vec![],
            negative_memory_version: "neg:v1".into(),
            stable_prefix_hash: prefix_hash.into(),
            fresh_suffix_hash: "suffix:blake3:s".into(),
            token_counts: LockTokenCounts {
                stable_prefix: 100,
                fresh_suffix: 50,
                compiled: 150,
            },
        }
    }

    #[test]
    fn roundtrips_json() {
        let l = lock("memprefix:v1:blake3:p", &["m1", "m2"]);
        let parsed = ContextPackLock::from_json(&l.to_json()).unwrap();
        assert!(l.matches(&parsed));
        assert_eq!(l.lock_hash(), parsed.lock_hash());
    }

    #[test]
    fn diff_reports_prefix_and_memory_changes() {
        let a = lock("memprefix:v1:blake3:AAA", &["m1", "m2"]);
        let b = lock("memprefix:v1:blake3:BBB", &["m2", "m3"]);
        let d = a.diff(&b);
        assert!(d.iter().any(|s| s.contains("stable_prefix_hash")));
        assert!(d.iter().any(|s| s == "+ memory m3"));
        assert!(d.iter().any(|s| s == "- memory m1"));
    }

    #[test]
    fn identical_locks_have_no_diff() {
        let a = lock("memprefix:v1:blake3:p", &["m1"]);
        let b = lock("memprefix:v1:blake3:p", &["m1"]);
        assert!(a.diff(&b).is_empty());
        assert!(a.matches(&b));
    }
}
