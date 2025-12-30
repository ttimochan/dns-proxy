use std::sync::Arc;
/// Performance metrics and monitoring
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Metrics collector for DNS proxy performance
#[derive(Clone, Debug)]
pub struct Metrics {
    /// Total number of requests processed
    pub total_requests: Arc<AtomicU64>,
    /// Total number of successful requests
    pub successful_requests: Arc<AtomicU64>,
    /// Total number of failed requests
    pub failed_requests: Arc<AtomicU64>,
    /// Total number of bytes received
    pub bytes_received: Arc<AtomicU64>,
    /// Total number of bytes sent
    pub bytes_sent: Arc<AtomicU64>,
    /// Total processing time in microseconds
    pub total_processing_time_us: Arc<AtomicU64>,
    /// Number of SNI rewrites performed
    pub sni_rewrites: Arc<AtomicU64>,
    /// Number of upstream errors
    pub upstream_errors: Arc<AtomicU64>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            total_requests: Arc::new(AtomicU64::new(0)),
            successful_requests: Arc::new(AtomicU64::new(0)),
            failed_requests: Arc::new(AtomicU64::new(0)),
            bytes_received: Arc::new(AtomicU64::new(0)),
            bytes_sent: Arc::new(AtomicU64::new(0)),
            total_processing_time_us: Arc::new(AtomicU64::new(0)),
            sni_rewrites: Arc::new(AtomicU64::new(0)),
            upstream_errors: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record a request
    pub fn record_request(
        &self,
        success: bool,
        bytes_received: u64,
        bytes_sent: u64,
        duration: Duration,
    ) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successful_requests.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }
        self.bytes_received
            .fetch_add(bytes_received, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes_sent, Ordering::Relaxed);
        self.total_processing_time_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }

    /// Record an SNI rewrite
    pub fn record_sni_rewrite(&self) {
        self.sni_rewrites.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an upstream error
    pub fn record_upstream_error(&self) {
        self.upstream_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current metrics snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        let total = self.total_requests.load(Ordering::Relaxed);
        let successful = self.successful_requests.load(Ordering::Relaxed);
        let failed = self.failed_requests.load(Ordering::Relaxed);
        let total_time_us = self.total_processing_time_us.load(Ordering::Relaxed);

        MetricsSnapshot {
            total_requests: total,
            successful_requests: successful,
            failed_requests: failed,
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            sni_rewrites: self.sni_rewrites.load(Ordering::Relaxed),
            upstream_errors: self.upstream_errors.load(Ordering::Relaxed),
            average_processing_time_ms: if total > 0 {
                (total_time_us as f64 / total as f64) / 1000.0
            } else {
                0.0
            },
            success_rate: if total > 0 {
                (successful as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        }
    }
}

/// Snapshot of current metrics
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub sni_rewrites: u64,
    pub upstream_errors: u64,
    pub average_processing_time_ms: f64,
    pub success_rate: f64,
}

impl MetricsSnapshot {
    // Snapshot is used for JSON serialization in healthcheck endpoint
}

/// Helper for timing operations
pub struct Timer {
    start: Instant,
}

impl Timer {
    /// Start a new timer
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}
