//! FFI tokio runtime handle — Task 3 of 2026-08-19 FFI integration plan.
//!
//! Async FFI exports (Tasks 4-5 — Esplora sync, balance, send, etc.)
//! need a tokio runtime on the Rust side. The wallet-desktop Dart
//! process runs on its own event loop; we can't `block_on()` a Dart
//! Future. Instead:
//!
//! 1. Dart calls `runtime_new()` ONCE at app startup, receives an
//!    opaque handle (`*mut c_void`).
//! 2. Dart passes the handle to every async FFI export.
//! 3. The FFI body synchronously calls `rt.0.block_on(async { ... })`
//!    on the Rust side; the Dart thread blocks until completion.
//! 4. Dart calls `runtime_drop()` at app shutdown.
//!
//! **Why not expose a `runtime_block_on` callback export** (as in the
//! original plan)? A C callback cannot construct a Rust `Future` —
//! futures must come from Rust. Exposing a callback would force Dart
//! to register every async op as a Rust-side function pointer,
//! defeating the purpose of FFI (every new op = Rust recompile).
//! Synchronous `block_on` inside each export is simpler + correct.
//!
//! **Why a separate Runtime (not just block_on in each export)?**
//! tokio's runtime constructor is expensive (~ms) and the bdk_wallet
//! async I/O needs a stable reactor across multiple calls. Constructing
//! once at startup + reusing is the standard pattern.

use std::os::raw::c_void;

use super::error::FfiError;

/// tokio::Runtime handle for FFI-side async work. Multi-threaded
/// runtime pinned to 1 worker — wallet-desktop calls these one at a
/// time on the Dart main isolate (so 1 worker is enough), but the
/// multi-thread flavor (vs. `new_current_thread`) preserves `tokio::spawn`
/// support for any `'static + Send` future Tasks 4-5 may want to schedule.
pub struct RuntimeHandle(tokio::runtime::Runtime);

/// Create a new tokio runtime. Returns an opaque `*mut c_void` that
/// the Dart side stores and passes back to every async FFI export.
/// Caller MUST eventually call `runtime_drop` on the returned pointer.
///
/// # Safety
///
/// Returned pointer must be freed with `runtime_drop`. Thread-safe to
/// construct (separate runtimes per Dart isolate, if multi-isolate).
#[no_mangle]
pub unsafe extern "C" fn runtime_new() -> *mut c_void {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .thread_name("bitcoin-wallet-core-ffi")
        .build()
        .expect("failed to build tokio runtime");
    Box::into_raw(Box::new(RuntimeHandle(rt))) as *mut c_void
}

/// Drop the tokio runtime previously created by `runtime_new`.
///
/// # Safety
///
/// `handle` must either be null (no-op) or a pointer returned by
/// `runtime_new` AND not previously freed. Double-free is undefined
/// behavior. After this call, `handle` MUST NOT be used again.
#[no_mangle]
pub unsafe extern "C" fn runtime_drop(handle: *mut c_void) {
    if !handle.is_null() {
        let _ = Box::from_raw(handle as *mut RuntimeHandle);
    }
}

/// Re-export the `RuntimeHandle` newtype + the runtime handle so
/// downstream FFI exports (Tasks 4-5) can borrow the runtime and
/// call `.block_on(async { ... })` synchronously inside their body.
///
/// `runtime_block_on` is the internal helper FFI exports use to run
/// an async op synchronously. Returns the result of the future or
/// surfaces `FfiError` if the runtime pointer is invalid.
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `runtime_new` and not
/// yet dropped. `future` is any tokio `Future`. The caller must use
/// `ffi_catch_unwind` to convert panics into `FfiError::Panic`.
pub fn runtime_block_on<F, T>(handle: *mut c_void, future: F) -> Result<T, FfiError>
where
    F: std::future::Future<Output = T>,
{
    if handle.is_null() {
        return Err(FfiError::Unknown);
    }
    // SAFETY: documented contract — see # Safety on `runtime_new` /
    // `runtime_drop`. The caller is responsible for handle validity.
    let rt = unsafe { &*(handle as *const RuntimeHandle) };
    Ok(rt.0.block_on(future))
}

/// Borrow the tokio runtime for a synchronous block_on call. Returns
/// `None` if the handle is null (caller decides how to surface that —
/// typically as `FfiError::Unknown`).
///
/// This is a thin convenience wrapper used by individual FFI exports
/// (Tasks 4-5) to keep their bodies short:
///
/// ```ignore
/// pub unsafe extern "C" fn wallet_show(...) -> FfiError {
///     ffi_catch_unwind(|| {
///         let rt = runtime_or_unknown(runtime)?;
///         rt.block_on(async { ... }).into()
///     })
/// }
/// ```
///
/// # Safety
///
/// `handle` must be a valid pointer returned by `runtime_new` and not
/// yet dropped.
pub unsafe fn runtime_or_unknown(handle: *mut c_void) -> Option<&'static tokio::runtime::Runtime> {
    if handle.is_null() {
        return None;
    }
    let rt_handle: &RuntimeHandle = &*(handle as *const RuntimeHandle);
    Some(&rt_handle.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_new_and_drop() {
        let handle = unsafe { runtime_new() };
        assert!(!handle.is_null());
        unsafe { runtime_drop(handle) };
        // Double-free would be UB; rely on miri / sanitizer for that.
    }

    #[test]
    fn runtime_drop_null_is_noop() {
        unsafe { runtime_drop(std::ptr::null_mut()) };
        // Should not crash.
    }

    #[test]
    fn runtime_block_on_executes_async_op() {
        let handle = unsafe { runtime_new() };
        let result = runtime_block_on::<_, i32>(handle, async {
            // Simulate an async op (just resolves to a literal).
            42
        });
        assert_eq!(result, Ok(42));
        unsafe { runtime_drop(handle) };
    }

    #[test]
    fn runtime_block_on_null_returns_err() {
        let result = runtime_block_on(std::ptr::null_mut(), async {});
        assert_eq!(result, Err(FfiError::Unknown));
    }

    #[test]
    fn runtime_or_unknown_null_returns_none() {
        let result = unsafe { runtime_or_unknown(std::ptr::null_mut()) };
        assert!(result.is_none());
    }

    #[test]
    fn runtime_or_unknown_valid_returns_some() {
        let handle = unsafe { runtime_new() };
        let rt = unsafe { runtime_or_unknown(handle) };
        assert!(rt.is_some());
        unsafe { runtime_drop(handle) };
    }

    #[test]
    fn runtime_block_on_empty_future_resolves_ok() {
        let handle = unsafe { runtime_new() };
        let result = runtime_block_on(handle, async {});
        assert_eq!(result, Ok(()));
        unsafe { runtime_drop(handle) };
    }

    /// Documents the unwrapped-on-purpose contract: `runtime_block_on`
    /// does NOT catch panics from the future — `ffi_catch_unwind` at
    /// the FFI export boundary is the real guard. A panic in the
    /// future would unwind through `block_on` and reach the test
    /// thread; `catch_unwind` lets us assert that without aborting.
    #[test]
    fn runtime_block_on_panic_propagates() {
        let handle = unsafe { runtime_new() };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = runtime_block_on(handle, async { panic!("test panic") });
        }));
        assert!(outcome.is_err(), "panic should propagate, not be swallowed");
        unsafe { runtime_drop(handle) };
    }
}
