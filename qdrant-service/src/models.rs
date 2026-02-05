use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Service capabilities exposed via /capabilities endpoint
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ServiceCapabilities {
    pub service: String,
    pub version: String,
    pub vector_dimension: u64,
    pub embedding: EmbeddingCapabilities,
}

/// Embedding capabilities (always disabled - backend handles embedding)
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EmbeddingCapabilities {
    pub supported: bool,
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub device: String,
    pub vector_dimension: u64,
}

/// Memory payload structure
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "user_id": 1730,
    "category": "personal",
    "key": "name",
    "value": "Yusuf Senel",
    "source": "auto_detected",
    "message_id": 4488,
    "created": 1769034136,
    "updated": 1769034136,
    "active": true
}))]
pub struct MemoryPayload {
    #[schema(example = 1730)]
    pub user_id: i64,
    #[schema(example = "personal")]
    pub category: String,
    #[schema(example = "name")]
    pub key: String,
    #[schema(example = "Yusuf Senel")]
    pub value: String,
    #[schema(example = "auto_detected")]
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 4488)]
    pub message_id: Option<i64>,
    #[schema(example = 1769034136)]
    pub created: i64,
    #[schema(example = 1769034136)]
    pub updated: i64,
    #[schema(example = true)]
    pub active: bool,
}

/// Upsert memory with pre-computed vector
#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "point_id": "mem_1730_12345",
    "vector": [0.1, 0.2, 0.3],
    "payload": {
        "user_id": 1730,
        "category": "personal",
        "key": "name",
        "value": "Yusuf Senel",
        "source": "auto_detected",
        "created": 1769034136,
        "updated": 1769034136,
        "active": true
    }
}))]
pub struct UpsertMemoryRequest {
    #[schema(example = "mem_1730_12345")]
    pub point_id: String,
    #[schema(example = json!([0.1, 0.2, 0.3]))]
    pub vector: Vec<f32>,
    pub payload: MemoryPayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "feedback_false_positive")]
    pub namespace: Option<String>,
}

/// Batch upsert multiple memories
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchUpsertRequest {
    #[schema(min_items = 1, max_items = 100)]
    pub points: Vec<UpsertMemoryRequest>,
}

/// Memory response
#[derive(Debug, Serialize, ToSchema)]
pub struct MemoryResponse {
    pub id: String,
    pub payload: MemoryPayload,
}

/// Search memories by vector
#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({
    "query_vector": [0.1, 0.2, 0.3],
    "user_id": 1730,
    "category": "personal",
    "limit": 15,
    "min_score": 0.35
}))]
pub struct SearchMemoriesRequest {
    #[schema(example = json!([0.1, 0.2, 0.3]))]
    pub query_vector: Vec<f32>,
    #[schema(example = 1730)]
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "personal")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "feedback_false_positive")]
    pub namespace: Option<String>,
    #[serde(default = "default_limit")]
    #[schema(example = 15, minimum = 1, maximum = 100)]
    pub limit: u64,
    #[serde(default = "default_min_score")]
    #[schema(example = 0.35, minimum = 0.0, maximum = 1.0)]
    pub min_score: f32,
}

/// Search result with score
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResult {
    pub id: String,
    #[schema(example = 0.95, minimum = 0.0, maximum = 1.0)]
    pub score: f32,
    pub payload: MemoryPayload,
}

/// Search response
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchMemoriesResponse {
    pub results: Vec<SearchResult>,
    pub count: usize,
}

/// Scroll (list all) memories request
#[derive(Debug, Deserialize, ToSchema)]
pub struct ScrollMemoriesRequest {
    #[schema(example = 1730)]
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "personal")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "feedback_false_positive")]
    pub namespace: Option<String>,
    #[serde(default = "default_scroll_limit")]
    #[schema(example = 1000, minimum = 1, maximum = 10000)]
    pub limit: u64,
}

/// Scroll memories response
#[derive(Debug, Serialize, ToSchema)]
pub struct ScrollMemoriesResponse {
    pub memories: Vec<MemoryResponse>,
    pub count: usize,
}

/// Collection information
#[derive(Debug, Serialize, ToSchema)]
pub struct CollectionInfo {
    pub status: String,
    pub points_count: u64,
    pub vectors_count: u64,
    pub indexed_vectors_count: u64,
}

/// Batch operation response
#[derive(Debug, Serialize, ToSchema)]
pub struct BatchOperationResponse {
    pub success_count: usize,
    pub failed_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<BatchError>,
}

/// Individual batch operation error
#[derive(Debug, Serialize, ToSchema)]
pub struct BatchError {
    pub point_id: String,
    pub error: String,
}

/// Document chunk payload stored in Qdrant
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DocumentPayload {
    /// User ID for multi-tenant isolation
    pub user_id: i64,
    /// Reference to source file (BFILES.BID)
    pub file_id: i64,
    /// Grouping key (e.g., "WIDGET:xxx", "TASKPROMPT:xxx", "DEFAULT")
    pub group_key: String,
    /// File type identifier
    pub file_type: i32,
    /// Chunk position in file
    pub chunk_index: i32,
    /// Source line start
    pub start_line: i32,
    /// Source line end  
    pub end_line: i32,
    /// Chunk text content
    pub text: String,
    /// Unix timestamp
    pub created: i64,
}

/// Request to upsert a document chunk
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertDocumentRequest {
    /// Unique point ID (e.g., "doc_1_123_0")
    pub point_id: String,
    /// Vector embedding (must be exactly 1024 dimensions)
    pub vector: Vec<f32>,
    /// Document payload
    pub payload: DocumentPayload,
}

/// Request for batch document upsert
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchUpsertDocumentsRequest {
    /// Array of documents to upsert (max 100)
    pub documents: Vec<UpsertDocumentRequest>,
}

/// Response for batch operations
#[derive(Debug, Serialize, ToSchema)]
pub struct BatchUpsertResponse {
    pub success_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

/// Request to search documents
#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchDocumentsRequest {
    /// Query vector (must be exactly 1024 dimensions)
    pub vector: Vec<f32>,
    /// User ID (required for isolation)
    pub user_id: i64,
    /// Optional group key filter
    #[serde(default)]
    pub group_key: Option<String>,
    /// Maximum results (default: 10)
    #[serde(default = "default_limit")]
    pub limit: u64,
    /// Minimum similarity score (default: 0.3)
    #[serde(default = "default_min_score")]
    pub min_score: f32,
}

/// Search result
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentSearchResult {
    /// Point ID
    pub id: String,
    /// Similarity score (0.0 - 1.0)
    pub score: f32,
    /// Document payload
    pub payload: DocumentPayload,
    /// Vector (optional, only returned by get_document)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
}

/// Request to delete documents by file
#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteByFileRequest {
    pub user_id: i64,
    pub file_id: i64,
}

/// Request to delete documents by group key
#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteByGroupKeyRequest {
    pub user_id: i64,
    pub group_key: String,
}

/// Request to update group key
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateGroupKeyRequest {
    pub user_id: i64,
    pub file_id: i64,
    pub new_group_key: String,
}

/// Document statistics response
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentStatsResponse {
    pub total_chunks: u64,
    pub total_files: u64,
    pub total_groups: u64,
    pub chunks_by_group: std::collections::HashMap<String, u64>,
}

// Default functions
#[inline]
const fn default_limit() -> u64 {
    5
}

#[inline]
const fn default_min_score() -> f32 {
    0.7
}

#[inline]
const fn default_scroll_limit() -> u64 {
    1000
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
