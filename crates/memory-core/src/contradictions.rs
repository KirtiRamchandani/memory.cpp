//! Contradiction detection and resolution.
//!
//! Two memories contradict when they make opposing claims about the same topic
//! ("use npm" vs "use pnpm", "JWT" vs "server-side sessions"). Sending both into
//! a prompt is worse than sending neither: it makes the agent guess. This module
//! detects contradictory pairs and picks a winner using a clear policy
//! (human-confirmed > test-backed > newer > source-backed).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A memory as seen by the contradiction detector.
#[derive(Debug, Clone)]
pub struct MemoryView {
    pub id: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub human_confirmed: bool,
    pub has_source: bool,
    pub test_backed: bool,
}

/// Resolution policy when a contradiction is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergePolicy {
    PreferNewer,
    PreferHumanConfirmed,
}

/// A detected contradiction between two memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    pub topic: String,
    pub a_id: String,
    pub b_id: String,
    pub a_text: String,
    pub b_text: String,
    /// The memory that should win under the policy.
    pub winner_id: String,
    /// The memory that should be marked superseded.
    pub loser_id: String,
    pub reason: String,
}

impl Contradiction {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Mutually-exclusive term pairs that signal a contradiction when they appear
/// in two memories that otherwise discuss the same topic.
const CONFLICT_PAIRS: &[(&str, &str, &str)] = &[
    ("npm", "pnpm", "package manager"),
    ("yarn", "pnpm", "package manager"),
    ("jwt", "server-side session", "auth strategy"),
    ("jwt", "session cookie", "auth strategy"),
    ("json export", "csv export", "export format"),
    ("rest", "graphql", "api style"),
    ("tabs", "spaces", "indentation"),
    ("sql", "nosql", "datastore"),
];

/// Which of two mutually-exclusive terms a memory actually *prefers*. When a
/// memory mentions both (e.g. "use pnpm, never npm" or "migrated from npm to
/// pnpm") the negation/replacement words decide; if it mentions only one, that
/// is the preference; otherwise `None`.
fn preferred<'a>(text: &str, t1: &'a str, t2: &'a str) -> Option<&'a str> {
    let lower = text.to_ascii_lowercase();
    let has1 = lower.contains(t1);
    let has2 = lower.contains(t2);
    if has1 && has2 {
        for neg in [
            "never ",
            "not ",
            "no ",
            "avoid ",
            "from ",
            "instead of ",
            "drop ",
            "stop using ",
            "don't use ",
        ] {
            if lower.contains(&format!("{neg}{t1}")) {
                return Some(t2);
            }
            if lower.contains(&format!("{neg}{t2}")) {
                return Some(t1);
            }
        }
        None // mentions both, no clear preference
    } else if has1 {
        Some(t1)
    } else if has2 {
        Some(t2)
    } else {
        None
    }
}

/// Pick the winner of a contradiction under a policy.
fn choose<'a>(
    a: &'a MemoryView,
    b: &'a MemoryView,
    policy: MergePolicy,
) -> (&'a MemoryView, &'a MemoryView, String) {
    // Human confirmation and test backing always dominate.
    let score = |m: &MemoryView| -> (u8, i64) {
        let mut tier = 0u8;
        if m.human_confirmed {
            tier += 4;
        }
        if m.test_backed {
            tier += 2;
        }
        if m.has_source {
            tier += 1;
        }
        if policy == MergePolicy::PreferHumanConfirmed && m.human_confirmed {
            tier += 8;
        }
        (tier, m.created_at.timestamp())
    };
    let sa = score(a);
    let sb = score(b);
    let a_wins = match policy {
        MergePolicy::PreferHumanConfirmed => sa >= sb,
        MergePolicy::PreferNewer => {
            // Newer wins unless the other is strictly higher tier.
            if sa.0 != sb.0 {
                sa.0 > sb.0
            } else {
                sa.1 >= sb.1
            }
        }
    };
    if a_wins {
        (a, b, policy_reason(policy, a, b))
    } else {
        (b, a, policy_reason(policy, b, a))
    }
}

fn policy_reason(policy: MergePolicy, winner: &MemoryView, _loser: &MemoryView) -> String {
    let mut why = Vec::new();
    if winner.human_confirmed {
        why.push("human-confirmed");
    }
    if winner.test_backed {
        why.push("test-backed");
    }
    if winner.has_source {
        why.push("source-backed");
    }
    match policy {
        MergePolicy::PreferNewer => why.push("newer"),
        MergePolicy::PreferHumanConfirmed => why.push("preferred via policy"),
    }
    format!("kept {} ({})", winner.id, why.join(", "))
}

/// Detect contradictions across a set of memories.
pub fn detect(memories: &[MemoryView], policy: MergePolicy) -> Vec<Contradiction> {
    let mut out = Vec::new();
    for i in 0..memories.len() {
        for j in (i + 1)..memories.len() {
            let a = &memories[i];
            let b = &memories[j];
            for (t1, t2, topic) in CONFLICT_PAIRS {
                // A contradiction exists only when both memories express a clear
                // and opposite preference for the same topic.
                let pa = preferred(&a.text, t1, t2);
                let pb = preferred(&b.text, t1, t2);
                let conflict = matches!((pa, pb), (Some(x), Some(y)) if x != y);
                if conflict {
                    let (winner, loser, reason) = choose(a, b, policy);
                    out.push(Contradiction {
                        topic: topic.to_string(),
                        a_id: a.id.clone(),
                        b_id: b.id.clone(),
                        a_text: a.text.clone(),
                        b_text: b.text.clone(),
                        winner_id: winner.id.clone(),
                        loser_id: loser.id.clone(),
                        reason,
                    });
                    break; // one topic per pair is enough
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn mv(id: &str, text: &str, age_days: i64, human: bool) -> MemoryView {
        MemoryView {
            id: id.into(),
            text: text.into(),
            created_at: Utc::now() - Duration::days(age_days),
            human_confirmed: human,
            has_source: false,
            test_backed: false,
        }
    }

    #[test]
    fn detects_npm_vs_pnpm() {
        let mems = vec![
            mv("old", "Install dependencies with npm install.", 10, false),
            mv("new", "Use pnpm for installs. Never npm.", 1, false),
        ];
        let c = detect(&mems, MergePolicy::PreferNewer);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].topic, "package manager");
        assert_eq!(c[0].winner_id, "new"); // newer wins
        assert_eq!(c[0].loser_id, "old");
    }

    #[test]
    fn human_confirmed_beats_newer() {
        let mems = vec![
            mv("human", "Auth uses server-side sessions.", 30, true),
            mv("recent", "Auth uses JWT tokens.", 1, false),
        ];
        let c = detect(&mems, MergePolicy::PreferHumanConfirmed);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].winner_id, "human");
    }

    #[test]
    fn no_false_positive_for_single_memory_mentioning_both() {
        let mems = vec![
            mv("a", "We migrated from npm to pnpm.", 1, false),
            mv("b", "Unrelated note about caching.", 1, false),
        ];
        assert!(detect(&mems, MergePolicy::PreferNewer).is_empty());
    }
}
