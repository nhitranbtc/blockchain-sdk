//! Phase 5 — `Clock` PAL trait.
//!
//! Per plan §Phase 5 Task 4.1: `now_millis / sleep`.
//!
//! **Why a PAL around time:** transaction `expiration` and `timestamp`
//! fields are wall-clock sensitive; tests need deterministic time to
//! assert `expiration = now + 60s` exactly. The trait gives `TxBuilder`
//! a `Clock` injection point so unit tests can pin time without
//! monkey-patching the system clock.
//!
//! **Production impls** return `SystemTime::now().duration_since(UNIX_EPOCH)`
//! and `tokio::time::sleep`. **Test impls** advance only when the test
//! asks them to.

use std::time::Duration;

/// Monotonic-ish time source for transaction expiration + timestamps.
///
/// **Not strictly monotonic** — implementations MAY go backwards (NTP
/// correction, system clock change). The TRON wire format depends on
/// wall-clock seconds, not monotonic time, so the test for "did time
/// go backwards?" is the network's accept-reject decision, not ours.
pub trait Clock: Send + Sync {
    /// Milliseconds since the Unix epoch (1970-01-01T00:00:00Z).
    ///
    /// `set_timestamp` in `tx::builder` consumes this directly. Tests
    /// inject a `MockClock` whose `now_millis` is a pinned offset
    /// from a fixed epoch so byte-for-byte protobuf assertions hold.
    fn now_millis(&self) -> i64;

    /// Sleep for `duration`. Tests usually `instant_sleep` (no-op).
    ///
    /// Implementations on a tokio runtime use `std::thread::sleep`
    /// (the trait is sync); the wallet binary awaits this on an async
    /// path via `tokio::task::spawn_blocking`. Async `sleep` would
    /// force `BoxFuture` indirection that buys nothing — sync
    /// `Thread::sleep` integrates cleanly with both the CLI's
    /// multi-thread runtime and FFI's pinned `current_thread` runtime.
    fn sleep(&self, duration: Duration);
}
