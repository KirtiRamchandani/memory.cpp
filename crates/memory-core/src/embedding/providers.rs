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

/// Embeddings through AI/ML API. The host is fixed: the API key is only ever sent
/// to `api.aimlapi.com`, and redirects are never followed.
#[derive(Debug, Clone)]
pub struct AimlapiEmbedder {
    base_url: String,
    api_key: Option<String>,
    model: String,
    dimensions: usize,
}

pub const AIMLAPI_BASE_URL: &str = "https://api.aimlapi.com";
pub const AIMLAPI_DEFAULT_MODEL: &str = "openai/text-embedding-3-small";

impl AimlapiEmbedder {
    pub fn new(api_key: Option<String>, model: impl Into<String>, dimensions: usize) -> Self {
        Self::with_base_url(AIMLAPI_BASE_URL, api_key, model, dimensions)
    }

    pub(crate) fn with_base_url(
        base_url: impl Into<String>,
        api_key: Option<String>,
        model: impl Into<String>,
        dimensions: usize,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            api_key,
            model: model.into(),
            dimensions,
        }
    }
}

impl Embedder for AimlapiEmbedder {
    fn name(&self) -> &'static str {
        "aimlapi"
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/v1/embeddings", self.base_url.trim_end_matches('/'));
        let agent = ureq::AgentBuilder::new().redirects(0).build();
        let mut request = agent.post(&url).set("Content-Type", "application/json");

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    /// One-shot HTTP server: returns the raw request it received.
    fn serve_once(response: String) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let _ = tx.send(String::from_utf8_lossy(&buf[..n]).to_string());
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (format!("http://{addr}"), rx)
    }

    #[test]
    fn default_base_url_is_exact_aimlapi_host() {
        assert_eq!(AIMLAPI_BASE_URL, "https://api.aimlapi.com");
        let embedder = AimlapiEmbedder::new(None, AIMLAPI_DEFAULT_MODEL, 8);
        assert_eq!(embedder.base_url, "https://api.aimlapi.com");
        assert_eq!(embedder.name(), "aimlapi");
    }

    #[test]
    fn sends_bearer_key_and_model_to_embeddings_path() {
        let body = r#"{"data":[{"embedding":[3.0,4.0]}]}"#;
        let (base, rx) = serve_once(format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ));
        let embedder =
            AimlapiEmbedder::with_base_url(base, Some("test-key".into()), AIMLAPI_DEFAULT_MODEL, 2);
        let vector = embedder.embed("hello").unwrap();
        assert!((vector[0] - 0.6).abs() < 1e-5 && (vector[1] - 0.8).abs() < 1e-5);
        let request = rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("post /v1/embeddings "));
        assert!(request.contains("authorization: bearer test-key"));
    }

    #[test]
    fn key_is_not_forwarded_on_redirect_to_another_host() {
        let (other_base, other_rx) = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_string(),
        );
        let (base, _rx) = serve_once(format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: {other_base}/v1/embeddings\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ));
        let embedder =
            AimlapiEmbedder::with_base_url(base, Some("test-key".into()), AIMLAPI_DEFAULT_MODEL, 2);
        assert!(embedder.embed("hello").is_err());
        assert!(other_rx
            .recv_timeout(std::time::Duration::from_millis(500))
            .is_err());
    }
}
