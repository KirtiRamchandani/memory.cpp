//! Invariant extraction.
//!
//! Durable project rules are usually already written down --- in tests, ADRs,
//! README/AGENTS files, code comments, prior bug fixes, and CI config --- just
//! not in a form an agent can consume. This module extracts candidate
//! invariants from those sources, each carrying the evidence that produced it.

use serde::{Deserialize, Serialize};

/// Where an invariant came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Test,
    Adr,
    Readme,
    Agents,
    Comment,
    Ci,
    Fix,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::Test => "test",
            SourceKind::Adr => "adr",
            SourceKind::Readme => "readme",
            SourceKind::Agents => "agents",
            SourceKind::Comment => "comment",
            SourceKind::Ci => "ci",
            SourceKind::Fix => "fix",
        }
    }

    /// Base confidence for invariants from this source.
    fn confidence(self) -> f32 {
        match self {
            SourceKind::Test | SourceKind::Adr => 0.9,
            SourceKind::Ci | SourceKind::Fix => 0.8,
            SourceKind::Agents | SourceKind::Readme => 0.7,
            SourceKind::Comment => 0.6,
        }
    }

    /// Guess the source kind from a file path.
    pub fn from_path(path: &str) -> SourceKind {
        let p = path.to_ascii_lowercase();
        if p.contains("agents.md") || p.contains("gemini.md") {
            SourceKind::Agents
        } else if p.contains("readme") {
            SourceKind::Readme
        } else if p.contains("adr") || p.contains("/decisions/") {
            SourceKind::Adr
        } else if p.contains("test") || p.contains("spec") {
            SourceKind::Test
        } else if p.contains(".github")
            || p.contains("ci")
            || p.contains(".yml")
            || p.contains(".yaml")
        {
            SourceKind::Ci
        } else {
            SourceKind::Comment
        }
    }
}

/// One extracted invariant with its evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invariant {
    pub text: String,
    pub source_kind: String,
    pub evidence: Vec<String>,
    pub confidence: f32,
}

impl Invariant {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Signal phrases that mark a line as a likely invariant.
const SIGNALS: &[&str] = &[
    "must not",
    "must ",
    "never ",
    "always ",
    "do not ",
    "don't ",
    "shall ",
    "required",
    "ensure ",
    "should not",
    "are not allowed",
    "is forbidden",
];

fn looks_like_code(line: &str) -> bool {
    let t = line.trim();
    t.contains("=>")
        || t.contains("::")
        || t.contains("fn ")
        || t.contains("let ")
        || t.contains("\");")
        || t.contains("assert")
        // a bare string literal entry like `"never ",`
        || (t.starts_with('"') && t.ends_with("\","))
        || (t.starts_with('"') && t.ends_with('"'))
}

fn looks_invariant(line: &str) -> bool {
    if looks_like_code(line) {
        return false;
    }
    let l = line.to_ascii_lowercase();
    SIGNALS.iter().any(|s| l.contains(s))
}

/// Clean a raw source line into a one-sentence invariant.
fn normalize(line: &str) -> String {
    let trimmed = line
        .trim()
        .trim_start_matches(['#', '/', '*', '-', '>', '!', ' ', '\t'])
        .trim();
    // keep only the first sentence
    let end = trimmed
        .find(['.', '\n'])
        .map(|i| i + 1)
        .unwrap_or(trimmed.len());
    trimmed[..end].trim().to_string()
}

/// Extract invariants from one source's content.
pub fn extract_from(content: &str, path: &str) -> Vec<Invariant> {
    let kind = SourceKind::from_path(path);
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for line in content.lines() {
        if !looks_invariant(line) {
            continue;
        }
        let text = normalize(line);
        if text.len() < 8 || text.len() > 200 {
            continue;
        }
        let key = text.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        out.push(Invariant {
            text,
            source_kind: kind.as_str().to_string(),
            evidence: vec![path.to_string()],
            confidence: kind.confidence(),
        });
    }
    out
}

/// Extract from many sources, de-duplicating by invariant text.
pub fn extract(sources: &[(String, String)]) -> Vec<Invariant> {
    let mut by_text: std::collections::BTreeMap<String, Invariant> =
        std::collections::BTreeMap::new();
    for (path, content) in sources {
        for inv in extract_from(content, path) {
            by_text
                .entry(inv.text.to_ascii_lowercase())
                .and_modify(|existing| {
                    // Merge evidence and keep the higher confidence.
                    for e in &inv.evidence {
                        if !existing.evidence.contains(e) {
                            existing.evidence.push(e.clone());
                        }
                    }
                    if inv.confidence > existing.confidence {
                        existing.confidence = inv.confidence;
                        existing.source_kind = inv.source_kind.clone();
                    }
                })
                .or_insert(inv);
        }
    }
    by_text.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_must_and_never_lines() {
        let content = "# Rules\n- Billing exports must be deterministic.\n- Never call the database from controllers.\nSome prose without a rule.\n";
        let inv = extract_from(content, "AGENTS.md");
        assert_eq!(inv.len(), 2);
        assert!(inv.iter().any(|i| i.text.contains("deterministic")));
        assert_eq!(inv[0].source_kind, "agents");
    }

    #[test]
    fn source_kind_drives_confidence() {
        let from_test = extract_from("Tokens must be validated before use.", "tests/auth_test.rs");
        let from_comment =
            extract_from("Tokens must be validated before use.", "src/util_notes.md");
        assert!(from_test[0].confidence > from_comment[0].confidence);
    }

    #[test]
    fn merges_evidence_across_sources() {
        let sources = vec![
            (
                "AGENTS.md".to_string(),
                "Exports must be deterministic.".to_string(),
            ),
            (
                "tests/x.rs".to_string(),
                "Exports must be deterministic.".to_string(),
            ),
        ];
        let inv = extract(&sources);
        assert_eq!(inv.len(), 1);
        assert_eq!(inv[0].evidence.len(), 2);
        // higher-confidence source (test) wins
        assert_eq!(inv[0].source_kind, "test");
    }
}
