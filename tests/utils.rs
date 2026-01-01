use dns_proxy::utils::{BackoffCounter, exponential_backoff};
use std::time::Duration;

#[test]
fn test_exponential_backoff_attempt_0() {
    let delay = exponential_backoff(0, 100, 10000);
    // 2^0 = 1, so 100 * 1 = 100ms
    assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn test_exponential_backoff_attempt_1() {
    let delay = exponential_backoff(1, 100, 10000);
    // 2^1 = 2, so 100 * 2 = 200ms
    assert_eq!(delay, Duration::from_millis(200));
}

#[test]
fn test_exponential_backoff_attempt_2() {
    let delay = exponential_backoff(2, 100, 10000);
    // 2^2 = 4, so 100 * 4 = 400ms
    assert_eq!(delay, Duration::from_millis(400));
}

#[test]
fn test_exponential_backoff_attempt_3() {
    let delay = exponential_backoff(3, 100, 10000);
    // 2^3 = 8, so 100 * 8 = 800ms
    assert_eq!(delay, Duration::from_millis(800));
}

#[test]
fn test_exponential_backoff_max_delay() {
    let delay = exponential_backoff(20, 100, 1000);
    // 2^20 * 100 = 104857600ms, but capped at max_delay 1000ms
    assert_eq!(delay, Duration::from_millis(1000));
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

    // First call: attempt 0, 2^0 = 1, so 100 * 1 = 100ms
    let delay1 = counter.next_delay(100, 10000);
    assert_eq!(delay1, Duration::from_millis(100));

    // Second call: attempt 1, 2^1 = 2, so 100 * 2 = 200ms
    let delay2 = counter.next_delay(100, 10000);
    assert_eq!(delay2, Duration::from_millis(200));

    // Third call: attempt 2, 2^2 = 4, so 100 * 4 = 400ms
    let delay3 = counter.next_delay(100, 10000);
    assert_eq!(delay3, Duration::from_millis(400));
}

#[test]
fn test_backoff_counter_reset_after_10() {
    let counter = BackoffCounter::new();

    // Call 10 times (attempt 0-9)
    for i in 0..10 {
        let delay = counter.next_delay(100, 10000);
        let expected = 100u64 * 2u64.pow(i);
        let capped = expected.min(10000);
        assert_eq!(
            delay,
            Duration::from_millis(capped),
            "Failed at iteration {}",
            i
        );
    }

    // 11th call: attempt 10 >= 10, so it triggers reset after getting the delay
    // The delay for attempt 10 should be 2^10 * 100 = 102400ms, capped to 10000ms
    let delay = counter.next_delay(100, 10000);
    assert_eq!(
        delay,
        Duration::from_millis(10000),
        "11th call should return attempt 10 delay"
    );

    // 12th call: reset was triggered, so this is attempt 0
    let delay = counter.next_delay(100, 10000);
    assert_eq!(
        delay,
        Duration::from_millis(100),
        "12th call should be attempt 0 after reset"
    );
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
