use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

/// Authentication state holding optional API key
#[derive(Clone)]
pub struct AuthState {
    pub api_key: Option<String>,
}

impl AuthState {
    /// Create new auth state with optional API key
    #[inline]
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }

    /// Check if authentication is enabled
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.api_key.is_some()
    }
}

/// Authentication middleware
/// Supports both "Authorization: Bearer TOKEN" and "X-API-Key: TOKEN" headers
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

    // Check Authorization header (Bearer token)
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    // Check X-API-Key header (alternative)
    let api_key_header = headers.get("X-API-Key").and_then(|h| h.to_str().ok());

    let provided_key = auth_header.or(api_key_header);

    match provided_key {
        Some(key) if key == expected_key => Ok(next.run(request).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_creation() {
        let state = AuthState::new(Some("test-key".to_string()));
        assert!(state.api_key.is_some());
        assert_eq!(state.api_key.unwrap(), "test-key");
    }

    #[test]
    fn test_auth_state_disabled() {
        let state = AuthState::new(None);
        assert!(state.api_key.is_none());
    }

    #[test]
    fn test_auth_state_is_enabled() {
        let enabled = AuthState::new(Some("key".to_string()));
        assert!(enabled.is_enabled());

        let disabled = AuthState::new(None);
        assert!(!disabled.is_enabled());
    }

    #[test]
    fn test_auth_state_clone() {
        let state1 = AuthState::new(Some("key".to_string()));
        let state2 = state1.clone();
        assert_eq!(state1.api_key, state2.api_key);
    }
}

