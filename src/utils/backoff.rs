//! Exponential backoff retry utilities
//!
//! Provides thread-safe backoff counting and delay calculation for retry logic.

use std::sync::atomic::{AtomicU32, Ordering};

/// Calculate exponential backoff delay for a given attempt number.
///
/// # Arguments
///
/// * `attempt` - Current attempt number (0-indexed)
/// * `base_delay_ms` - Base delay in milliseconds for the first retry
/// * `max_delay_ms` - Maximum delay cap in milliseconds
///
/// # Returns
///
/// The delay duration for the next retry attempt.
///
/// # Example
///
/// ```
/// use dns_ingress::utils::backoff::exponential_backoff;
///
/// let delay = exponential_backoff(0, 100, 5000);  // 100ms
/// let delay = exponential_backoff(1, 100, 5000);  // 200ms
/// let delay = exponential_backoff(3, 100, 5000);  // 800ms
/// ```
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

/// Thread-safe exponential backoff counter.
///
/// Automatically tracks retry attempts and calculates delays using
/// exponential backoff strategy. Resets after reaching maximum attempts.
///
/// # Example
///
/// ```rust
/// use std::time::Duration;
/// use dns_ingress::utils::backoff::BackoffCounter;
///
/// let counter = BackoffCounter::new();
/// let delay = counter.next_delay(100, 5000); // 100ms
/// let _delay = counter.next_delay(100, 5000); // 200ms
/// ```
#[derive(Debug)]
pub struct BackoffCounter {
    counter: AtomicU32,
}

impl Default for BackoffCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl BackoffCounter {
    /// Create a new backoff counter with zero attempts.
    #[must_use]
    pub fn new() -> Self {
        Self {
            counter: AtomicU32::new(0),
        }
    }

    /// Get the next delay duration and increment the attempt counter.
    ///
    /// # Arguments
    ///
    /// * `base_delay_ms` - Base delay in milliseconds
    /// * `max_delay_ms` - Maximum delay cap in milliseconds
    ///
    /// # Returns
    ///
    /// The calculated delay duration. Counter resets after 10 attempts.
    pub fn next_delay(&self, base_delay_ms: u64, max_delay_ms: u64) -> std::time::Duration {
        let attempt = self.counter.fetch_add(1, Ordering::Relaxed);
        let delay = exponential_backoff(attempt, base_delay_ms, max_delay_ms);
        if attempt >= 10 {
            self.counter.store(0, Ordering::Relaxed); // Reset after max attempts
        }
        delay
    }
}
