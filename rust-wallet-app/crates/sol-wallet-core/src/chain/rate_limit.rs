//! `chain::rate_limit` — `RateLimiter` (Task 5.4).
//!
//! Hand-rolled token bucket (no `governor` crate dep to keep workspace
//! lean). `acquire()` is non-blocking: returns `Ok(())` if a token is
//! available, sleeps up to 1s for refill, then returns
//! `Err(Error::Transport("rate limit exceeded: <host>"))` if still empty.
//!
//! `RateLimiter::new(0, 0)` is a "disabled" sentinel — every
//! `acquire()` returns `Ok(())` immediately. Useful for tests that
//! don't want rate limiting.
//!
//! Plan §Phase 5 Task 5.4 acceptance:
//! - Default `50 req/s, burst 100`
//! - `sol send` rate-limited to ~1 req per ~20ms (50 req/s with
//!   headroom under cluster-level rate limits)
//! - `with_rate_limit(0, 0)` disables for tests
//!
//! Tier 3 finding #5 rationale: a malicious CLI script or buggy Phase 7
//! handler could fire 1000s of `sendTransaction` per second, bypassing
//! cluster-level rate limits + enabling duplicate-send footguns.
//!
//! Importers: `chain::client::RpcClient` (one per client instance);
//! `chain::account` 15 RPC method wrappers (each `post()` call goes
//! through `acquire()`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

/// Per-RPC rate limiter (token bucket).
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct RateLimiter {
    capacity: f64,
    refill_per_sec: f64,
    tokens: Arc<std::sync::Mutex<f64>>,
    last_refill: Arc<std::sync::Mutex<Instant>>,
    disabled: bool,
}

impl RateLimiter {
    /// Construct a new rate limiter.
    ///
    /// `req_per_sec` is the steady-state rate; `burst` is the max number
    /// of requests allowed in a single instant. `req_per_sec == 0` AND
    /// `burst == 0` is a sentinel for "disabled" — every call to
    /// `acquire()` returns `Ok(())` immediately.
    pub fn new(req_per_sec: u32, burst: u32) -> Self {
        let disabled = req_per_sec == 0 && burst == 0;
        Self {
            capacity: burst as f64,
            refill_per_sec: req_per_sec as f64,
            tokens: Arc::new(std::sync::Mutex::new(burst as f64)),
            last_refill: Arc::new(std::sync::Mutex::new(Instant::now())),
            disabled,
        }
    }

    /// Acquire a permit. Returns `Ok(())` if a token is available, or
    /// sleeps up to 1s for the bucket to refill before giving up.
    /// Returns `Err(Error::Transport("rate limit exceeded: <host>"))`
    /// if the bucket is still empty after the wait.
    pub async fn acquire(&self, host: &str) -> Result<()> {
        if self.disabled {
            return Ok(());
        }
        // Refill bucket based on elapsed wall time
        let now = Instant::now();
        {
            let mut last = self.last_refill.lock().expect("RateLimiter mutex poisoned");
            let elapsed = now.duration_since(*last).as_secs_f64();
            let mut tokens = self.tokens.lock().expect("RateLimiter mutex poisoned");
            *tokens = (*tokens + elapsed * self.refill_per_sec).min(self.capacity);
            *last = now;
        }
        // Try to take a token
        {
            let mut tokens = self.tokens.lock().expect("RateLimiter mutex poisoned");
            if *tokens >= 1.0 {
                *tokens -= 1.0;
                return Ok(());
            }
        }
        // Bucket empty — sleep up to 1s for refill, then give up
        tokio::time::sleep(Duration::from_secs(1)).await;
        Err(Error::Transport(format!("rate limit exceeded: {host}")))
    }
}
