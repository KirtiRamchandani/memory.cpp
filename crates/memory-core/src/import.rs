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

    let files = collect_files(path, options.recursive)?;
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

fn collect_files(path: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut files = Vec::new();
    collect_dir(path, recursive, &mut files)?;
    Ok(files)
}

fn collect_dir(path: &Path, recursive: bool, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && recursive {
            collect_dir(&path, recursive, files)?;
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
