//! owner: WP-R.3
//!
//! Transient-failure schedule of the upload queue (`ResearchSyncRetryPolicy`).

/// Delays after the 1st, 2nd, ... failure; the last one repeats.
pub const BACKOFF_DELAYS_SECS: [u64; 5] = [1, 2, 4, 8, 60];
/// Failures tolerated before a payload is dead-lettered: one past the last delay.
pub const MAX_ATTEMPTS: u32 = BACKOFF_DELAYS_SECS.len() as u32 + 1;

/// Delay in seconds after failed attempt number `attempt` (1-based; 0 is treated as 1).
pub fn backoff_schedule(attempt: u32) -> u64 {
    let index = (attempt.max(1) as usize - 1).min(BACKOFF_DELAYS_SECS.len() - 1);
    BACKOFF_DELAYS_SECS[index]
}

/// The wait before the next attempt: the larger of the schedule and the server's
/// `Retry-After` (already capped by [`crate::retry_after_seconds`]).
pub fn next_retry_delay(attempt: u32, retry_after_secs: Option<f64>) -> f64 {
    (backoff_schedule(attempt) as f64).max(retry_after_secs.unwrap_or(0.0))
}
