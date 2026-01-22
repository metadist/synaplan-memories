//! Daily statistics tracking for vector operations
//!
//! Tracks:
//! - Total upserts (single + batch)
//! - Total searches
//! - Total deletes
//! - Total vectors stored
//! - Uptime

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Thread-safe statistics tracker
#[derive(Clone)]
pub struct StatsTracker {
    pub upserts: Arc<AtomicU64>,
    pub searches: Arc<AtomicU64>,
    pub deletes: Arc<AtomicU64>,
    pub start_time: Instant,
}

impl StatsTracker {
    pub fn new() -> Self {
        Self {
            upserts: Arc::new(AtomicU64::new(0)),
            searches: Arc::new(AtomicU64::new(0)),
            deletes: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    #[inline]
    pub fn increment_upserts(&self, count: u64) {
        self.upserts.fetch_add(count, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_searches(&self) {
        self.searches.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_deletes(&self) {
        self.deletes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            upserts: self.upserts.load(Ordering::Relaxed),
            searches: self.searches.load(Ordering::Relaxed),
            deletes: self.deletes.load(Ordering::Relaxed),
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }

    pub fn reset(&self) {
        self.upserts.store(0, Ordering::Relaxed);
        self.searches.store(0, Ordering::Relaxed);
        self.deletes.store(0, Ordering::Relaxed);
    }
}

impl Default for StatsTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct StatsSnapshot {
    pub upserts: u64,
    pub searches: u64,
    pub deletes: u64,
    pub uptime_seconds: u64,
}

impl StatsSnapshot {
    pub fn format_uptime(&self) -> String {
        let days = self.uptime_seconds / 86400;
        let hours = (self.uptime_seconds % 86400) / 3600;
        let minutes = (self.uptime_seconds % 3600) / 60;

        if days > 0 {
            format!("{}d {}h {}m", days, hours, minutes)
        } else if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}m", minutes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_tracking() {
        let stats = StatsTracker::new();

        stats.increment_upserts(5);
        stats.increment_searches();
        stats.increment_searches();
        stats.increment_deletes();

        let snapshot = stats.get_snapshot();
        assert_eq!(snapshot.upserts, 5);
        assert_eq!(snapshot.searches, 2);
        assert_eq!(snapshot.deletes, 1);

        stats.reset();
        let snapshot = stats.get_snapshot();
        assert_eq!(snapshot.upserts, 0);
        assert_eq!(snapshot.searches, 0);
        assert_eq!(snapshot.deletes, 0);
    }

    #[test]
    fn test_uptime_formatting() {
        let snapshot = StatsSnapshot {
            upserts: 0,
            searches: 0,
            deletes: 0,
            uptime_seconds: 90061, // 1d 1h 1m 1s
        };
        assert_eq!(snapshot.format_uptime(), "1d 1h 1m");

        let snapshot = StatsSnapshot {
            upserts: 0,
            searches: 0,
            deletes: 0,
            uptime_seconds: 3661, // 1h 1m 1s
        };
        assert_eq!(snapshot.format_uptime(), "1h 1m");

        let snapshot = StatsSnapshot {
            upserts: 0,
            searches: 0,
            deletes: 0,
            uptime_seconds: 61, // 1m 1s
        };
        assert_eq!(snapshot.format_uptime(), "1m");
    }
}

