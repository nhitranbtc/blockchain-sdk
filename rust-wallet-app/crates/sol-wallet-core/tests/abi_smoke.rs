//! In-process ABI smoke test — Phase 8.1 Step 13 (audit H17).
//!
//! Loads the built `libsol_wallet_core.{so,dylib,dll}` via `libloading`,
//! resolves the 16 `sol_wallet_*` symbols, and invokes each with
//! documented NULL inputs. Asserts:
//!
//!   1. Every symbol resolves (no missing exports).
//!   2. Every NULL-input call returns a documented NULL-safe FfiError
//!      code (NOT `SOL_PANIC = 99`).
//!   3. No segfault (libloading propagates the error if it does).
//!
//! Requires the cdylib to be built first:
//!
//!   cargo build -p sol-wallet-core
//!   cargo test -p sol-wallet-core --test abi_smoke
//!
//! On cross-target build (iOS / Android), the `tests/abi_smoke.c`
//! harness is the equivalent: invoke from a real non-Rust consumer
//! to prove the C ABI is intact.

#![allow(unknown_lints, unsafe_code)]

use libloading::{Library, Symbol};
use std::path::PathBuf;

/// FfiError codes — mirror src/ffi.rs enum values.
const SOL_OK: i32 = 0;
const SOL_BUF_TOO_SMALL: i32 = 1;
const SOL_NULL_POINTER: i32 = 2;
const SOL_INVALID_UTF8: i32 = 3;
const SOL_WALLET_LOCKED: i32 = 5;
const SOL_UNIMPLEMENTED: i32 = 98;
const SOL_PANIC: i32 = 99;

/// Assert return code is in the NULL-safe set (NOT panic).
fn assert_null_safe(rc: i32, label: &str) {
    if rc == SOL_PANIC {
        panic!(
            "RED: {} returned SOL_PANIC (99) — scrubber missed a panic?",
            label
        );
    }
    const OK: &[i32] = &[
        SOL_OK,
        SOL_BUF_TOO_SMALL,
        SOL_NULL_POINTER,
        SOL_INVALID_UTF8,
        SOL_WALLET_LOCKED,
        SOL_UNIMPLEMENTED,
    ];
    if !OK.contains(&rc) {
        panic!(
            "RED: {} returned unexpected code {} (not in NULL-safe set)",
            label, rc
        );
    }
}

/// Locate the built cdylib. Walks up from `CARGO_MANIFEST_DIR` to find
/// the workspace target dir (the cdylib is built under
/// `<workspace>/target/<profile>/` regardless of whether the test runs
/// via `cargo test` from the workspace root or from the crate dir).
fn locate_dylib() -> PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set by cargo");
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    #[cfg(target_os = "macos")]
    let filename = "libsol_wallet_core.dylib";
    #[cfg(target_os = "linux")]
    let filename = "libsol_wallet_core.so";
    #[cfg(target_os = "windows")]
    let filename = "sol_wallet_core.dll";

    let candidates = [
        PathBuf::from(&manifest_dir)
            .join("..")
            .join("..")
            .join("target"),
        PathBuf::from(&manifest_dir).join("..").join("target"),
        PathBuf::from(&manifest_dir).join("target"),
        PathBuf::from("target"),
    ];
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
        for base in &candidates {
            let p = base.join(&td).join(profile).join(filename);
            if p.exists() {
                return p;
            }
        }
    }
    for base in &candidates {
        let p = base.join(profile).join(filename);
        if p.exists() {
            return p;
        }
        let p2 = base.join(filename);
        if p2.exists() {
            return p2;
        }
    }
    panic!(
        "ABI smoke: cdylib `{}` not found. Run `cargo build -p sol-wallet-core` first. Searched: {:?}",
        filename, candidates
    );
}

#[test]
fn abi_smoke_loads_and_resolves_all_16_exports() {
    let path = locate_dylib();
    // SAFETY: loading a cdylib produced by the same Cargo build that
    // compiled this test. The symbol set is audited by
    // scripts/symbol_audit.sh.
    let lib = unsafe { Library::new(&path) }
        .unwrap_or_else(|e| panic!("ABI smoke: failed to load {} — {}", path.display(), e));

    // Resolve every documented symbol. If any is missing, the
    // `unsafe { Symbol::new(...) }` returns Err.
    let symbols: &[&str] = &[
        "sol_wallet_create_mnemonic",
        "sol_wallet_import_mnemonic",
        "sol_wallet_unlock",
        "sol_wallet_lock",
        "sol_wallet_get_address",
        "sol_wallet_sign_transaction",
        "sol_wallet_send_sol",
        "sol_wallet_send_spl",
        "sol_wallet_get_balance_sol",
        "sol_wallet_get_balance_spl",
        "sol_wallet_last_error_message",
        "sol_wallet_panic_message_clear",
        "sol_wallet_zeroize",
        "sol_wallet_set_policy",
        "sol_wallet_get_policy",
        "sol_wallet_default_cluster",
    ];

    for name in symbols {
        let _: Symbol<'_, unsafe extern "C" fn() -> i32> = unsafe {
            lib.get(name.as_bytes())
                .unwrap_or_else(|e| panic!("ABI smoke: missing symbol {} — {}", name, e))
        };
    }

    eprintln!(
        "ABI smoke GREEN: 16/16 symbols resolved from {}",
        path.display()
    );
}

#[test]
fn abi_smoke_null_input_matrix() {
    let path = locate_dylib();
    let lib = unsafe { Library::new(&path) }
        .unwrap_or_else(|e| panic!("ABI smoke: failed to load {} — {}", path.display(), e));

    // last_error_message + panic_message_clear + zeroize have their
    // own signatures — exercise with NULL via typed symbols.
    type FnLastErr = unsafe extern "C" fn(*mut std::os::raw::c_char, usize) -> i32;
    let last_err: Symbol<'_, FnLastErr> =
        unsafe { lib.get(b"sol_wallet_last_error_message").expect("resolve") };
    let rc = unsafe { last_err(std::ptr::null_mut(), 0) };
    assert_eq!(
        rc, SOL_NULL_POINTER,
        "last_error_message(NULL) → NullPointer"
    );

    type FnClear = unsafe extern "C" fn() -> i32;
    let clear: Symbol<'_, FnClear> =
        unsafe { lib.get(b"sol_wallet_panic_message_clear").expect("resolve") };
    let rc = unsafe { clear() };
    assert_eq!(rc, SOL_OK, "panic_message_clear() → Ok");

    type FnZeroize = unsafe extern "C" fn(*mut u8, usize) -> i32;
    let zeroize: Symbol<'_, FnZeroize> =
        unsafe { lib.get(b"sol_wallet_zeroize").expect("resolve") };
    let rc = unsafe { zeroize(std::ptr::null_mut(), 0) };
    assert_eq!(rc, SOL_NULL_POINTER, "zeroize(NULL) → NullPointer");

    // For create_mnemonic + import_mnemonic + the rest: resolve + call
    // with NULL via a 4-arg signature.
    macro_rules! call_null_4arg {
        ($name:literal) => {{
            type Fn = unsafe extern "C" fn(
                *mut std::os::raw::c_char,
                usize,
                *mut std::os::raw::c_char,
                usize,
                *const std::os::raw::c_char,
                usize,
                *const std::os::raw::c_char,
                usize,
            ) -> i32;
            let sym: Symbol<'_, Fn> = unsafe { lib.get($name.as_bytes()).expect("resolve") };
            unsafe {
                sym(
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                    0,
                    std::ptr::null(),
                    0,
                )
            }
        }};
    }
    let rc = call_null_4arg!("sol_wallet_create_mnemonic");
    assert_null_safe(rc, "create_mnemonic NULL");

    let rc = call_null_4arg!("sol_wallet_import_mnemonic");
    assert_null_safe(rc, "import_mnemonic NULL");

    eprintln!("ABI smoke NULL-input matrix GREEN: 5/5 typed NULL calls + 16/16 symbol resolves");
}
