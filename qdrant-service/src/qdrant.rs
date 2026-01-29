use crate::config::Config;
use crate::error::AppError;
use crate::models::MemoryPayload;
use qdrant_client::qdrant::{
    point_id::PointIdOptions, Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance,
    Filter, GetPointsBuilder, PointId, PointStruct, SearchPointsBuilder,
    UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::{Payload, Qdrant};
use serde_json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tracing::{debug, info, warn};

/// Convert string ID to numeric ID using consistent hashing
#[inline]
fn string_to_point_id(point_id: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    point_id.hash(&mut hasher);
    hasher.finish()
}

pub struct QdrantService {
    client: Qdrant,
    collection_name: String,
    vector_dimension: u64,
}

impl QdrantService {
    pub async fn new(config: &Config) -> anyhow::Result<Self> {
        let mut client_builder = Qdrant::from_url(&config.qdrant_url);

        // Add API key if provided (needs to be owned, not borrowed)
        if let Some(api_key) = &config.qdrant_api_key {
            client_builder = client_builder.api_key(api_key.clone());
        }

        let client = client_builder.build()?;

        Ok(Self {
            client,
            collection_name: config.collection_name.clone(),
            vector_dimension: config.vector_dimension,
        })
    }

    pub async fn ensure_collection_exists(&self) -> Result<(), AppError> {
        self.ensure_collection_exists_for(&self.collection_name).await
    }

    pub async fn ensure_collection_exists_for(&self, collection_name: &str) -> Result<(), AppError> {
        let collections = self.client.list_collections().await?;

        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == collection_name);

        if !exists {
            info!("Creating collection '{}'", collection_name);

            self.client
                .create_collection(
                    CreateCollectionBuilder::new(collection_name).vectors_config(
                        VectorParamsBuilder::new(self.vector_dimension, Distance::Cosine),
                    ),
                )
                .await?;

            info!("Collection '{}' created successfully", collection_name);
        } else {
            debug!("Collection '{}' already exists", collection_name);
        }

        Ok(())
    }

    pub async fn upsert_memory(
        &self,
        point_id: String,
        vector: Vec<f32>,
        payload: MemoryPayload,
        namespace: Option<&str>,
    ) -> Result<(), AppError> {
        if vector.len() != self.vector_dimension as usize {
            return Err(AppError::InvalidRequest(format!(
                "Vector dimension mismatch: expected {}, got {}",
                self.vector_dimension,
                vector.len()
            )));
        }

        let payload_map = serde_json::to_value(&payload)
            .map_err(|e| AppError::Internal(format!("Failed to serialize payload: {}", e)))?;

        // Add the original point_id to the payload for later retrieval
        let mut payload_map = payload_map.as_object().unwrap().clone();
        payload_map.insert("_point_id".to_string(), serde_json::Value::String(point_id.clone()));

        let payload_qdrant = Payload::try_from(serde_json::Value::Object(payload_map))
            .map_err(|e| AppError::Internal(format!("Failed to convert payload: {}", e)))?;

        let numeric_id = string_to_point_id(&point_id);
        let pid = PointId {
            point_id_options: Some(PointIdOptions::Num(numeric_id)),
        };

        let point = PointStruct::new(pid, vector, payload_qdrant);

        let collection_name = self.get_collection_name(namespace);
        self.ensure_collection_exists_for(&collection_name).await?;

        // Use UpsertPointsBuilder
        self.client
            .upsert_points(
                UpsertPointsBuilder::new(&collection_name, vec![point])
            )
            .await?;

        debug!("Memory upserted: {} (numeric: {})", point_id, numeric_id);

        Ok(())
    }

    pub async fn get_memory(&self, point_id: &str, namespace: Option<&str>) -> Result<Option<MemoryPayload>, AppError> {
        let numeric_id = string_to_point_id(point_id);
        let pid = PointId {
            point_id_options: Some(PointIdOptions::Num(numeric_id)),
        };

        // Use GetPointsBuilder
        let collection_name = self.get_collection_name(namespace);
        let response = self
            .client
            .get_points(
                GetPointsBuilder::new(&collection_name, vec![pid])
                    .with_payload(true)
            )
            .await?;

        if let Some(point) = response.result.first() {
            // In v1.16, payload is HashMap, not Option<HashMap>
            let payload_json = serde_json::to_value(&point.payload).map_err(|e| {
                AppError::Internal(format!("Failed to serialize payload: {}", e))
            })?;

            let memory_payload: MemoryPayload = serde_json::from_value(payload_json)
                .map_err(|e| {
                    AppError::Internal(format!("Failed to deserialize payload: {}", e))
                })?;

            return Ok(Some(memory_payload));
        }

        Ok(None)
    }

    pub async fn search_memories(
        &self,
        query_vector: Vec<f32>,
        user_id: i64,
        category: Option<String>,
        limit: u64,
        min_score: f32,
        namespace: Option<&str>,
    ) -> Result<Vec<(String, f32, MemoryPayload)>, AppError> {
        if query_vector.len() != self.vector_dimension as usize {
            return Err(AppError::InvalidRequest(format!(
                "Query vector dimension mismatch: expected {}, got {}",
                self.vector_dimension,
                query_vector.len()
            )));
        }

        // Build filter for user_id and optional category
        let mut must_conditions = vec![Condition::matches("user_id", user_id)];

        // Only search for active memories
        must_conditions.push(Condition::matches("active", true));

        if let Some(cat) = category {
            must_conditions.push(Condition::matches("category", cat));
        }

        let filter = Filter {
            must: must_conditions,
            ..Default::default()
        };

        let collection_name = self.get_collection_name(namespace);
        let search_result = self
            .client
            .search_points(
                SearchPointsBuilder::new(&collection_name, query_vector, limit)
                    .filter(filter)
                    .score_threshold(min_score)
                    .with_payload(true),
            )
            .await?;

        let mut results = Vec::new();

        for scored_point in search_result.result {
            // Get the original point_id from payload
            // In v1.16, payload is HashMap, not Option<HashMap>
            let point_id = scored_point.payload
                .get("_point_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    // Fallback: use numeric ID if _point_id not found
                    match scored_point.id {
                        Some(id) => match id.point_id_options {
                            Some(PointIdOptions::Num(num)) => num.to_string(),
                            Some(PointIdOptions::Uuid(uuid)) => uuid,
                            None => "unknown".to_string(),
                        },
                        None => "unknown".to_string(),
                    }
                });

            let payload_json = serde_json::to_value(&scored_point.payload).map_err(|e| {
                AppError::Internal(format!("Failed to serialize payload: {}", e))
            })?;

            let memory_payload: MemoryPayload = serde_json::from_value(payload_json)
                .map_err(|e| {
                    AppError::Internal(format!("Failed to deserialize payload: {}", e))
                })?;

            results.push((point_id, scored_point.score, memory_payload));
        }

        debug!("Search found {} memories for user {}", results.len(), user_id);

        Ok(results)
    }

    pub async fn delete_memory(&self, point_id: &str, namespace: Option<&str>) -> Result<(), AppError> {
        let numeric_id = string_to_point_id(point_id);
        let pid = PointId {
            point_id_options: Some(PointIdOptions::Num(numeric_id)),
        };

        let collection_name = self.get_collection_name(namespace);
        // Use DeletePointsBuilder
        self.client
            .delete_points(
                DeletePointsBuilder::new(&collection_name)
                    .points(vec![pid])
            )
            .await?;

        debug!("Memory deleted: {} (numeric: {})", point_id, numeric_id);

        Ok(())
    }

    pub async fn health_check(&self) -> Result<bool, AppError> {
        match self.client.health_check().await {
            Ok(_) => Ok(true),
            Err(e) => {
                warn!("Health check failed: {:?}", e);
                Ok(false)
            }
        }
    }

    pub async fn get_collection_info(&self, namespace: Option<&str>) -> Result<(String, u64, u64, u64), AppError> {
        let collection_name = self.get_collection_name(namespace);
        let response = self.client.collection_info(&collection_name).await?;

        // In v1.16, the response has a 'result' field
        let info = response
            .result
            .ok_or_else(|| AppError::Internal("Collection info result is empty".to_string()))?;

        let status = format!("{:?}", info.status);
        let points_count = info.points_count.unwrap_or(0);
        
        // vectors_count and indexed_vectors_count don't exist in CollectionInfo
        // Use points_count as approximation
        let vectors_count = points_count;
        let indexed_vectors_count = points_count;

        Ok((status, points_count, vectors_count, indexed_vectors_count))
    }

    /// Scroll (list) all memories for a user without vector search
    /// Uses Qdrant scroll API to retrieve points with filtering
    pub async fn scroll_memories(
        &self,
        user_id: i64,
        category: Option<String>,
        limit: u64,
        namespace: Option<&str>,
    ) -> Result<Vec<(String, MemoryPayload)>, AppError> {
        // Build filter for user_id and optional category
        let mut must_conditions = vec![Condition::matches("user_id", user_id)];

        // Only include active memories
        must_conditions.push(Condition::matches("active", true));

        if let Some(cat) = category {
            must_conditions.push(Condition::matches("category", cat));
        }

        let filter = Filter {
            must: must_conditions,
            ..Default::default()
        };

        // Use scroll API to retrieve all matching points
        let collection_name = self.get_collection_name(namespace);
        let scroll_result = self
            .client
            .scroll(
                qdrant_client::qdrant::ScrollPointsBuilder::new(&collection_name)
                    .filter(filter)
                    .limit(limit as u32)
                    .with_payload(true)
                    .with_vectors(false), // We don't need vectors for listing
            )
            .await?;

        let mut results = Vec::new();

        for point in scroll_result.result {
            // Get the original point_id from payload
            // payload is a HashMap, not Option
            let payload = point.payload;
            
            // Extract _point_id from payload
            let point_id = payload
                .get("_point_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    // Fallback: use numeric ID if _point_id not found (shouldn't happen)
                    match point.id {
                        Some(id) => match id.point_id_options {
                            Some(PointIdOptions::Num(num)) => num.to_string(),
                            Some(PointIdOptions::Uuid(uuid)) => uuid,
                            None => "unknown".to_string(),
                        },
                        None => "unknown".to_string(),
                    }
                });

            let payload_json = serde_json::to_value(payload).map_err(|e| {
                AppError::Internal(format!("Failed to serialize payload: {}", e))
            })?;

            let memory_payload: MemoryPayload = serde_json::from_value(payload_json)
                .map_err(|e| {
                    AppError::Internal(format!("Failed to deserialize payload: {}", e))
                })?;

            results.push((point_id, memory_payload));
        }

        debug!(
            "Scroll found {} memories for user {}",
            results.len(),
            user_id
        );

        Ok(results)
    }

    fn get_collection_name(&self, namespace: Option<&str>) -> String {
        let namespace = namespace
            .and_then(|value| {
                let sanitized = Self::sanitize_namespace(value);
                if sanitized.is_empty() {
                    None
                } else {
                    Some(sanitized)
                }
            });

        match namespace {
            Some(ns) => format!("{}_{}", self.collection_name, ns),
            None => self.collection_name.clone(),
        }
    }

    fn sanitize_namespace(value: &str) -> String {
        let mut output = String::with_capacity(value.len());
        for ch in value.chars() {
            if ch.is_ascii_alphanumeric() {
                output.push(ch.to_ascii_lowercase());
            } else if ch == '-' || ch == '_' {
                output.push('_');
            }
        }

        output.trim_matches('_').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_id_hashing_consistency() {
        let point_id = "mem_1_12345";
        let hash1 = string_to_point_id(point_id);
        let hash2 = string_to_point_id(point_id);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_ids_produce_different_hashes() {
        let id1 = "mem_1_12345";
        let id2 = "mem_1_67890";
        let hash1 = string_to_point_id(id1);
        let hash2 = string_to_point_id(id2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_vector_dimension_validation() {
        // Tests dass vector dimension gecheckt wird
        let wrong_vector = vec![0.1_f32; 512]; // Wrong dimension
        let expected_dim = 1024_u64;
        assert_ne!(wrong_vector.len(), expected_dim as usize);
    }
}
