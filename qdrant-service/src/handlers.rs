use crate::error::AppError;
use crate::models::*;
use crate::AppState;
use axum::{
    extract::{Path, State},
    Json,
};
use tracing::info;

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

