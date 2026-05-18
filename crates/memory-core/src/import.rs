use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{MemoryEngine, MemoryKind, NewMemory, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    Auto,
    Text,
    Markdown,
    Json,
    Jsonl,
}

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub scope: String,
    pub kind: MemoryKind,
    pub format: ImportFormat,
    pub chunk_chars: usize,
    pub recursive: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            scope: "import".to_string(),
            kind: MemoryKind::Note,
            format: ImportFormat::Auto,
            chunk_chars: 1_800,
            recursive: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub root: PathBuf,
    pub imported: usize,
    pub skipped: usize,
    pub files: usize,
}

pub fn import_path(
    engine: &MemoryEngine,
    path: &Path,
    options: &ImportOptions,
) -> Result<ImportReport> {
    let mut report = ImportReport {
        root: path.to_path_buf(),
        imported: 0,
        skipped: 0,
        files: 0,
    };

    let files = collect_importable_files(path, options.recursive)?;
    for file in files {
        report.files += 1;
        let memories = parse_file(&file, options)?;
        if memories.is_empty() {
            report.skipped += 1;
            continue;
        }

        for mut memory in memories {
            memory.scope = options.scope.clone();
            memory.kind = options.kind;
            engine.remember(memory)?;
            report.imported += 1;
        }
    }

    Ok(report)
}

pub fn parse_file(path: &Path, options: &ImportOptions) -> Result<Vec<NewMemory>> {
    let raw = fs::read_to_string(path)?;
    let format = match options.format {
        ImportFormat::Auto => infer_format(path),
        explicit => explicit,
    };

    let metadata = json!({
        "source": {
            "path": path.to_string_lossy(),
            "format": format
        }
    });

    match format {
        ImportFormat::Json => parse_json(&raw, metadata, options.chunk_chars),
        ImportFormat::Jsonl => parse_jsonl(&raw, metadata, options.chunk_chars),
        ImportFormat::Markdown | ImportFormat::Text | ImportFormat::Auto => {
            Ok(chunk_text(&raw, options.chunk_chars)
                .into_iter()
                .map(|content| NewMemory::new(content).metadata(metadata.clone()))
                .collect())
        }
    }
}

pub fn collect_importable_files(path: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut files = Vec::new();
    let root = path.to_path_buf();
    let rules = load_ignore_rules(&root)?;
    collect_dir(&root, path, recursive, &rules, &mut files)?;
    Ok(files)
}

fn collect_dir(
    root: &Path,
    path: &Path,
    recursive: bool,
    rules: &[String],
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if is_ignored(root, &path, rules) {
            continue;
        }
        if path.is_dir() && recursive {
            collect_dir(root, &path, recursive, rules, files)?;
        } else if path.is_file() && is_importable(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn is_importable(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase()),
        Some(ext) if matches!(
            ext.as_str(),
            "txt" | "md" | "markdown" | "json" | "jsonl" | "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "c" | "cpp" | "h" | "hpp"
        )
    )
}

fn infer_format(path: &Path) -> ImportFormat {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("md" | "markdown") => ImportFormat::Markdown,
        Some("json") => ImportFormat::Json,
        Some("jsonl") => ImportFormat::Jsonl,
        _ => ImportFormat::Text,
    }
}

fn load_ignore_rules(root: &Path) -> Result<Vec<String>> {
    let mut rules = Vec::new();
    for name in [".memoryignore", ".gitignore"] {
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(path)?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            rules.push(trimmed.replace('\\', "/"));
        }
    }
    Ok(rules)
}

fn is_ignored(root: &Path, path: &Path, rules: &[String]) -> bool {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    rules
        .iter()
        .any(|rule| match_ignore_rule(rule, &relative, name))
}

fn match_ignore_rule(rule: &str, relative: &str, name: &str) -> bool {
    let normalized = rule.trim_start_matches("./").trim_matches('/');
    if normalized.is_empty() {
        return false;
    }

    if rule.ends_with('/') {
        return relative == normalized || relative.starts_with(&format!("{normalized}/"));
    }

    if let Some(ext) = normalized.strip_prefix("*.") {
        return relative.ends_with(&format!(".{ext}"));
    }

    if normalized.contains('*') {
        return wildcard_match(normalized, relative) || wildcard_match(normalized, name);
    }

    relative == normalized
        || relative.starts_with(&format!("{normalized}/"))
        || name == normalized
        || relative.contains(&format!("/{normalized}/"))
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return text == pattern;
    }

    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');
    let mut cursor = 0usize;

    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if index == 0 && anchored_start {
            if !text[cursor..].starts_with(part) {
                return false;
            }
            cursor += part.len();
            continue;
        }
        if let Some(offset) = text[cursor..].find(part) {
            cursor += offset + part.len();
        } else {
            return false;
        }
    }

    if anchored_end {
        if let Some(last) = parts.iter().rev().find(|part| !part.is_empty()) {
            return text.ends_with(last);
        }
    }

    true
}

fn parse_json(raw: &str, metadata: Value, chunk_chars: usize) -> Result<Vec<NewMemory>> {
    let value: Value = serde_json::from_str(raw)?;
    let mut texts = Vec::new();
    collect_json_text(&value, &mut texts);

    Ok(texts
        .into_iter()
        .flat_map(|text| chunk_text(&text, chunk_chars))
        .map(|content| NewMemory::new(content).metadata(metadata.clone()))
        .collect())
}

fn parse_jsonl(raw: &str, metadata: Value, chunk_chars: usize) -> Result<Vec<NewMemory>> {
    let mut memories = Vec::new();

    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line)?;
        let mut texts = Vec::new();
        collect_json_text(&value, &mut texts);

        for text in texts {
            for chunk in chunk_text(&text, chunk_chars) {
                memories.push(NewMemory::new(chunk).metadata(metadata.clone()));
            }
        }
    }

    Ok(memories)
}

fn collect_json_text(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) if text.split_whitespace().count() >= 4 => {
            output.push(text.clone());
        }
        Value::Array(items) => {
            for item in items {
                collect_json_text(item, output);
            }
        }
        Value::Object(map) => {
            for key in [
                "content", "text", "message", "summary", "title", "name", "parts",
            ] {
                if let Some(value) = map.get(key) {
                    collect_json_text(value, output);
                }
            }

            for key in ["messages", "mapping", "conversations", "items", "children"] {
                if let Some(value) = map.get(key) {
                    collect_json_text(value, output);
                }
            }
        }
        _ => {}
    }
}

fn chunk_text(text: &str, chunk_chars: usize) -> Vec<String> {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Vec::new();
    }

    let chunk_chars = chunk_chars.max(280);
    if normalized.len() <= chunk_chars {
        return vec![normalized];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for sentence in normalized.split_terminator(['.', '!', '?']) {
        let sentence = sentence.trim();
        if sentence.is_empty() {
            continue;
        }

        let sentence = format!("{sentence}.");
        if current.len() + sentence.len() + 1 > chunk_chars && !current.is_empty() {
            chunks.push(current.trim().to_string());
            current.clear();
        }

        current.push_str(&sentence);
        current.push(' ');
    }

    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::{is_ignored, match_ignore_rule, wildcard_match};
    use std::path::Path;

    #[test]
    fn wildcard_patterns_match_expected_paths() {
        assert!(wildcard_match("*.pem", "server.pem"));
        assert!(wildcard_match("secrets/*", "secrets/app.env"));
        assert!(wildcard_match(
            "node_modules/*",
            "node_modules/react/index.js"
        ));
        assert!(!wildcard_match("*.pem", "server.txt"));
    }

    #[test]
    fn ignore_rules_match_relative_paths_and_names() {
        let root = Path::new("repo");
        let rules = vec![
            ".env".to_string(),
            "secrets/".to_string(),
            "*.pem".to_string(),
            "node_modules/".to_string(),
        ];

        assert!(is_ignored(root, Path::new("repo/.env"), &rules));
        assert!(is_ignored(
            root,
            Path::new("repo/secrets/config.json"),
            &rules
        ));
        assert!(is_ignored(root, Path::new("repo/keys/prod.pem"), &rules));
        assert!(is_ignored(
            root,
            Path::new("repo/node_modules/react/index.js"),
            &rules
        ));
        assert!(!is_ignored(root, Path::new("repo/src/main.rs"), &rules));
    }

    #[test]
    fn direct_rule_matching_handles_directory_and_file_rules() {
        assert!(match_ignore_rule(
            "private/",
            "private/notes.txt",
            "notes.txt"
        ));
        assert!(match_ignore_rule(".env", ".env", ".env"));
        assert!(match_ignore_rule(
            "*.key",
            "keys/service.key",
            "service.key"
        ));
        assert!(!match_ignore_rule("docs/", "src/docs.rs", "docs.rs"));
    }
}
