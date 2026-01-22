//! Request ID middleware for distributed tracing
//!
//! **Purpose:**
//! - Assigns unique ID to each request (`X-Request-ID` header)
//! - Propagates ID through logs for debugging across services
//! - Enables tracing: Backend → Microservice → Qdrant
//!
//! **Usage:**
//! - Client can send `X-Request-ID` (we'll use it)
//! - If not present, we generate a new UUIDv4
//! - All logs include the request ID
//! - Response includes `X-Request-ID` header
//!
//! **Example:**
//! ```
//! Backend sends: POST /memories/search
//!   X-Request-ID: abc-123
//!
//! Microservice logs:
//!   [INFO] [abc-123] Searching memories for user 1730
//!   [INFO] [abc-123] Qdrant query took 3ms
//!
//! Response includes:
//!   X-Request-ID: abc-123
//! ```

use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use tracing::Span;
use uuid::Uuid;

/// Header name for request ID
static REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Request ID middleware
///
/// Extracts or generates a request ID and adds it to:
/// 1. Response headers
/// 2. Tracing span (for logs)
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    // Extract existing request ID or generate new one
    let request_id = request
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Add request ID to tracing span
    let span = Span::current();
    span.record("request_id", &request_id.as_str());

    // Store request ID in extensions for potential use in handlers
    request.extensions_mut().insert(request_id.clone());

    // Call next middleware/handler
    let mut response = next.run(request).await;

    // Add request ID to response headers
    if let Ok(header_value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER.clone(), header_value);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_request_id_generated_if_missing() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(request_id_middleware));

        let response = app
            .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let request_id = response.headers().get("x-request-id");
        assert!(request_id.is_some(), "Response should include x-request-id");

        let id_str = request_id.unwrap().to_str().unwrap();
        assert!(Uuid::parse_str(id_str).is_ok(), "Should be valid UUID");
    }

    #[tokio::test]
    async fn test_request_id_propagated_from_client() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(request_id_middleware));

        let client_id = "client-request-123";
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header("x-request-id", client_id)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let request_id = response.headers().get("x-request-id").unwrap();
        assert_eq!(request_id.to_str().unwrap(), client_id);
    }
}

