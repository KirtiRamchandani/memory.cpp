//! Freshness proofs.
//!
//! A memory that claims to describe the repository should be able to *prove*
//! that it is still valid before it is allowed into a compiled context pack.
//! A [`FreshnessProof`] runs a battery of [`ProofCheck`]s — source anchor still
//! exists, content hash matches, commit reachable, not expired, no newer
//! contradiction, source reliable enough, human confirmed — and produces a
//! JSON-serializable, human-readable verdict.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::prefix::content_hash;

/// Outcome of a single check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofStatus {
    Pass,
    Fail,
    /// Not applicable (e.g. no source file recorded) — does not fail the proof.
    Skipped,
    /// Could not be determined (e.g. git not available).
    Unknown,
}

impl ProofStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ProofStatus::Pass => "pass",
            ProofStatus::Fail => "fail",
            ProofStatus::Skipped => "skipped",
            ProofStatus::Unknown => "unknown",
        }
    }
}

/// A single named freshness check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofCheck {
    pub name: String,
    pub status: ProofStatus,
    pub detail: String,
}

impl ProofCheck {
    fn new(name: &str, status: ProofStatus, detail: impl Into<String>) -> Self {
        ProofCheck {
            name: name.to_string(),
            status,
            detail: detail.into(),
        }
    }
}

/// Final verdict for a memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessVerdict {
    Fresh,
    Stale,
    Unverifiable,
}

impl FreshnessVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            FreshnessVerdict::Fresh => "fresh",
            FreshnessVerdict::Stale => "stale",
            FreshnessVerdict::Unverifiable => "unverifiable",
        }
    }
}

/// The full proof for one memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessProof {
    pub memory_id: String,
    pub verdict: FreshnessVerdict,
    /// passed / applicable checks, in 0.0..=1.0.
    pub score: f32,
    pub checks: Vec<ProofCheck>,
    /// Stable hash of the proof set, usable as a checksum in a lockfile.
    pub proof_hash: String,
}

impl FreshnessProof {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    pub fn is_fresh(&self) -> bool {
        self.verdict == FreshnessVerdict::Fresh
    }
}

/// Inputs required to prove a single memory. The caller resolves source paths
/// against the repo root and (optionally) precomputes commit reachability so
/// this module stays free of process/git coupling.
pub struct FreshnessInput<'a> {
    pub memory_id: &'a str,
    /// Source file path, already resolved to an absolute/working path.
    pub source_file: Option<&'a Path>,
    pub source_line: Option<u64>,
    pub source_commit: Option<&'a str>,
    /// Hash of the source span recorded when the memory was created, if any.
    pub stored_span_hash: Option<&'a str>,
    pub expires_at: Option<DateTime<Utc>>,
    pub human_confirmed: bool,
    pub reliability: f32,
    /// True when a newer memory contradicts this one.
    pub newer_contradiction: bool,
    /// Caller-supplied git reachability of `source_commit` (None = unknown).
    pub commit_reachable: Option<bool>,
}

const RELIABILITY_THRESHOLD: f32 = 0.4;

/// Run all freshness checks for a memory and produce a proof.
pub fn prove(input: &FreshnessInput<'_>, now: DateTime<Utc>) -> FreshnessProof {
    let mut checks = Vec::new();

    // 1. Source file still exists.
    let file_exists = match input.source_file {
        None => {
            checks.push(ProofCheck::new(
                "source_file_exists",
                ProofStatus::Skipped,
                "no source file recorded",
            ));
            None
        }
        Some(path) => {
            let exists = path.is_file();
            checks.push(ProofCheck::new(
                "source_file_exists",
                if exists {
                    ProofStatus::Pass
                } else {
                    ProofStatus::Fail
                },
                format!("{}", path.display()),
            ));
            Some(exists)
        }
    };

    // 2. Source line still exists (or can be relocated within the file).
    match (input.source_file, input.source_line, file_exists) {
        (Some(path), Some(line), Some(true)) => {
            let line_count = std::fs::read_to_string(path)
                .map(|c| c.lines().count() as u64)
                .unwrap_or(0);
            let status = if line == 0 || line <= line_count {
                ProofStatus::Pass
            } else {
                ProofStatus::Fail
            };
            checks.push(ProofCheck::new(
                "source_line_exists",
                status,
                format!("line {line} of {line_count}"),
            ));
        }
        _ => checks.push(ProofCheck::new(
            "source_line_exists",
            ProofStatus::Skipped,
            "no anchored line",
        )),
    }

    // 3. Source span hash matches.
    match (input.stored_span_hash, input.source_file, file_exists) {
        (Some(stored), Some(path), Some(true)) => {
            let current = std::fs::read(path)
                .map(|bytes| content_hash("span", &bytes))
                .unwrap_or_default();
            let matches = !current.is_empty() && current == stored;
            checks.push(ProofCheck::new(
                "source_hash_matches",
                if matches {
                    ProofStatus::Pass
                } else {
                    ProofStatus::Fail
                },
                if matches {
                    "content hash unchanged".to_string()
                } else {
                    "source content changed since memory was recorded".to_string()
                },
            ));
        }
        _ => checks.push(ProofCheck::new(
            "source_hash_matches",
            ProofStatus::Skipped,
            "no recorded span hash",
        )),
    }

    // 4. Source commit reachable.
    match (input.source_commit, input.commit_reachable) {
        (Some(commit), Some(reachable)) => checks.push(ProofCheck::new(
            "source_commit_reachable",
            if reachable {
                ProofStatus::Pass
            } else {
                ProofStatus::Fail
            },
            commit.to_string(),
        )),
        (Some(commit), None) => checks.push(ProofCheck::new(
            "source_commit_reachable",
            ProofStatus::Unknown,
            format!("could not check {commit}"),
        )),
        (None, _) => checks.push(ProofCheck::new(
            "source_commit_reachable",
            ProofStatus::Skipped,
            "no source commit",
        )),
    }

    // 5. Not expired.
    match input.expires_at {
        Some(exp) => checks.push(ProofCheck::new(
            "not_expired",
            if exp > now {
                ProofStatus::Pass
            } else {
                ProofStatus::Fail
            },
            format!("expires_at {exp}"),
        )),
        None => checks.push(ProofCheck::new(
            "not_expired",
            ProofStatus::Pass,
            "no expiry set",
        )),
    }

    // 6. No newer contradiction.
    checks.push(ProofCheck::new(
        "no_newer_contradiction",
        if input.newer_contradiction {
            ProofStatus::Fail
        } else {
            ProofStatus::Pass
        },
        if input.newer_contradiction {
            "a newer memory contradicts this one"
        } else {
            "no contradiction found"
        },
    ));

    // 7. Source reliability above threshold.
    checks.push(ProofCheck::new(
        "source_reliable",
        if input.reliability >= RELIABILITY_THRESHOLD {
            ProofStatus::Pass
        } else {
            ProofStatus::Fail
        },
        format!(
            "reliability {:.2} (threshold {:.2})",
            input.reliability, RELIABILITY_THRESHOLD
        ),
    ));

    // 8. Human confirmed (informational).
    checks.push(ProofCheck::new(
        "human_confirmed",
        if input.human_confirmed {
            ProofStatus::Pass
        } else {
            ProofStatus::Skipped
        },
        if input.human_confirmed {
            "confirmed by a human"
        } else {
            "not human confirmed (optional)"
        },
    ));

    let applicable = checks
        .iter()
        .filter(|c| matches!(c.status, ProofStatus::Pass | ProofStatus::Fail))
        .count();
    let passed = checks
        .iter()
        .filter(|c| c.status == ProofStatus::Pass)
        .count();
    let any_fail = checks.iter().any(|c| c.status == ProofStatus::Fail);
    let score = if applicable == 0 {
        0.0
    } else {
        passed as f32 / applicable as f32
    };
    let verdict = if applicable == 0 {
        FreshnessVerdict::Unverifiable
    } else if any_fail {
        FreshnessVerdict::Stale
    } else {
        FreshnessVerdict::Fresh
    };

    let proof_hash = content_hash(
        "proof",
        serde_json::to_string(&checks)
            .unwrap_or_default()
            .as_bytes(),
    );

    FreshnessProof {
        memory_id: input.memory_id.to_string(),
        verdict,
        score,
        checks,
        proof_hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn base<'a>(id: &'a str) -> FreshnessInput<'a> {
        FreshnessInput {
            memory_id: id,
            source_file: None,
            source_line: None,
            source_commit: None,
            stored_span_hash: None,
            expires_at: None,
            human_confirmed: false,
            reliability: 0.8,
            newer_contradiction: false,
            commit_reachable: None,
        }
    }

    #[test]
    fn fresh_when_anchor_present_and_no_failures() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("src.rs");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "line1\nline2\nline3").unwrap();
        let mut input = base("m1");
        input.source_file = Some(&path);
        input.source_line = Some(2);
        let proof = prove(&input, Utc::now());
        assert_eq!(proof.verdict, FreshnessVerdict::Fresh);
        assert!(proof.score > 0.9);
    }

    #[test]
    fn stale_when_source_file_missing() {
        let missing = Path::new("definitely/not/here.rs");
        let mut input = base("m2");
        input.source_file = Some(missing);
        let proof = prove(&input, Utc::now());
        assert_eq!(proof.verdict, FreshnessVerdict::Stale);
    }

    #[test]
    fn stale_when_contradicted_or_expired() {
        let mut input = base("m3");
        input.newer_contradiction = true;
        assert_eq!(prove(&input, Utc::now()).verdict, FreshnessVerdict::Stale);

        let mut input = base("m4");
        input.expires_at = Some(Utc::now() - chrono::Duration::days(1));
        assert_eq!(prove(&input, Utc::now()).verdict, FreshnessVerdict::Stale);
    }

    #[test]
    fn stale_when_reliability_below_threshold() {
        let mut input = base("m5");
        input.reliability = 0.1;
        assert_eq!(prove(&input, Utc::now()).verdict, FreshnessVerdict::Stale);
    }

    #[test]
    fn proof_is_json_serializable_and_hashed() {
        let proof = prove(&base("m6"), Utc::now());
        assert!(proof.to_json()["checks"].is_array());
        assert!(proof.proof_hash.starts_with("proof:blake3:"));
    }
}
