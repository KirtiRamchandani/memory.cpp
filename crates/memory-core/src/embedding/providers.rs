use serde::Deserialize;
use serde_json::json;

use super::Embedder;
use crate::{vector::l2_normalize, MemoryError, Result};

#[derive(Debug, Clone)]
pub struct OllamaEmbedder {
    endpoint: String,
    model: String,
    dimensions: usize,
}

impl OllamaEmbedder {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>, dimensions: usize) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            dimensions,
        }
    }
}

impl Embedder for OllamaEmbedder {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.endpoint.trim_end_matches('/'));
        let response: OllamaEmbeddingResponse = ureq::post(&url)
            .send_json(json!({
                "model": self.model,
                "prompt": text,
            }))
            .map_err(|err| MemoryError::Http(err.to_string()))?
            .into_json()
            .map_err(|err| MemoryError::Http(err.to_string()))?;

        normalize_provider_vector(response.embedding)
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleEmbedder {
    endpoint: String,
    api_key: Option<String>,
    model: String,
    dimensions: usize,
}

impl OpenAiCompatibleEmbedder {
    pub fn new(
        endpoint: impl Into<String>,
        api_key: Option<String>,
        model: impl Into<String>,
        dimensions: usize,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key,
            model: model.into(),
            dimensions,
        }
    }
}

impl Embedder for OpenAiCompatibleEmbedder {
    fn name(&self) -> &'static str {
        "openai-compatible"
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/v1/embeddings", self.endpoint.trim_end_matches('/'));
        let mut request = ureq::post(&url).set("Content-Type", "application/json");

        if let Some(api_key) = &self.api_key {
            if !api_key.is_empty() {
                request = request.set("Authorization", &format!("Bearer {api_key}"));
            }
        }

        let response: OpenAiEmbeddingResponse = request
            .send_json(json!({
                "model": self.model,
                "input": text,
            }))
            .map_err(|err| MemoryError::Http(err.to_string()))?
            .into_json()
            .map_err(|err| MemoryError::Http(err.to_string()))?;

        let embedding = response
            .data
            .into_iter()
            .next()
            .ok_or_else(|| {
                MemoryError::Http("embedding response did not include data".to_string())
            })?
            .embedding;

        normalize_provider_vector(embedding)
    }
}

#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
}

fn normalize_provider_vector(mut vector: Vec<f32>) -> Result<Vec<f32>> {
    if vector.is_empty() {
        return Err(MemoryError::Embedder(
            "provider returned an empty vector".to_string(),
        ));
    }

    l2_normalize(&mut vector);
    Ok(vector)
}
