use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Global metrics state
#[derive(Clone)]
pub struct MetricsState {
    start_time: Arc<Instant>,
    pub requests_total: Arc<AtomicU64>,
    pub requests_failed: Arc<AtomicU64>,
}

impl MetricsState {
    pub fn new() -> Self {
        // Register metrics with descriptions
        describe_counter!("requests_total", "Total number of requests received");
        describe_counter!("requests_failed", "Total number of failed requests");
        describe_histogram!("request_duration_seconds", "Request duration in seconds");
        describe_gauge!("uptime_seconds", "Service uptime in seconds");
        describe_gauge!("qdrant_points_total", "Total number of points in Qdrant");
        describe_gauge!("qdrant_vectors_total", "Total number of vectors in Qdrant");

        Self {
            start_time: Arc::new(Instant::now()),
            requests_total: Arc::new(AtomicU64::new(0)),
            requests_failed: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn increment_requests(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        counter!("requests_total").increment(1);
    }

    pub fn increment_failures(&self) {
        self.requests_failed.fetch_add(1, Ordering::Relaxed);
        counter!("requests_failed").increment(1);
    }

    pub fn record_request_duration(&self, duration: f64) {
        histogram!("request_duration_seconds").record(duration);
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub fn get_requests_total(&self) -> u64 {
        self.requests_total.load(Ordering::Relaxed)
    }

    pub fn get_requests_failed(&self) -> u64 {
        self.requests_failed.load(Ordering::Relaxed)
    }

    pub fn update_qdrant_stats(&self, points_count: u64, vectors_count: u64) {
        gauge!("qdrant_points_total").set(points_count as f64);
        gauge!("qdrant_vectors_total").set(vectors_count as f64);
        gauge!("uptime_seconds").set(self.uptime_seconds() as f64);
    }
}

impl Default for MetricsState {
    fn default() -> Self {
        Self::new()
    }
}

