use prometheus::{Histogram, HistogramOpts, IntCounter, Opts, Registry};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Metrics collector for DNS proxy performance using Prometheus
#[derive(Clone)]
pub struct Metrics {
    registry: Arc<Registry>,

    // Prometheus metrics
    total_requests: IntCounter,
    successful_requests: IntCounter,
    failed_requests: IntCounter,
    bytes_received: IntCounter,
    bytes_sent: IntCounter,
    sni_rewrites: IntCounter,
    upstream_errors: IntCounter,
    processing_time: Histogram,

    // Cached snapshot to avoid repeated reads
    cached_snapshot: Arc<RwLock<Option<CachedSnapshot>>>,
}

/// Cached snapshot with timestamp
#[derive(Clone, Debug)]
struct CachedSnapshot {
    snapshot: MetricsSnapshot,
    timestamp: Instant,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    /// Create a new metrics collector with Prometheus registry
    pub fn new() -> Self {
        let registry = Registry::new();

        let total_requests = IntCounter::with_opts(Opts::new(
            "dns_proxy_requests_total",
            "Total number of DNS requests",
        ))
        .expect("Failed to create total_requests metric");

        let successful_requests = IntCounter::with_opts(Opts::new(
            "dns_proxy_requests_success",
            "Number of successful DNS requests",
        ))
        .expect("Failed to create successful_requests metric");

        let failed_requests = IntCounter::with_opts(Opts::new(
            "dns_proxy_requests_failed",
            "Number of failed DNS requests",
        ))
        .expect("Failed to create failed_requests metric");

        let bytes_received = IntCounter::with_opts(Opts::new(
            "dns_proxy_bytes_received_total",
            "Total bytes received",
        ))
        .expect("Failed to create bytes_received metric");

        let bytes_sent =
            IntCounter::with_opts(Opts::new("dns_proxy_bytes_sent_total", "Total bytes sent"))
                .expect("Failed to create bytes_sent metric");

        let sni_rewrites = IntCounter::with_opts(Opts::new(
            "dns_proxy_sni_rewrites_total",
            "Total number of SNI rewrites",
        ))
        .expect("Failed to create sni_rewrites metric");

        let upstream_errors = IntCounter::with_opts(Opts::new(
            "dns_proxy_upstream_errors_total",
            "Total number of upstream errors",
        ))
        .expect("Failed to create upstream_errors metric");

        let processing_time = Histogram::with_opts(
            HistogramOpts::new(
                "dns_proxy_processing_time_seconds",
                "DNS request processing time in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
        )
        .expect("Failed to create processing_time metric");

        // Register all metrics - use expect for better error messages
        registry
            .register(Box::new(total_requests.clone()))
            .expect("Failed to register total_requests metric");
        registry
            .register(Box::new(successful_requests.clone()))
            .expect("Failed to register successful_requests metric");
        registry
            .register(Box::new(failed_requests.clone()))
            .expect("Failed to register failed_requests metric");
        registry
            .register(Box::new(bytes_received.clone()))
            .expect("Failed to register bytes_received metric");
        registry
            .register(Box::new(bytes_sent.clone()))
            .expect("Failed to register bytes_sent metric");
        registry
            .register(Box::new(sni_rewrites.clone()))
            .expect("Failed to register sni_rewrites metric");
        registry
            .register(Box::new(upstream_errors.clone()))
            .expect("Failed to register upstream_errors metric");
        registry
            .register(Box::new(processing_time.clone()))
            .expect("Failed to register processing_time metric");

        Self {
            registry: Arc::new(registry),
            total_requests,
            successful_requests,
            failed_requests,
            bytes_received,
            bytes_sent,
            sni_rewrites,
            upstream_errors,
            processing_time,
            cached_snapshot: Arc::new(RwLock::new(None)),
        }
    }

    /// Record a request with all metrics in a single batch update
    /// This is more efficient than multiple separate updates
    pub fn record_request(
        &self,
        success: bool,
        bytes_received_val: u64,
        bytes_sent_val: u64,
        duration: Duration,
    ) {
        self.total_requests.inc();
        if success {
            self.successful_requests.inc();
        } else {
            self.failed_requests.inc();
        }
        self.bytes_received.inc_by(bytes_received_val);
        self.bytes_sent.inc_by(bytes_sent_val);
        self.processing_time.observe(duration.as_secs_f64());
    }

    /// Record an SNI rewrite
    pub fn record_sni_rewrite(&self) {
        self.sni_rewrites.inc();
    }

    /// Record an upstream error
    pub fn record_upstream_error(&self) {
        self.upstream_errors.inc();
    }

    /// Export metrics in Prometheus text format
    pub fn export_prometheus(&self) -> String {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .expect("Failed to encode Prometheus metrics");
        String::from_utf8(buffer).expect("Prometheus output is not valid UTF-8")
    }

    /// Get current metrics snapshot with caching
    /// The snapshot is cached for 1 second to reduce lock contention
    pub async fn snapshot(&self) -> MetricsSnapshot {
        // Check if we have a valid cached snapshot
        let cache = self.cached_snapshot.read().await;
        if let Some(cached) = cache.as_ref() {
            // Return cached snapshot if less than 1 second old
            if cached.timestamp.elapsed() < Duration::from_secs(1) {
                return cached.snapshot.clone();
            }
        }
        drop(cache);

        // Generate new snapshot from Prometheus metrics
        let snapshot = self.generate_snapshot();

        // Update cache
        let mut cache = self.cached_snapshot.write().await;
        *cache = Some(CachedSnapshot {
            snapshot: snapshot.clone(),
            timestamp: Instant::now(),
        });

        snapshot
    }

    /// Generate a snapshot from Prometheus metrics
    fn generate_snapshot(&self) -> MetricsSnapshot {
        let total = self.total_requests.get();
        let successful = self.successful_requests.get();
        let failed = self.failed_requests.get();

        // Calculate derived metrics
        let success_rate = if total > 0 {
            (successful as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Get average processing time from histogram
        let processing_time_sum = self.processing_time.get_sample_sum();
        let processing_time_count = self.processing_time.get_sample_count();
        let avg_latency_ms = if processing_time_count > 0 {
            (processing_time_sum / processing_time_count as f64) * 1000.0
        } else {
            0.0
        };

        MetricsSnapshot {
            total_requests: total,
            successful_requests: successful,
            failed_requests: failed,
            bytes_received: self.bytes_received.get(),
            bytes_sent: self.bytes_sent.get(),
            sni_rewrites: self.sni_rewrites.get(),
            upstream_errors: self.upstream_errors.get(),
            average_processing_time_ms: avg_latency_ms,
            success_rate,
            throughput_requests_per_sec: total as f64,
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
    /// Estimated requests per second
    pub throughput_requests_per_sec: f64,
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
