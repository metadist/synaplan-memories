use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthState {
    pub api_key: Option<String>,
}

pub async fn auth_middleware(
    State(auth_state): State<Arc<AuthState>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // If no API key is configured, skip auth
    let Some(expected_key) = &auth_state.api_key else {
        return Ok(next.run(request).await);
    };

    // Check Authorization header
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    // Also check X-API-Key header (alternative)
    let api_key_header = headers
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok());

    let provided_key = auth_header.or(api_key_header);

    match provided_key {
        Some(key) if key == expected_key => Ok(next.run(request).await),
        _ => {
            let body = Json(json!({
                "error": "Unauthorized: Invalid or missing API key",
                "status": 401
            }));
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_creation() {
        let state = AuthState {
            api_key: Some("test-key".to_string()),
        };
        assert!(state.api_key.is_some());
    }
}

