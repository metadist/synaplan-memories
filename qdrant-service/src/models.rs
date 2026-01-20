use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingCapabilities {
    pub supported: bool,
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub device: String,
    pub vector_dimension: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceCapabilities {
    pub service: String,
    pub version: String,
    pub vector_dimension: u64,
    pub embedding: EmbeddingCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPayload {
    pub user_id: i64,
    pub category: String,
    pub key: String,
    pub value: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<i64>,
    pub created: i64,
    pub updated: i64,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpsertMemoryRequest {
    pub point_id: String,
    pub vector: Vec<f32>,
    pub payload: MemoryPayload,
}

#[derive(Debug, Deserialize)]
pub struct UpsertMemoryTextRequest {
    pub point_id: String,
    pub text: String,
    pub payload: MemoryPayload,
}

#[derive(Debug, Serialize)]
pub struct MemoryResponse {
    pub id: String,
    pub payload: MemoryPayload,
}

#[derive(Debug, Deserialize)]
pub struct SearchMemoriesRequest {
    pub query_vector: Vec<f32>,
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default = "default_min_score")]
    pub min_score: f32,
}

#[derive(Debug, Deserialize)]
pub struct SearchMemoriesTextRequest {
    pub query_text: String,
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default = "default_min_score")]
    pub min_score: f32,
}

fn default_limit() -> u64 {
    5
}

fn default_min_score() -> f32 {
    0.7
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub payload: MemoryPayload,
}

#[derive(Debug, Serialize)]
pub struct SearchMemoriesResponse {
    pub results: Vec<SearchResult>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct CollectionInfo {
    pub status: String,
    pub points_count: u64,
    pub vectors_count: u64,
    pub indexed_vectors_count: u64,
}

// Scroll (list all) memories request
#[derive(Debug, Deserialize)]
pub struct ScrollMemoriesRequest {
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default = "default_scroll_limit")]
    pub limit: u64,
}

fn default_scroll_limit() -> u64 {
    1000
}

// Scroll memories response
#[derive(Debug, Serialize)]
pub struct ScrollMemoriesResponse {
    pub memories: Vec<MemoryResponse>,
    pub count: usize,
}

// Unit tests for models
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_memory_payload_serialization() {
        let payload = MemoryPayload {
            user_id: 1,
            category: "personal".to_string(),
            key: "food_preferences".to_string(),
            value: "Loves kebab".to_string(),
            source: "auto_detected".to_string(),
            message_id: Some(2565),
            created: 1737115234,
            updated: 1737115234,
            active: true,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: MemoryPayload = serde_json::from_str(&json).unwrap();

        assert_eq!(payload.user_id, deserialized.user_id);
        assert_eq!(payload.category, deserialized.category);
        assert_eq!(payload.key, deserialized.key);
        assert_eq!(payload.value, deserialized.value);
    }

    #[test]
    fn test_search_request_defaults() {
        let req = SearchMemoriesRequest {
            query_vector: vec![0.1; 1024],
            user_id: 1,
            category: None,
            limit: default_limit(),
            min_score: default_min_score(),
        };

        assert_eq!(req.limit, 5);
        assert_eq!(req.min_score, 0.7);
    }

    #[test]
    fn test_upsert_request_deserialization() {
        let json = r#"{
            "point_id": "mem_1_123",
            "vector": [0.1, 0.2, 0.3],
            "payload": {
                "user_id": 1,
                "category": "test",
                "key": "test_key",
                "value": "test_value",
                "source": "manual",
                "created": 1234567890,
                "updated": 1234567890,
                "active": true
            }
        }"#;

        let req: UpsertMemoryRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.point_id, "mem_1_123");
        assert_eq!(req.vector.len(), 3);
        assert_eq!(req.payload.user_id, 1);
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            id: "mem_1_123".to_string(),
            score: 0.95,
            payload: MemoryPayload {
                user_id: 1,
                category: "test".to_string(),
                key: "test_key".to_string(),
                value: "test_value".to_string(),
                source: "test".to_string(),
                message_id: None,
                created: 1234567890,
                updated: 1234567890,
                active: true,
            },
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("mem_1_123"));
        assert!(json.contains("0.95"));
    }

    #[test]
    fn test_capabilities_serialization() {
        let caps = ServiceCapabilities {
            service: "synaplan-qdrant-service".to_string(),
            version: "0.0.0".to_string(),
            vector_dimension: 1024,
            embedding: EmbeddingCapabilities {
                supported: false,
                backend: "none".to_string(),
                model: None,
                device: "auto".to_string(),
                vector_dimension: 1024,
            },
        };

        let json = serde_json::to_string(&caps).unwrap();
        assert!(json.contains("synaplan-qdrant-service"));
        assert!(json.contains("\"supported\":false"));
        assert!(json.contains("\"vector_dimension\":1024"));
    }
}
