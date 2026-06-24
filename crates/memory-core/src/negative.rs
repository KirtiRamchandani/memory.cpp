//! Negative memory — an explicit memory of what *not* to send.
//!
//! Most retrieval systems only model what to include. Coding agents fail just
//! as often from what gets included by accident: stale docs, superseded
//! decisions, prior wrong fixes, generated files, duplicated context, misleading
//! traces, and prompt-injection sources. Negative memory records those targets
//! so the compiler can exclude them deterministically and explain why.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::prefix::content_hash;

/// Category of negative memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NegativeKind {
    StaleDoc,
    SupersededDecision,
    WrongFix,
    GeneratedFile,
    DuplicateContext,
    MisleadingTrace,
    SecurityRisk,
    PromptInjectionSource,
    LowSignalFile,
    ObsoleteApi,
}

impl NegativeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NegativeKind::StaleDoc => "stale_doc",
            NegativeKind::SupersededDecision => "superseded_decision",
            NegativeKind::WrongFix => "wrong_fix",
            NegativeKind::GeneratedFile => "generated_file",
            NegativeKind::DuplicateContext => "duplicate_context",
            NegativeKind::MisleadingTrace => "misleading_trace",
            NegativeKind::SecurityRisk => "security_risk",
            NegativeKind::PromptInjectionSource => "prompt_injection_source",
            NegativeKind::LowSignalFile => "low_signal_file",
            NegativeKind::ObsoleteApi => "obsolete_api",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(
            match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
                "stale_doc" | "stale" => NegativeKind::StaleDoc,
                "superseded_decision" | "superseded" => NegativeKind::SupersededDecision,
                "wrong_fix" | "wrong" => NegativeKind::WrongFix,
                "generated_file" | "generated" => NegativeKind::GeneratedFile,
                "duplicate_context" | "duplicate" => NegativeKind::DuplicateContext,
                "misleading_trace" | "misleading" => NegativeKind::MisleadingTrace,
                "security_risk" | "security" => NegativeKind::SecurityRisk,
                "prompt_injection_source" | "prompt_injection" | "injection" => {
                    NegativeKind::PromptInjectionSource
                }
                "low_signal_file" | "low_signal" => NegativeKind::LowSignalFile,
                "obsolete_api" | "obsolete" => NegativeKind::ObsoleteApi,
                _ => return None,
            },
        )
    }
}

/// One negative-memory entry. `target` is a file path or a memory id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegativeMemory {
    pub id: String,
    pub target: String,
    pub kind: NegativeKind,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub source: Option<String>,
}

impl NegativeMemory {
    pub fn new(target: &str, kind: NegativeKind, reason: &str, now: DateTime<Utc>) -> Self {
        let hash = content_hash("neg", format!("{target}|{}", kind.as_str()).as_bytes());
        let id = format!("neg:{}", &hash[hash.len() - 16..]);
        NegativeMemory {
            id,
            target: target.to_string(),
            kind,
            reason: reason.to_string(),
            created_at: now,
            source: None,
        }
    }

    /// Does this entry apply to a candidate identified by path and/or memory id?
    pub fn matches(&self, candidate_path: Option<&str>, candidate_id: Option<&str>) -> bool {
        let t = self.target.trim_start_matches("./");
        if let Some(id) = candidate_id {
            if id == self.target {
                return true;
            }
        }
        if let Some(path) = candidate_path {
            let p = path.trim_start_matches("./");
            if p == t || p.ends_with(t) || t.ends_with(p) {
                return true;
            }
        }
        false
    }
}

/// A persistable collection of negative memories.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NegativeMemoryStore {
    #[serde(default)]
    pub entries: Vec<NegativeMemory>,
}

impl NegativeMemoryStore {
    pub fn from_json(raw: &str) -> Self {
        serde_json::from_str(raw).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{\"entries\":[]}".to_string())
    }

    /// Add an entry, replacing any existing entry with the same target+kind.
    pub fn add(&mut self, entry: NegativeMemory) {
        self.entries
            .retain(|e| !(e.target == entry.target && e.kind == entry.kind));
        self.entries.push(entry);
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() != before
    }

    /// Find the first entry matching a candidate.
    pub fn matching(
        &self,
        candidate_path: Option<&str>,
        candidate_id: Option<&str>,
    ) -> Option<&NegativeMemory> {
        self.entries
            .iter()
            .find(|e| e.matches(candidate_path, candidate_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kinds_with_aliases() {
        assert_eq!(NegativeKind::parse("stale"), Some(NegativeKind::StaleDoc));
        assert_eq!(
            NegativeKind::parse("prompt-injection"),
            Some(NegativeKind::PromptInjectionSource)
        );
        assert_eq!(NegativeKind::parse("nonsense"), None);
    }

    #[test]
    fn matches_by_path_suffix_and_id() {
        let n = NegativeMemory::new(
            "docs/export-v1.md",
            NegativeKind::StaleDoc,
            "old",
            Utc::now(),
        );
        assert!(n.matches(Some("docs/export-v1.md"), None));
        assert!(n.matches(Some("./docs/export-v1.md"), None));
        assert!(!n.matches(Some("docs/export-v2.md"), None));

        let by_id = NegativeMemory::new("mem-123", NegativeKind::WrongFix, "bad", Utc::now());
        assert!(by_id.matches(None, Some("mem-123")));
    }

    #[test]
    fn store_add_dedups_and_roundtrips_json() {
        let mut store = NegativeMemoryStore::default();
        store.add(NegativeMemory::new(
            "a.md",
            NegativeKind::StaleDoc,
            "r1",
            Utc::now(),
        ));
        store.add(NegativeMemory::new(
            "a.md",
            NegativeKind::StaleDoc,
            "r2",
            Utc::now(),
        ));
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].reason, "r2");

        let json = store.to_json();
        let reloaded = NegativeMemoryStore::from_json(&json);
        assert_eq!(reloaded.entries.len(), 1);
    }

    #[test]
    fn store_remove_by_id() {
        let mut store = NegativeMemoryStore::default();
        let entry = NegativeMemory::new("b.md", NegativeKind::GeneratedFile, "gen", Utc::now());
        let id = entry.id.clone();
        store.add(entry);
        assert!(store.remove(&id));
        assert!(store.entries.is_empty());
    }
}
