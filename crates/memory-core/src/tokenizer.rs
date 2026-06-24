//! Tokenizer abstraction.
//!
//! Token counts drive every budget, cache, and KV estimate in the compiler, so
//! they deserve a real abstraction rather than a hard-coded `chars / 4`.
//!
//! memory.cpp ships dependency-free *estimators* by default and is honest about
//! it: every count produced here is an estimate. A provider-specific BPE
//! tokenizer can be plugged in later (behind a feature) to produce *measured*
//! counts; until then [`TokenCount::measured`] is always `false`.

use serde::{Deserialize, Serialize};

/// How tokens were counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenizerMode {
    /// Generic char/word heuristic.
    GenericApproximation,
    /// Provider-tuned heuristic (still an estimate).
    ProviderApproximation,
    /// A real provider/model tokenizer produced this count.
    Measured,
}

/// A token count plus how it was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenCount {
    pub tokens: usize,
    pub mode: TokenizerMode,
    /// True only when produced by a real tokenizer.
    pub measured: bool,
}

impl TokenCount {
    pub fn to_json(self) -> serde_json::Value {
        serde_json::json!({
            "tokens": self.tokens,
            "mode": match self.mode {
                TokenizerMode::GenericApproximation => "generic_approximation",
                TokenizerMode::ProviderApproximation => "provider_approximation",
                TokenizerMode::Measured => "measured",
            },
            "measured": self.measured,
        })
    }
}

/// A pluggable tokenizer. The default variants are estimators.
#[derive(Debug, Clone)]
pub enum Tokenizer {
    /// chars/4 baseline, kept as a dependency-free fallback.
    Generic,
    /// Provider-tuned estimator (different average chars-per-token).
    Provider(Provider),
}

/// Known providers with tuned chars-per-token ratios. These ratios are public
/// rules of thumb, not vendor tokenizers; counts remain estimates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenAi,
    Anthropic,
    Gemini,
    Generic,
}

impl Provider {
    pub fn parse(s: &str) -> Provider {
        match s.trim().to_ascii_lowercase().as_str() {
            "openai" | "codex" | "gpt" => Provider::OpenAi,
            "anthropic" | "claude" => Provider::Anthropic,
            "gemini" | "google" => Provider::Gemini,
            _ => Provider::Generic,
        }
    }

    /// Average characters per token used by the estimator.
    fn chars_per_token(self) -> f64 {
        match self {
            // Common English-text rules of thumb.
            Provider::OpenAi => 4.0,
            Provider::Anthropic => 3.8,
            Provider::Gemini => 4.1,
            Provider::Generic => 4.0,
        }
    }
}

impl Tokenizer {
    pub fn for_provider(provider: &str) -> Tokenizer {
        match Provider::parse(provider) {
            Provider::Generic => Tokenizer::Generic,
            p => Tokenizer::Provider(p),
        }
    }

    /// Count tokens. Blends a character-rate estimate with a word-count floor,
    /// because both pure-char and pure-word estimates are wrong on code.
    pub fn count(&self, text: &str) -> TokenCount {
        let chars = text.chars().count();
        if chars == 0 {
            return TokenCount {
                tokens: 0,
                mode: self.mode(),
                measured: false,
            };
        }
        let words = text.split_whitespace().count().max(1);
        let (cpt, mode) = match self {
            Tokenizer::Generic => (
                Provider::Generic.chars_per_token(),
                TokenizerMode::GenericApproximation,
            ),
            Tokenizer::Provider(p) => (p.chars_per_token(), TokenizerMode::ProviderApproximation),
        };
        let char_estimate = (chars as f64 / cpt).ceil() as usize;
        // Code and punctuation push token counts above the word count; use the
        // larger of the char estimate and ~1.3 tokens/word.
        let word_estimate = ((words as f64) * 1.3).ceil() as usize;
        TokenCount {
            tokens: char_estimate.max(word_estimate),
            mode,
            measured: false,
        }
    }

    fn mode(&self) -> TokenizerMode {
        match self {
            Tokenizer::Generic => TokenizerMode::GenericApproximation,
            Tokenizer::Provider(_) => TokenizerMode::ProviderApproximation,
        }
    }
}

/// Convenience: generic estimate, matching the legacy `chars/4` behaviour
/// closely enough for existing call sites.
pub fn estimate_tokens(text: &str) -> usize {
    Tokenizer::Generic.count(text).tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(Tokenizer::Generic.count("").tokens, 0);
    }

    #[test]
    fn counts_scale_with_length() {
        let short = Tokenizer::Generic.count("hello world");
        let long = Tokenizer::Generic.count(&"hello world ".repeat(100));
        assert!(long.tokens > short.tokens * 50);
    }

    #[test]
    fn provider_tokenizers_are_estimates() {
        let c = Tokenizer::for_provider("anthropic").count("some repo context here");
        assert!(!c.measured);
        assert_eq!(c.mode, TokenizerMode::ProviderApproximation);
    }

    #[test]
    fn generic_provider_falls_back_to_generic_mode() {
        let c = Tokenizer::for_provider("something-unknown").count("text");
        assert_eq!(c.mode, TokenizerMode::GenericApproximation);
    }

    #[test]
    fn json_labels_measured_flag() {
        let json = Tokenizer::for_provider("openai")
            .count("abcd efgh")
            .to_json();
        assert_eq!(json["measured"], false);
    }
}
