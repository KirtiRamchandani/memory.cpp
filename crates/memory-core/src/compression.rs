use crate::StoredMemory;

pub trait Compressor: Send + Sync {
    fn compress(&self, text: &str) -> String;
}

#[derive(Debug, Clone)]
pub struct CompressionConfig {
    pub max_summary_chars: usize,
    pub min_sentence_chars: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            max_summary_chars: 420,
            min_sentence_chars: 24,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeuristicCompressor {
    config: CompressionConfig,
}

impl HeuristicCompressor {
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    pub fn compress_collection(&self, memories: &[StoredMemory], max_chars: usize) -> String {
        let mut output = String::new();

        for memory in memories {
            let line = format!(
                "- [{}:{}] {}\n",
                memory.kind,
                memory.scope,
                if memory.summary.is_empty() {
                    self.compress(&memory.content)
                } else {
                    memory.summary.clone()
                }
            );

            if output.len() + line.len() > max_chars {
                break;
            }

            output.push_str(&line);
        }

        if output.is_empty() {
            "No memories were compacted.".to_string()
        } else {
            format!("Compacted long-term memory:\n{}", output.trim_end())
        }
    }
}

impl Compressor for HeuristicCompressor {
    fn compress(&self, text: &str) -> String {
        let normalized = normalize_whitespace(text);
        if normalized.len() <= self.config.max_summary_chars {
            return normalized;
        }

        let sentences = split_sentences(&normalized);
        if sentences.is_empty() {
            return truncate_at_boundary(&normalized, self.config.max_summary_chars);
        }

        let mut selected = Vec::new();

        if let Some(first) = sentences.first() {
            selected.push(first.clone());
        }

        let mut scored: Vec<_> = sentences
            .iter()
            .skip(1)
            .filter(|sentence| sentence.len() >= self.config.min_sentence_chars)
            .map(|sentence| (score_sentence(sentence), sentence.clone()))
            .collect();

        scored.sort_by(|left, right| {
            right
                .0
                .partial_cmp(&left.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (_, sentence) in scored {
            let candidate_len = selected.iter().map(String::len).sum::<usize>()
                + selected.len().saturating_sub(1)
                + sentence.len()
                + 1;

            if candidate_len > self.config.max_summary_chars {
                continue;
            }

            selected.push(sentence);
        }

        let summary = selected.join(" ");
        if summary.is_empty() {
            truncate_at_boundary(&normalized, self.config.max_summary_chars)
        } else {
            summary
        }
    }
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?' | '\n') {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                sentences.push(trimmed.to_string());
            }
            current.clear();
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        sentences.push(trimmed.to_string());
    }

    sentences
}

fn score_sentence(sentence: &str) -> f32 {
    let lower = sentence.to_ascii_lowercase();
    let mut score = 0.0;

    for keyword in [
        "prefer",
        "decision",
        "must",
        "should",
        "api",
        "error",
        "todo",
        "bug",
        "performance",
        "latency",
        "memory",
        "ship",
        "user",
    ] {
        if lower.contains(keyword) {
            score += 1.0;
        }
    }

    if sentence.chars().any(|ch| ch.is_ascii_digit()) {
        score += 0.7;
    }

    if sentence.contains("::") || sentence.contains("()") || sentence.contains('/') {
        score += 0.5;
    }

    score + (sentence.len().min(140) as f32 / 140.0)
}

fn truncate_at_boundary(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }

    let mut end = 0;
    for (idx, _) in text.char_indices() {
        if idx > max_chars {
            break;
        }
        end = idx;
    }

    let truncated = text[..end].trim_end_matches(|ch: char| !ch.is_alphanumeric());
    format!("{truncated}...")
}
