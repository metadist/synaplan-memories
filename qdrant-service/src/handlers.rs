use crate::error::AppError;
use crate::models::*;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::header,
    response::IntoResponse,
    Json,
};
use tracing::info;

pub async fn get_capabilities(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    // Keep this lightweight and cacheable; do not call external services here.
    let config = state.config.as_ref();

    let supported = state.embedder.is_some();
    let backend = state
        .embedder
        .as_ref()
        .map(|e| e.backend())
        .unwrap_or_else(|| "none".to_string());
    let model = state.embedder.as_ref().and_then(|e| e.model());
    let device = state
        .embedder
        .as_ref()
        .map(|e| e.device())
        .unwrap_or_else(|| "auto".to_string());

    let body = ServiceCapabilities {
        service: "synaplan-qdrant-service".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        vector_dimension: config.vector_dimension,
        embedding: EmbeddingCapabilities {
            supported,
            backend,
            model,
            device,
            vector_dimension: config.vector_dimension,
        },
    };

    Ok((
        [(header::CACHE_CONTROL, "public, max-age=30")],
        Json(body),
    ))
}

pub async fn upsert_memory(
    State(state): State<AppState>,
    Json(req): Json<UpsertMemoryRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    info!("Upserting memory: {}", req.point_id);

    state
        .qdrant
        .upsert_memory(req.point_id.clone(), req.vector, req.payload)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "point_id": req.point_id,
        "message": "Memory upserted successfully"
    })))
}

pub async fn upsert_memory_text(
    State(state): State<AppState>,
    Json(req): Json<UpsertMemoryTextRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let embedder = state.embedder.as_ref().ok_or_else(|| {
        AppError::ServiceUnavailable(
            "Embedding backend is not available (use vector-based routes as fallback)".to_string(),
        )
    })?;

    info!("Upserting memory (text): {}", req.point_id);

    let vector = embedder.embed(&req.text).await?;
    state
        .qdrant
        .upsert_memory(req.point_id.clone(), vector, req.payload)
        .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "point_id": req.point_id,
        "message": "Memory upserted successfully (text)"
    })))
}

pub async fn get_memory(
    State(state): State<AppState>,
    Path(point_id): Path<String>,
) -> Result<Json<MemoryResponse>, AppError> {
    info!("Getting memory: {}", point_id);

    let payload = state
        .qdrant
        .get_memory(&point_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Memory not found: {}", point_id)))?;

    Ok(Json(MemoryResponse {
        id: point_id,
        payload,
    }))
}

pub async fn delete_memory(
    State(state): State<AppState>,
    Path(point_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    info!("Deleting memory: {}", point_id);

    state.qdrant.delete_memory(&point_id).await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "point_id": point_id,
        "message": "Memory deleted successfully"
    })))
}

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
        )
        .await?;

    let search_results: Vec<SearchResult> = results
        .into_iter()
        .map(|(id, score, payload)| SearchResult { id, score, payload })
        .collect();

    let count = search_results.len();

    Ok(Json(SearchMemoriesResponse {
        results: search_results,
        count,
    }))
}

pub async fn search_memories_text(
    State(state): State<AppState>,
    Json(req): Json<SearchMemoriesTextRequest>,
) -> Result<Json<SearchMemoriesResponse>, AppError> {
    let embedder = state.embedder.as_ref().ok_or_else(|| {
        AppError::ServiceUnavailable(
            "Embedding backend is not available (use vector-based routes as fallback)".to_string(),
        )
    })?;

    info!(
        "Searching memories (text) for user {} with limit {}",
        req.user_id, req.limit
    );

    let query_vector = embedder.embed(&req.query_text).await?;
    let results = state
        .qdrant
        .search_memories(query_vector, req.user_id, req.category, req.limit, req.min_score)
        .await?;

    let search_results: Vec<SearchResult> = results
        .into_iter()
        .map(|(id, score, payload)| SearchResult { id, score, payload })
        .collect();

    let count = search_results.len();

    Ok(Json(SearchMemoriesResponse {
        results: search_results,
        count,
    }))
}

pub async fn get_collection_info(
    State(state): State<AppState>,
) -> Result<Json<CollectionInfo>, AppError> {
    let (status, points_count, vectors_count, indexed_vectors_count) =
        state.qdrant.get_collection_info().await?;

    Ok(Json(CollectionInfo {
        status,
        points_count,
        vectors_count,
        indexed_vectors_count,
    }))
}

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
        .scroll_memories(req.user_id, req.category, req.limit)
        .await?;

    let memories: Vec<MemoryResponse> = results
        .into_iter()
        .map(|(id, payload)| MemoryResponse { id, payload })
        .collect();

    let count = memories.len();

    Ok(Json(ScrollMemoriesResponse { memories, count }))
}

/// Get service info (version, stats, etc.)
/// Protected endpoint - requires API key
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
    let embedding_supported = state.embedder.is_some();
    let embedding_backend = state
        .embedder
        .as_ref()
        .map(|e| e.backend())
        .unwrap_or_else(|| "none".to_string());
    let embedding_model = state.embedder.as_ref().and_then(|e| e.model());
    let embedding_device = state
        .embedder
        .as_ref()
        .map(|e| e.device())
        .unwrap_or_else(|| "auto".to_string());

    Ok(Json(serde_json::json!({
        "service": "synaplan-qdrant-service",
        "version": version,
        "rust_version": rust_version,
        "status": "healthy",
        "embedding": {
            "supported": embedding_supported,
            "backend": embedding_backend,
            "model": embedding_model,
            "device": embedding_device,
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

