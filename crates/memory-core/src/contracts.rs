//! Memory contracts.
//!
//! An invariant becomes enforceable when it is turned into a *contract* that can
//! be checked against a diff. A contract forbids (or requires) patterns within a
//! scoped set of files, so a generated patch can be verified before it lands.

use serde::{Deserialize, Serialize};

/// Category of contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractType {
    Api,
    Database,
    Security,
    Style,
    Test,
    Dependency,
}

impl ContractType {
    pub fn parse(s: &str) -> ContractType {
        match s.trim().to_ascii_lowercase().as_str() {
            "api" => ContractType::Api,
            "database" | "db" => ContractType::Database,
            "security" => ContractType::Security,
            "style" => ContractType::Style,
            "test" => ContractType::Test,
            "dependency" | "dep" => ContractType::Dependency,
            _ => ContractType::Style,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            ContractType::Api => "api",
            ContractType::Database => "database",
            ContractType::Security => "security",
            ContractType::Style => "style",
            ContractType::Test => "test",
            ContractType::Dependency => "dependency",
        }
    }
}

/// One enforceable contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    pub id: String,
    pub contract_type: ContractType,
    pub rule: String,
    /// Added lines containing any of these (case-insensitive) violate the contract.
    #[serde(default)]
    pub forbid: Vec<String>,
    /// Files this contract applies to (substring match); empty = all files.
    #[serde(default)]
    pub scope_contains: Vec<String>,
}

/// A persistable set of contracts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContractStore {
    #[serde(default)]
    pub contracts: Vec<Contract>,
}

impl ContractStore {
    pub fn from_json(raw: &str) -> Self {
        serde_json::from_str(raw).unwrap_or_default()
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{\"contracts\":[]}".to_string())
    }
    pub fn add(&mut self, c: Contract) {
        self.contracts.retain(|x| x.id != c.id);
        self.contracts.push(c);
    }
}

/// A detected violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub contract_id: String,
    pub rule: String,
    pub file: String,
    pub line: String,
    pub matched: String,
}

impl Violation {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

fn in_scope(contract: &Contract, file: &str) -> bool {
    contract.scope_contains.is_empty()
        || contract
            .scope_contains
            .iter()
            .any(|s| file.to_ascii_lowercase().contains(&s.to_ascii_lowercase()))
}

/// Check contracts against a unified `git diff`. Only *added* lines (`+`) in
/// scoped files are evaluated.
pub fn check_diff(store: &ContractStore, diff: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    let mut current_file = String::new();
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            current_file = rest.trim().to_string();
            continue;
        }
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if !line.starts_with('+') {
            continue;
        }
        let added = &line[1..];
        let added_lower = added.to_ascii_lowercase();
        for c in &store.contracts {
            if !in_scope(c, &current_file) {
                continue;
            }
            for pat in &c.forbid {
                if added_lower.contains(&pat.to_ascii_lowercase()) {
                    violations.push(Violation {
                        contract_id: c.id.clone(),
                        rule: c.rule.clone(),
                        file: current_file.clone(),
                        line: added.trim().to_string(),
                        matched: pat.clone(),
                    });
                }
            }
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> ContractStore {
        let mut s = ContractStore::default();
        s.add(Contract {
            id: "no-db-in-controllers".into(),
            contract_type: ContractType::Database,
            rule: "Never call the database directly from controllers.".into(),
            forbid: vec!["db.query".into(), "database.execute".into()],
            scope_contains: vec!["controllers/".into()],
        });
        s
    }

    #[test]
    fn flags_forbidden_pattern_in_scope() {
        let diff = "+++ b/src/controllers/user.rs\n+    let rows = db.query(\"select 1\");\n";
        let v = check_diff(&store(), diff);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].contract_id, "no-db-in-controllers");
        assert_eq!(v[0].matched, "db.query");
    }

    #[test]
    fn ignores_out_of_scope_files() {
        let diff = "+++ b/src/services/user.rs\n+    let rows = db.query(\"x\");\n";
        assert!(check_diff(&store(), diff).is_empty());
    }

    #[test]
    fn ignores_removed_and_context_lines() {
        let diff = "+++ b/src/controllers/user.rs\n-    let rows = db.query(\"x\");\n     untouched db.query line\n";
        assert!(check_diff(&store(), diff).is_empty());
    }

    #[test]
    fn store_roundtrips_json() {
        let json = store().to_json();
        assert_eq!(ContractStore::from_json(&json).contracts.len(), 1);
    }
}
