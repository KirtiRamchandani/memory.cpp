//! Context garbage collection.
//!
//! A mark/sweep pass over the memory vault that keeps the live, high-signal set
//! small and safe: stale memory is archived, duplicates are compacted,
//! dangerous memory is quarantined, and invariants are pinned. Like a GC, it
//! reports what it did and can run dry.

use serde::{Deserialize, Serialize};

/// A memory as seen by the collector.
#[derive(Debug, Clone)]
pub struct GcInput {
    pub id: String,
    pub tokens: usize,
    pub is_stale: bool,
    pub is_duplicate: bool,
    pub is_dangerous: bool,
    pub is_invariant: bool,
    pub is_low_signal: bool,
}

/// What the collector decided to do with a memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GcAction {
    Keep,
    Pin,
    Archive,
    Compact,
    Quarantine,
    Sweep,
}

impl GcAction {
    pub fn as_str(self) -> &'static str {
        match self {
            GcAction::Keep => "keep",
            GcAction::Pin => "pin",
            GcAction::Archive => "archive",
            GcAction::Compact => "compact",
            GcAction::Quarantine => "quarantine",
            GcAction::Sweep => "sweep",
        }
    }
    /// Whether this action removes the memory from the live set.
    pub fn frees(self) -> bool {
        matches!(
            self,
            GcAction::Archive | GcAction::Compact | GcAction::Quarantine | GcAction::Sweep
        )
    }
}

/// One decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcDecision {
    pub id: String,
    pub action: String,
    pub reason: String,
    pub tokens: usize,
}

/// The full GC plan/report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcReport {
    pub active_tokens_before: usize,
    pub active_tokens_after: usize,
    pub pinned: usize,
    pub archived: usize,
    pub compacted: usize,
    pub quarantined: usize,
    pub swept: usize,
    pub decisions: Vec<GcDecision>,
}

impl GcReport {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    pub fn prefix_reduction_percent(&self) -> f32 {
        if self.active_tokens_before == 0 {
            0.0
        } else {
            ((self
                .active_tokens_before
                .saturating_sub(self.active_tokens_after)) as f32
                / self.active_tokens_before as f32)
                * 100.0
        }
    }
}

fn decide(m: &GcInput, aggressive: bool) -> (GcAction, &'static str) {
    if m.is_dangerous {
        (GcAction::Quarantine, "dangerous (secret/injection)")
    } else if m.is_stale {
        (GcAction::Archive, "stale/superseded")
    } else if m.is_duplicate {
        (GcAction::Compact, "duplicate summary")
    } else if m.is_invariant {
        (GcAction::Pin, "durable invariant")
    } else if aggressive && m.is_low_signal {
        (GcAction::Sweep, "low signal (aggressive)")
    } else {
        (GcAction::Keep, "live")
    }
}

/// Plan a GC pass. `aggressive` also sweeps low-signal memory.
pub fn plan(memories: &[GcInput], aggressive: bool) -> GcReport {
    let before: usize = memories.iter().map(|m| m.tokens).sum();
    let mut report = GcReport {
        active_tokens_before: before,
        active_tokens_after: 0,
        pinned: 0,
        archived: 0,
        compacted: 0,
        quarantined: 0,
        swept: 0,
        decisions: Vec::new(),
    };
    let mut after = 0usize;
    for m in memories {
        let (action, reason) = decide(m, aggressive);
        match action {
            GcAction::Pin => report.pinned += 1,
            GcAction::Archive => report.archived += 1,
            GcAction::Compact => report.compacted += 1,
            GcAction::Quarantine => report.quarantined += 1,
            GcAction::Sweep => report.swept += 1,
            GcAction::Keep => {}
        }
        if !action.frees() {
            after += m.tokens;
        }
        report.decisions.push(GcDecision {
            id: m.id.clone(),
            action: action.as_str().to_string(),
            reason: reason.to_string(),
            tokens: m.tokens,
        });
    }
    report.active_tokens_after = after;
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, tokens: usize) -> GcInput {
        GcInput {
            id: id.into(),
            tokens,
            is_stale: false,
            is_duplicate: false,
            is_dangerous: false,
            is_invariant: false,
            is_low_signal: false,
        }
    }

    #[test]
    fn archives_stale_and_pins_invariants() {
        let mems = vec![
            GcInput {
                is_stale: true,
                ..item("stale", 100)
            },
            GcInput {
                is_invariant: true,
                ..item("inv", 20)
            },
            item("live", 30),
        ];
        let r = plan(&mems, false);
        assert_eq!(r.archived, 1);
        assert_eq!(r.pinned, 1);
        assert_eq!(r.active_tokens_after, 50); // stale freed
        assert!(r.prefix_reduction_percent() > 60.0);
    }

    #[test]
    fn quarantines_dangerous_first() {
        let mems = vec![GcInput {
            is_dangerous: true,
            is_stale: true,
            ..item("x", 40)
        }];
        let r = plan(&mems, false);
        assert_eq!(r.quarantined, 1);
        assert_eq!(r.archived, 0);
    }

    #[test]
    fn aggressive_sweeps_low_signal() {
        let mems = vec![GcInput {
            is_low_signal: true,
            ..item("noise", 80)
        }];
        assert_eq!(plan(&mems, false).swept, 0);
        assert_eq!(plan(&mems, true).swept, 1);
    }
}
