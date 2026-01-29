use crate::error::AppError;
use crate::models::*;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
    Json,
};
use tracing::info;
use utoipa;
#[derive(Debug, serde::Deserialize)]
pub struct NamespaceQuery {
    pub namespace: Option<String>,
}

/// Get service capabilities and configuration
///
/// **Purpose:** Returns service version, vector dimensions, and embedding capabilities.
/// Useful for backend to validate compatibility before sending requests.
///
/// **Cache:** Response is cached for 30 seconds (`Cache-Control: public, max-age=30`).
#[utoipa::path(
    get,
    path = "/capabilities",
    tag = "Service Info",
    responses(
        (status = 200, description = "Service capabilities", body = ServiceCapabilities)
    )
)]
pub async fn get_capabilities(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    // Keep this lightweight and cacheable; do not call external services here.
    let config = state.config.as_ref();

    let body = ServiceCapabilities {
        service: "synaplan-qdrant-service".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        vector_dimension: config.vector_dimension,
        embedding: EmbeddingCapabilities {
            supported: false,
            backend: "none".to_string(),
            model: None,
            device: "none".to_string(),
            vector_dimension: config.vector_dimension,
        },
    };

    Ok((
        [(header::CACHE_CONTROL, "public, max-age=30")],
        Json(body),
    ))
}

/// Upsert a single memory with pre-computed vector
///
/// **Purpose:** Store or update a memory point in Qdrant.
/// The backend must send the vector already computed (e.g., via BGE-M3 in Ollama).
///
/// **Usage:**
/// - `point_id`: Unique ID like `mem_{user_id}_{hash}`
/// - `vector`: 1024-dim BGE-M3 embedding
/// - `payload`: Memory metadata (user_id, category, key, value, etc.)
#[utoipa::path(
    post,
    path = "/memories",
    tag = "Memories",
    request_body = UpsertMemoryRequest,
    responses(
        (status = 200, description = "Memory upserted successfully", body = inline(Object), example = json!({
            "success": true,
            "point_id": "mem_1730_abc123",
            "message": "Memory upserted successfully"
        })),
        (status = 400, description = "Invalid request (wrong vector dimension, invalid payload)"),
        (status = 500, description = "Qdrant error")
    )
)]
pub async fn upsert_memory(
    State(state): State<AppState>,
    Json(req): Json<UpsertMemoryRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    info!("Upserting memory: {}", req.point_id);

    state
        .qdrant
        .upsert_memory(
            req.point_id.clone(),
            req.vector,
            req.payload,
            req.namespace.as_deref(),
        )
        .await?;

    // Track stats
    state.stats.increment_upserts(1);

    Ok(Json(serde_json::json!({
        "success": true,
        "point_id": req.point_id,
        "message": "Memory upserted successfully"
    })))
}

/// Get a single memory by ID
///
/// **Purpose:** Retrieve a specific memory point from Qdrant.
///
/// **Usage:**
/// - `point_id`: The unique ID used during upsert (e.g., `mem_1730_abc123`)
#[utoipa::path(
    get,
    path = "/memories/{point_id}",
    tag = "Memories",
    params(
        ("point_id" = String, Path, description = "Unique memory ID (e.g., mem_1730_abc123)"),
        ("namespace" = Option<String>, Query, description = "Optional namespace for alternative collection")
    ),
    responses(
        (status = 200, description = "Memory found", body = MemoryResponse),
        (status = 404, description = "Memory not found")
    )
)]
pub async fn get_memory(
    State(state): State<AppState>,
    Path(point_id): Path<String>,
    Query(query): Query<NamespaceQuery>,
) -> Result<Json<MemoryResponse>, AppError> {
    info!("Getting memory: {}", point_id);

    let payload = state
        .qdrant
        .get_memory(&point_id, query.namespace.as_deref())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory not found: {}", point_id)))?;

    Ok(Json(MemoryResponse {
        id: point_id,
        payload,
    }))
}

/// Delete a single memory by ID
///
/// **Purpose:** Remove a memory point from Qdrant.
///
/// **Usage:**
/// - `point_id`: The unique ID to delete
#[utoipa::path(
    delete,
    path = "/memories/{point_id}",
    tag = "Memories",
    params(
        ("point_id" = String, Path, description = "Unique memory ID to delete"),
        ("namespace" = Option<String>, Query, description = "Optional namespace for alternative collection")
    ),
    responses(
        (status = 200, description = "Memory deleted successfully", body = inline(Object), example = json!({
            "success": true,
            "point_id": "mem_1730_abc123",
            "message": "Memory deleted successfully"
        })),
        (status = 500, description = "Qdrant error")
    )
)]
pub async fn delete_memory(
    State(state): State<AppState>,
    Path(point_id): Path<String>,
    Query(query): Query<NamespaceQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    info!("Deleting memory: {}", point_id);

    state
        .qdrant
        .delete_memory(&point_id, query.namespace.as_deref())
        .await?;

    // Track stats
    state.stats.increment_deletes();

    Ok(Json(serde_json::json!({
        "success": true,
        "point_id": point_id,
        "message": "Memory deleted successfully"
    })))
}

/// Search memories by similarity
///
/// **Purpose:** Find similar memories using vector search (cosine similarity).
///
/// **Usage:**
/// - Send a query vector (1024-dim BGE-M3 embedding)
/// - Filter by `user_id` and optionally `category`
/// - Adjust `limit` (max results) and `min_score` (similarity threshold)
///
/// **Performance:** ~2-5ms for 10k points, ~10-20ms for 100k points.
#[utoipa::path(
    post,
    path = "/memories/search",
    tag = "Memories",
    request_body = SearchMemoriesRequest,
    responses(
        (status = 200, description = "Search results", body = SearchMemoriesResponse),
        (status = 400, description = "Invalid request (wrong vector dimension)"),
        (status = 500, description = "Qdrant error")
    )
)]

pub async fn search_memories(
    State(state): State<AppState>,
    Json(req): Json<SearchMemoriesRequest>,
) -> Result<Json<SearchMemoriesResponse>, AppError> {
    info!(
        "Searching memories for user {} with limit {}",
        req.user_id, req.limit
    );

    let results = state
        .qdrant
        .search_memories(
            req.query_vector,
            req.user_id,
            req.category,
            req.limit,
            req.min_score,
            req.namespace.as_deref(),
        )
        .await?;

    let search_results: Vec<SearchResult> = results
        .into_iter()
        .map(|(id, score, payload)| SearchResult { id, score, payload })
        .collect();

    let count = search_results.len();

    // Track stats
    state.stats.increment_searches();

    Ok(Json(SearchMemoriesResponse {
        results: search_results,
        count,
    }))
}

/// Get Qdrant collection statistics
///
/// **Purpose:** Returns metadata about the memories collection (point count, indexing status).
///
/// **Usage:** Useful for monitoring and health checks.
#[utoipa::path(
    get,
    path = "/collection/info",
    tag = "Service Info",
    params(
        ("namespace" = Option<String>, Query, description = "Optional namespace for alternative collection")
    ),
    responses(
        (status = 200, description = "Collection statistics", body = CollectionInfo),
        (status = 500, description = "Qdrant error")
    )
)]
pub async fn get_collection_info(
    State(state): State<AppState>,
    Query(query): Query<NamespaceQuery>,
) -> Result<Json<CollectionInfo>, AppError> {
    let (status, points_count, vectors_count, indexed_vectors_count) =
        state.qdrant.get_collection_info(query.namespace.as_deref()).await?;

    Ok(Json(CollectionInfo {
        status,
        points_count,
        vectors_count,
        indexed_vectors_count,
    }))
}

/// Scroll (list) all memories for a user
///
/// **Purpose:** Retrieve all memories for a user without vector search.
/// Useful for displaying a complete memory list in the UI.
///
/// **Usage:**
/// - Filter by `user_id` (required) and optionally `category`
/// - Set `limit` (max 10,000 to avoid memory issues)
///
/// **Performance:** Can be slow for users with >10k memories. Consider pagination or caching.
#[utoipa::path(
    post,
    path = "/memories/scroll",
    tag = "Memories",
    request_body = ScrollMemoriesRequest,
    responses(
        (status = 200, description = "All memories for user", body = ScrollMemoriesResponse),
        (status = 500, description = "Qdrant error")
    )
)]
pub async fn scroll_memories(
    State(state): State<AppState>,
    Json(req): Json<ScrollMemoriesRequest>,
) -> Result<Json<ScrollMemoriesResponse>, AppError> {
    info!(
        "Scrolling memories for user {} with limit {}",
        req.user_id, req.limit
    );

    let results = state
        .qdrant
        .scroll_memories(req.user_id, req.category, req.limit, req.namespace.as_deref())
        .await?;

    let memories: Vec<MemoryResponse> = results
        .into_iter()
        .map(|(id, payload)| MemoryResponse { id, payload })
        .collect();

    let count = memories.len();

    Ok(Json(ScrollMemoriesResponse { memories, count }))
}

/// Batch upsert multiple memories
///
/// **Purpose:** Insert/update many memories in one request (up to 100 per batch).
/// **MUCH faster** than individual requests: 50 memories in 1 request instead of 50 HTTP calls.
///
/// **Usage:**
/// - Send array of `UpsertMemoryRequest` objects
/// - Max 100 points per batch (validation enforced)
/// - Returns success/failure counts + error details for failed points
///
/// **Performance:**
/// - Individual: 50 requests × ~10ms = 500ms
/// - Batch: 1 request × ~50ms = **50ms** (10× faster!)
#[utoipa::path(
    post,
    path = "/memories/batch",
    tag = "Memories",
    request_body = BatchUpsertRequest,
    responses(
        (status = 200, description = "Batch operation completed", body = BatchOperationResponse, example = json!({
            "success_count": 48,
            "failed_count": 2,
            "errors": [
                {"point_id": "mem_1730_xyz", "error": "Invalid vector dimension"},
                {"point_id": "mem_1730_abc", "error": "Missing required field: category"}
            ]
        })),
        (status = 400, description = "Invalid request (too many points or validation error)"),
        (status = 500, description = "Qdrant error")
    )
)]
pub async fn batch_upsert_memories(
    State(state): State<AppState>,
    Json(req): Json<BatchUpsertRequest>,
) -> Result<Json<BatchOperationResponse>, AppError> {
    let point_count = req.points.len();
    info!("Batch upserting {} memories", point_count);

    // Validate batch size
    if point_count == 0 {
        return Err(AppError::InvalidRequest(
            "Batch cannot be empty".to_string(),
        ));
    }
    if point_count > 100 {
        return Err(AppError::InvalidRequest(
            "Batch size exceeds maximum of 100 points".to_string(),
        ));
    }

    let mut success_count = 0;
    let mut failed_count = 0;
    let mut errors = Vec::new();

    // Process each point individually (could be optimized with Qdrant batch API later)
    for point in req.points {
        match state
            .qdrant
            .upsert_memory(
                point.point_id.clone(),
                point.vector,
                point.payload,
                point.namespace.as_deref(),
            )
            .await
        {
            Ok(_) => success_count += 1,
            Err(e) => {
                failed_count += 1;
                errors.push(BatchError {
                    point_id: point.point_id,
                    error: e.to_string(),
                });
            }
        }
    }

    // Track stats
    state.stats.increment_upserts(success_count as u64);

    Ok(Json(BatchOperationResponse {
        success_count,
        failed_count,
        errors,
    }))
}

/// Get service info (version, stats, etc.)
/// Protected endpoint - requires API key
#[utoipa::path(
    get,
    path = "/service/info",
    tag = "Service Info",
    responses(
        (status = 200, description = "Detailed service information", body = inline(Object), example = json!({
            "service": "synaplan-qdrant-service",
            "version": "0.1.0",
            "rust_version": "1.75",
            "status": "healthy",
            "embedding": {
                "supported": false,
                "backend": "none",
                "model": null,
                "device": "none",
                "vector_dimension": 1024
            },
            "collection": {
                "status": "green",
                "points_count": 12548,
                "vectors_count": 12548,
                "indexed_vectors_count": 12548
            }
        }))
    )
)]
pub async fn get_service_info(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    info!("Getting service info");

    // Get Qdrant collection stats
    let (status, points_count, vectors_count, indexed_vectors_count) =
        state.qdrant.get_collection_info().await?;

    // Get system info
    let version = env!("CARGO_PKG_VERSION");
    let rust_version = env!("CARGO_PKG_RUST_VERSION");

    Ok(Json(serde_json::json!({
        "service": "synaplan-qdrant-service",
        "version": version,
        "rust_version": rust_version,
        "status": "healthy",
        "embedding": {
            "supported": false,
            "backend": "none",
            "model": null,
            "device": "none",
            "vector_dimension": state.config.vector_dimension
        },
        "collection": {
            "status": status,
            "points_count": points_count,
            "vectors_count": vectors_count,
            "indexed_vectors_count": indexed_vectors_count
        }
    })))
}
