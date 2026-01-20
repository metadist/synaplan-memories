use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub qdrant_url: String,
    pub qdrant_api_key: Option<String>,
    pub collection_name: String,
    pub vector_dimension: u64,
    pub port: u16,
    pub service_api_key: Option<String>,
    pub tls_enabled: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub discord_webhook_url: Option<String>,
    /// Embedding backend used by this service (e.g. "none", "onnxruntime", "candle", "ollama").
    /// This is exposed via /capabilities for downstream routing decisions.
    pub embedding_backend: String,
    /// Embedding model identifier (e.g. "bge-m3"). Exposed via /capabilities.
    pub embedding_model: Option<String>,
    /// Device used for embeddings ("cpu", "cuda", "auto"). Exposed via /capabilities.
    pub embedding_device: String,

    /// Optional Ollama base URL for embedding backend "ollama" (e.g. http://ollama:11434)
    pub ollama_base_url: Option<String>,

    /// Native ONNX embedding model path (e.g. /models/bge-m3/model.onnx)
    pub embedding_onnx_model_path: Option<String>,
    /// Tokenizer.json path (e.g. /models/bge-m3/tokenizer.json)
    pub embedding_tokenizer_path: Option<String>,
    /// Max token length for embeddings (keep small for memories; e.g. 256/512)
    pub embedding_max_length: u32,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            qdrant_url: env::var("QDRANT_URL")
                .unwrap_or_else(|_| "http://localhost:6334".to_string()),
            qdrant_api_key: env::var("QDRANT_API_KEY").ok(),
            collection_name: env::var("QDRANT_COLLECTION_NAME")
                .unwrap_or_else(|_| "user_memories".to_string()),
            vector_dimension: env::var("QDRANT_VECTOR_DIMENSION")
                .unwrap_or_else(|_| "1024".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid QDRANT_VECTOR_DIMENSION: {}", e))?,
            port: env::var("PORT")
                .unwrap_or_else(|_| "8090".to_string())
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid PORT: {}", e))?,
            service_api_key: env::var("SERVICE_API_KEY").ok(),
            tls_enabled: env::var("TLS_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            tls_cert_path: env::var("TLS_CERT_PATH").ok(),
            tls_key_path: env::var("TLS_KEY_PATH").ok(),
            discord_webhook_url: env::var("DISCORD_WEBHOOK_URL").ok(),
            embedding_backend: env::var("EMBEDDING_BACKEND").unwrap_or_else(|_| "none".to_string()),
            embedding_model: env::var("EMBEDDING_MODEL").ok(),
            embedding_device: env::var("EMBEDDING_DEVICE").unwrap_or_else(|_| "auto".to_string()),
            ollama_base_url: env::var("OLLAMA_BASE_URL").ok(),
            embedding_onnx_model_path: env::var("EMBEDDING_ONNX_MODEL_PATH").ok(),
            embedding_tokenizer_path: env::var("EMBEDDING_TOKENIZER_PATH").ok(),
            embedding_max_length: env::var("EMBEDDING_MAX_LENGTH")
                .unwrap_or_else(|_| "512".to_string())
                .parse()
                .unwrap_or(512),
        })
    }

    #[cfg(test)]
    pub fn test_config() -> Self {
        Self {
            qdrant_url: "http://localhost:6334".to_string(),
            qdrant_api_key: None,
            collection_name: "test_collection".to_string(),
            vector_dimension: 128,
            port: 8090,
            service_api_key: None,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            discord_webhook_url: None,
            embedding_backend: "none".to_string(),
            embedding_model: None,
            embedding_device: "auto".to_string(),
            ollama_base_url: None,
            embedding_onnx_model_path: None,
            embedding_tokenizer_path: None,
            embedding_max_length: 512,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_defaults() {
        // Clear env vars for this test
        env::remove_var("QDRANT_URL");
        env::remove_var("QDRANT_API_KEY");
        env::remove_var("QDRANT_COLLECTION_NAME");
        env::remove_var("QDRANT_VECTOR_DIMENSION");
        env::remove_var("PORT");

        let config = Config::from_env().unwrap();

        assert_eq!(config.qdrant_url, "http://localhost:6334");
        assert_eq!(config.qdrant_api_key, None);
        assert_eq!(config.collection_name, "user_memories");
        assert_eq!(config.vector_dimension, 1024);
        assert_eq!(config.port, 8090);
    }

    #[test]
    fn test_config_from_env() {
        env::set_var("QDRANT_URL", "http://custom:6334");
        env::set_var("QDRANT_API_KEY", "test-key");
        env::set_var("QDRANT_COLLECTION_NAME", "test_collection");
        env::set_var("QDRANT_VECTOR_DIMENSION", "512");
        env::set_var("PORT", "9000");

        let config = Config::from_env().unwrap();

        assert_eq!(config.qdrant_url, "http://custom:6334");
        assert_eq!(config.qdrant_api_key, Some("test-key".to_string()));
        assert_eq!(config.collection_name, "test_collection");
        assert_eq!(config.vector_dimension, 512);
        assert_eq!(config.port, 9000);

        // Cleanup
        env::remove_var("QDRANT_URL");
        env::remove_var("QDRANT_API_KEY");
        env::remove_var("QDRANT_COLLECTION_NAME");
        env::remove_var("QDRANT_VECTOR_DIMENSION");
        env::remove_var("PORT");
    }

    #[test]
    fn test_config_invalid_dimension() {
        env::set_var("QDRANT_VECTOR_DIMENSION", "invalid");

        let result = Config::from_env();
        assert!(result.is_err());

        env::remove_var("QDRANT_VECTOR_DIMENSION");
    }

    #[test]
    fn test_config_invalid_port() {
        env::set_var("PORT", "99999");

        let result = Config::from_env();
        assert!(result.is_err());

        env::remove_var("PORT");
    }
}
