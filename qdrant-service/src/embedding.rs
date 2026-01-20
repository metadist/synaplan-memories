use crate::error::AppError;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError>;
    fn backend(&self) -> String;
    fn model(&self) -> Option<String>;
    fn device(&self) -> String;
}

#[derive(Clone)]
pub struct OllamaEmbedder {
    client: Client,
    base_url: String,
    model: String,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Vec<f32>,
}

impl OllamaEmbedder {
    pub fn new(base_url: String, model: String) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_millis(800))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
        }
    }

    fn embeddings_url(&self) -> String {
        // Ollama embeddings endpoint (common): POST /api/embeddings
        format!("{}/api/embeddings", self.base_url)
    }
}

#[async_trait]
impl Embedder for OllamaEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError> {
        if text.trim().is_empty() {
            return Err(AppError::InvalidRequest(
                "Text must not be empty".to_string(),
            ));
        }

        let resp = self
            .client
            .post(self.embeddings_url())
            .json(&serde_json::json!({
                "model": self.model,
                "prompt": text,
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Embedding request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "Embedding request failed (HTTP {}): {}",
                status, body
            )));
        }

        let data: OllamaEmbeddingResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse embedding response: {}", e)))?;

        if data.embedding.is_empty() {
            return Err(AppError::Internal("Empty embedding returned".to_string()));
        }

        Ok(data.embedding)
    }

    fn backend(&self) -> String {
        "ollama".to_string()
    }

    fn model(&self) -> Option<String> {
        Some(self.model.clone())
    }

    fn device(&self) -> String {
        // Ollama decides device internally; we expose it as "external".
        "external".to_string()
    }
}


