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
    pub webhook_url: Option<String>,
    pub enable_daily_stats: bool,
    pub stats_interval_hours: u64,
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
            webhook_url: env::var("WEBHOOK_URL").ok(),
            enable_daily_stats: env::var("ENABLE_DAILY_STATS")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            stats_interval_hours: env::var("STATS_INTERVAL_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .unwrap_or(24),
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
            webhook_url: None,
            enable_daily_stats: false,
            stats_interval_hours: 24,
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

    #[test]
    fn test_webhook_url() {
        env::set_var("WEBHOOK_URL", "https://discord.com/api/webhooks/123");

        let config = Config::from_env().unwrap();
        assert_eq!(
            config.webhook_url,
            Some("https://discord.com/api/webhooks/123".to_string())
        );

        env::remove_var("WEBHOOK_URL");
    }

    #[test]
    fn test_tls_config() {
        env::set_var("TLS_ENABLED", "true");
        env::set_var("TLS_CERT_PATH", "/path/to/cert.pem");
        env::set_var("TLS_KEY_PATH", "/path/to/key.pem");

        let config = Config::from_env().unwrap();
        assert!(config.tls_enabled);
        assert_eq!(config.tls_cert_path, Some("/path/to/cert.pem".to_string()));
        assert_eq!(config.tls_key_path, Some("/path/to/key.pem".to_string()));

        env::remove_var("TLS_ENABLED");
        env::remove_var("TLS_CERT_PATH");
        env::remove_var("TLS_KEY_PATH");
    }
}
