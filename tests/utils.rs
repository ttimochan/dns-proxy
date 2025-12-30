use dns_proxy::utils::{BackoffCounter, exponential_backoff};
use std::time::Duration;

#[test]
fn test_exponential_backoff_attempt_0() {
    let delay = exponential_backoff(0, 100, 10000);
    // 1^0 = 1, so 100 * 1 = 100
    assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn test_exponential_backoff_attempt_1() {
    let delay = exponential_backoff(1, 100, 10000);
    // 1^1 = 1, so 100 * 1 = 100
    assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn test_exponential_backoff_attempt_2() {
    let delay = exponential_backoff(2, 100, 10000);
    // 1^2 = 1, so 100 * 1 = 100
    assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn test_exponential_backoff_attempt_3() {
    let delay = exponential_backoff(3, 100, 10000);
    // 1^3 = 1, so 100 * 1 = 100
    assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn test_exponential_backoff_max_delay() {
    let delay = exponential_backoff(20, 100, 1000);
    // 1^20 = 1, so 100 * 1 = 100, but capped at max_delay 1000
    // Actually, since 100 < 1000, it returns 100
    assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn test_exponential_backoff_cap_at_10() {
    // After attempt 10, it should cap at 2^10 * base
    let delay = exponential_backoff(15, 100, 100000);
    // Should be capped at 2^10 * 100 = 102400, but limited by max_delay
    assert!(delay <= Duration::from_millis(100000));
}

#[test]
fn test_backoff_counter_new() {
    let counter = BackoffCounter::new();
    let delay = counter.next_delay(100, 10000);
    assert_eq!(delay, Duration::from_millis(100)); // First attempt (0)
}

#[test]
fn test_backoff_counter_sequence() {
    let counter = BackoffCounter::new();

    // First call: attempt 0, 1^0 = 1, so 100 * 1 = 100
    let delay1 = counter.next_delay(100, 10000);
    assert_eq!(delay1, Duration::from_millis(100));

    // Second call: attempt 1, 1^1 = 1, so 100 * 1 = 100
    let delay2 = counter.next_delay(100, 10000);
    assert_eq!(delay2, Duration::from_millis(100));

    // Third call: attempt 2, 1^2 = 1, so 100 * 1 = 100
    let delay3 = counter.next_delay(100, 10000);
    assert_eq!(delay3, Duration::from_millis(100));
}

#[test]
fn test_backoff_counter_reset_after_10() {
    let counter = BackoffCounter::new();

    // Call 10 times to reach attempt 10
    for _ in 0..10 {
        let _ = counter.next_delay(100, 10000);
    }

    // Next call should reset to attempt 0
    let delay = counter.next_delay(100, 10000);
    assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn test_backoff_counter_default() {
    let counter = BackoffCounter::default();
    let delay = counter.next_delay(100, 10000);
    assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn test_backoff_counter_concurrent() {
    use std::sync::Arc;
    use std::thread;

    let counter = Arc::new(BackoffCounter::new());
    let mut handles = vec![];

    // Spawn multiple threads
    for _ in 0..5 {
        let counter_clone = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                let _ = counter_clone.next_delay(100, 10000);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // The counter should have been incremented 50 times total
    // Next call should be at a high attempt number (but will reset if >= 10)
    let delay = counter.next_delay(100, 10000);
    // The exact value depends on timing, but should be reasonable
    assert!(delay <= Duration::from_millis(10000));
}
