//! FFI surface — wallet-desktop dart:ffi bindings.
//!
//! Per Task 1 spike (`docs/superpowers/plans/2026-08-19-flutter-ffi-bitcoin-wallet-core.md`).
//! Minimal: only `wallet_list` to prove the FFI path works end-to-end.
//! Phase 1 Tasks 2-5 add error mapping, runtime bridge, full wallet + Esplora exports.

#![allow(unsafe_code)] // FFI surface; safe `extern "C" fn` only.

pub mod wallet;
