//! `sol-wallet-core` — `Clock` PAL.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Monotonic time source. Used for tx-expiration windows +
/// `commitment` polling timeouts.
pub trait Clock: Send + Sync {
    /// Monotonic instant — for measuring elapsed time.
    fn now_monotonic(&self) -> Instant;
    /// Wall-clock seconds since UNIX epoch.
    fn now_unix_secs(&self) -> u64;
    /// Sleep for `duration`.
    fn sleep(&self, duration: Duration);
}

/// Production clock — uses `Instant::now` / `SystemTime::now` /
/// `std::thread::sleep`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_monotonic(&self) -> Instant {
        Instant::now()
    }
    fn now_unix_secs(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Test clock — `now_unix_secs` returns the captured start time +
/// accumulated `advance` deltas. `sleep` is a no-op.
#[derive(Debug)]
pub struct MockClock {
    start_unix: u64,
    offset_secs: u64,
}

impl Default for MockClock {
    fn default() -> Self {
        Self {
            start_unix: 1_700_000_000,
            offset_secs: 0,
        }
    }
}

impl MockClock {
    /// Construct with explicit start.
    pub fn new(start_unix: u64) -> Self {
        Self {
            start_unix,
            offset_secs: 0,
        }
    }
    /// Advance the clock by `duration`.
    pub fn advance(&mut self, duration: Duration) {
        self.offset_secs += duration.as_secs();
    }
}

impl Clock for MockClock {
    fn now_monotonic(&self) -> Instant {
        Instant::now()
    }
    fn now_unix_secs(&self) -> u64 {
        self.start_unix + self.offset_secs
    }
    fn sleep(&self, _duration: Duration) {
        // No-op — caller uses `MockClock::advance` to move time.
    }
}
