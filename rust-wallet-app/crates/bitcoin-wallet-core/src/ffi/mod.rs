//! FFI surface — wallet-desktop dart:ffi bindings.
//!
//! Per Task 1 spike (`docs/superpowers/plans/2026-08-19-flutter-ffi-bitcoin-wallet-core.md`).
//! Task 1: minimal `wallet_list` to prove the FFI path works.
//! Task 2: error mapping (`FfiError` enum + thread-local message +
//!   `catch_unwind` wrapper) — foundation for Tasks 4-5.
//! Task 3: tokio runtime handle — async FFI bridge for Tasks 4-5.
//! Tasks 4-5: full wallet + Esplora FFI exports (use `FfiError` +
//!   `runtime_or_unknown` + `ffi_catch_unwind`).

#![allow(unsafe_code)] // FFI surface; safe `extern "C" fn` only.

pub mod bdk_extras;
pub mod error;
pub mod panic;
pub mod runtime;
pub mod wallet;
pub mod wallet_ops;

pub use error::{ffi_last_error_message, set_last_error, FfiError};
pub use panic::{ffi_catch_unwind, scrub_panic_message};
pub use runtime::{runtime_block_on, runtime_drop, runtime_new, runtime_or_unknown, RuntimeHandle};
