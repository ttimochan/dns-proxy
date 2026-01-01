/// Utility functions for error handling and retry logic
use std::sync::atomic::{AtomicU32, Ordering};

/// Exponential backoff retry helper
/// Returns the delay duration for the given attempt number (0-indexed)
pub fn exponential_backoff(
    attempt: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
) -> std::time::Duration {
    let delay_ms = base_delay_ms
        .saturating_mul(2u64.saturating_pow(attempt.min(10))) // 2^attempt with cap at 2^10
        .min(max_delay_ms);
    std::time::Duration::from_millis(delay_ms)
}

/// Thread-safe exponential backoff counter
pub struct BackoffCounter {
    counter: AtomicU32,
}

impl Default for BackoffCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl BackoffCounter {
    pub fn new() -> Self {
        Self {
            counter: AtomicU32::new(0),
        }
    }

    pub fn next_delay(&self, base_delay_ms: u64, max_delay_ms: u64) -> std::time::Duration {
        let attempt = self.counter.fetch_add(1, Ordering::Relaxed);
        let delay = exponential_backoff(attempt, base_delay_ms, max_delay_ms);
        if attempt >= 10 {
            self.counter.store(0, Ordering::Relaxed); // Reset after max attempts
        }
        delay
    }
}
