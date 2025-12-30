use dns_proxy::metrics::{Metrics, Timer};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_metrics_new() {
    let metrics = Metrics::new();
    assert_eq!(
        metrics
            .total_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        metrics
            .successful_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        metrics
            .failed_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        metrics
            .bytes_received
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        metrics
            .bytes_sent
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        metrics
            .sni_rewrites
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        metrics
            .upstream_errors
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn test_metrics_default() {
    let metrics = Metrics::default();
    assert_eq!(
        metrics
            .total_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn test_metrics_record_request_success() {
    let metrics = Metrics::new();
    metrics.record_request(true, 100, 200, Duration::from_millis(50));

    assert_eq!(
        metrics
            .total_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        metrics
            .successful_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        metrics
            .failed_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        metrics
            .bytes_received
            .load(std::sync::atomic::Ordering::Relaxed),
        100
    );
    assert_eq!(
        metrics
            .bytes_sent
            .load(std::sync::atomic::Ordering::Relaxed),
        200
    );
}

#[test]
fn test_metrics_record_request_failure() {
    let metrics = Metrics::new();
    metrics.record_request(false, 50, 0, Duration::from_millis(10));

    assert_eq!(
        metrics
            .total_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        metrics
            .successful_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert_eq!(
        metrics
            .failed_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        metrics
            .bytes_received
            .load(std::sync::atomic::Ordering::Relaxed),
        50
    );
    assert_eq!(
        metrics
            .bytes_sent
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn test_metrics_record_sni_rewrite() {
    let metrics = Metrics::new();
    metrics.record_sni_rewrite();
    metrics.record_sni_rewrite();

    assert_eq!(
        metrics
            .sni_rewrites
            .load(std::sync::atomic::Ordering::Relaxed),
        2
    );
}

#[test]
fn test_metrics_record_upstream_error() {
    let metrics = Metrics::new();
    metrics.record_upstream_error();
    metrics.record_upstream_error();
    metrics.record_upstream_error();

    assert_eq!(
        metrics
            .upstream_errors
            .load(std::sync::atomic::Ordering::Relaxed),
        3
    );
}

#[test]
fn test_metrics_snapshot_empty() {
    let metrics = Metrics::new();
    let snapshot = metrics.snapshot();

    assert_eq!(snapshot.total_requests, 0);
    assert_eq!(snapshot.successful_requests, 0);
    assert_eq!(snapshot.failed_requests, 0);
    assert_eq!(snapshot.bytes_received, 0);
    assert_eq!(snapshot.bytes_sent, 0);
    assert_eq!(snapshot.sni_rewrites, 0);
    assert_eq!(snapshot.upstream_errors, 0);
    assert_eq!(snapshot.average_processing_time_ms, 0.0);
    assert_eq!(snapshot.success_rate, 0.0);
}

#[test]
fn test_metrics_snapshot_with_data() {
    let metrics = Metrics::new();

    // Record some requests
    metrics.record_request(true, 100, 200, Duration::from_millis(50));
    metrics.record_request(true, 150, 250, Duration::from_millis(30));
    metrics.record_request(false, 50, 0, Duration::from_millis(10));
    metrics.record_sni_rewrite();
    metrics.record_upstream_error();

    let snapshot = metrics.snapshot();

    assert_eq!(snapshot.total_requests, 3);
    assert_eq!(snapshot.successful_requests, 2);
    assert_eq!(snapshot.failed_requests, 1);
    assert_eq!(snapshot.bytes_received, 300);
    assert_eq!(snapshot.bytes_sent, 450);
    assert_eq!(snapshot.sni_rewrites, 1);
    assert_eq!(snapshot.upstream_errors, 1);

    // Check success rate (2/3 * 100 = 66.67%)
    assert!((snapshot.success_rate - 66.66666666666666).abs() < 0.01);

    // Check average processing time (90ms total / 3 = 30ms)
    assert!(
        snapshot.average_processing_time_ms > 29.0 && snapshot.average_processing_time_ms < 31.0
    );
}

#[test]
fn test_metrics_concurrent_updates() {
    use std::thread;

    let metrics = Arc::new(Metrics::new());
    let mut handles = vec![];

    // Spawn multiple threads to update metrics concurrently
    for _ in 0..10 {
        let metrics_clone = Arc::clone(&metrics);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                metrics_clone.record_request(true, 10, 20, Duration::from_millis(1));
                metrics_clone.record_sni_rewrite();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.total_requests, 1000);
    assert_eq!(snapshot.successful_requests, 1000);
    assert_eq!(snapshot.sni_rewrites, 1000);
    assert_eq!(snapshot.bytes_received, 10000);
    assert_eq!(snapshot.bytes_sent, 20000);
}

#[test]
fn test_timer_start() {
    let timer = Timer::start();
    assert!(timer.elapsed() < Duration::from_secs(1));
}

#[test]
fn test_timer_elapsed() {
    let timer = Timer::start();
    std::thread::sleep(Duration::from_millis(10));
    let elapsed = timer.elapsed();
    assert!(elapsed >= Duration::from_millis(10));
    assert!(elapsed < Duration::from_secs(1));
}

#[test]
fn test_timer_multiple_calls() {
    let timer = Timer::start();
    std::thread::sleep(Duration::from_millis(5));
    let elapsed1 = timer.elapsed();
    std::thread::sleep(Duration::from_millis(5));
    let elapsed2 = timer.elapsed();

    assert!(elapsed2 >= elapsed1);
    assert!(elapsed2 >= Duration::from_millis(10));
}
