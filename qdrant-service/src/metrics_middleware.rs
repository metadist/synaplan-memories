use crate::metrics::MetricsState;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::time::Instant;

/// Middleware to track request metrics
pub async fn metrics_middleware(
    State(metrics): State<MetricsState>,
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();

    // Increment request counter
    metrics.increment_requests();

    // Process the request
    let response = next.run(request).await;

    // Record duration
    let duration = start.elapsed().as_secs_f64();
    metrics.record_request_duration(duration);

    // Track failures (status code >= 400)
    if response.status().as_u16() >= 400 {
        metrics.increment_failures();
    }

    response
}

