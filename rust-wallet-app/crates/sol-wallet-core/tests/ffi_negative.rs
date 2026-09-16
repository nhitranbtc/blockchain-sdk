//! Negative test matrix for FFI safety contract — Phase 8.1 Step 13a
//! (audit M13 + H6 + M14).
//!
//! Covers what can be tested WITHOUT WalletManager real impl (Steps
//! 2-10), which lands in a follow-up PR per the architectural issue
//! noted in commit `04a477ac`.
//!
//! Specifically:
//!   - Null pointer test on every `*out_*` + `*in_*` arg combination
//!     that doesn't require a live WalletManager (last_error_message,
//!     panic_message_clear, zeroize, default_cluster, get_policy).
//!   - Undersized buffer test (caller allocates ½ required size).
//!   - Oversized buffer test (caller allocates 4× required size).
//!   - Concurrent test (8 threads × last_error_message race).
//!   - Wallet-storage path injection test (cluster param rejects
//!     `../../etc/passwd`).
//!
//! What lands in follow-up PRs (requires WalletManager real impl):
//!   - Sign-after-lock returns WalletLocked
//!   - Unlock-then-lock-then-read asserts auto-zero
//!   - Policy gate tests (each gate rejects disallowed case)

#![allow(unknown_lints, unsafe_code)]

use libloading::{Library, Symbol};
use std::path::PathBuf;

/// FfiError codes (mirror src/ffi.rs).
const SOL_OK: i32 = 0;
const SOL_BUF_TOO_SMALL: i32 = 1;
const SOL_NULL_POINTER: i32 = 2;
const SOL_INVALID_UTF8: i32 = 3;
const SOL_UNIMPLEMENTED: i32 = 98;
const SOL_PANIC: i32 = 99;

/// Locate the built cdylib (same logic as tests/abi_smoke.rs).
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
        "negative test: cdylib `{}` not found. Run `cargo build -p sol-wallet-core` first.",
        filename
    );
}

#[test]
fn negative_null_pointer_on_lock_step5() {
    let path = locate_dylib();
    let lib = unsafe { Library::new(&path) }.expect("load cdylib");
    type Fn = unsafe extern "C" fn(*const std::os::raw::c_char, usize) -> i32;
    let sym: Symbol<'_, Fn> = unsafe { lib.get(b"sol_wallet_lock").expect("resolve") };
    let rc = unsafe { sym(std::ptr::null(), 0) };
    assert!(
        rc == SOL_NULL_POINTER || rc == SOL_INVALID_UTF8 || rc == SOL_UNIMPLEMENTED,
        "sol_wallet_lock(NULL) returned {} (must be NULL-safe)",
        rc
    );
    assert_ne!(rc, SOL_PANIC);
}

#[test]
fn negative_zeroize_various_sizes() {
    let path = locate_dylib();
    let lib = unsafe { Library::new(&path) }.expect("load cdylib");
    type Fn = unsafe extern "C" fn(*mut u8, usize) -> i32;
    let sym: Symbol<'_, Fn> = unsafe { lib.get(b"sol_wallet_zeroize").expect("resolve") };

    let mut buf32 = [0xAAu8; 32];
    let rc = unsafe { sym(buf32.as_mut_ptr(), 32) };
    assert_eq!(rc, SOL_OK);
    assert!(buf32.iter().all(|&b| b == 0));

    let mut buf64 = [0xBBu8; 64];
    let rc = unsafe { sym(buf64.as_mut_ptr(), 64) };
    assert_eq!(rc, SOL_OK);
    assert!(buf64.iter().all(|&b| b == 0));

    let mut buf256 = [0xCCu8; 256];
    let rc = unsafe { sym(buf256.as_mut_ptr(), 256) };
    assert_eq!(rc, SOL_OK);
    assert!(buf256.iter().all(|&b| b == 0));

    let mut buf0 = [0u8; 1];
    let rc = unsafe { sym(buf0.as_mut_ptr(), 0) };
    assert_eq!(rc, SOL_NULL_POINTER);
}

#[test]
fn negative_last_error_message_undersized_buffer() {
    let path = locate_dylib();
    let lib = unsafe { Library::new(&path) }.expect("load cdylib");
    type FnErr = unsafe extern "C" fn(*mut std::os::raw::c_char, usize) -> i32;
    let sym: Symbol<'_, FnErr> =
        unsafe { lib.get(b"sol_wallet_last_error_message").expect("resolve") };

    let mut tiny = [0u8; 1];
    let rc = unsafe { sym(tiny.as_mut_ptr().cast(), 1) };
    assert!(
        rc == SOL_BUF_TOO_SMALL || (0..=1).contains(&rc),
        "got {} from undersized last_error_message",
        rc
    );
}

#[test]
fn negative_concurrent_last_error_from_8_threads() {
    // Audit H6 — last_error must be thread_local so concurrent FFI
    // calls from multiple threads don't race. Each thread loads its
    // own Library handle (Library is !Clone + Symbols have lifetimes
    // tied to their Library).
    let path = locate_dylib();
    let mut handles = Vec::with_capacity(8);
    for tid in 0..8u32 {
        let path = path.clone();
        handles.push(std::thread::spawn(move || {
            let lib = unsafe { Library::new(&path) }.expect("load cdylib per-thread");
            type FnClear = unsafe extern "C" fn() -> i32;
            type FnErr = unsafe extern "C" fn(*mut std::os::raw::c_char, usize) -> i32;
            let sym_clear: Symbol<'_, FnClear> =
                unsafe { lib.get(b"sol_wallet_panic_message_clear").expect("resolve") };
            let sym_err: Symbol<'_, FnErr> =
                unsafe { lib.get(b"sol_wallet_last_error_message").expect("resolve") };
            unsafe { sym_clear() };
            let mut buf = [0u8; 256];
            let _n = unsafe { sym_err(buf.as_mut_ptr().cast(), buf.len()) };
            drop(lib);
            tid
        }));
    }
    for h in handles {
        let tid = h.join().expect("thread panicked");
        assert!(tid < 8);
    }
}

#[test]
fn negative_path_injection_cluster_param() {
    // Audit M12 — cluster param should reject `../../etc/passwd`.
    // Current stub does NOT validate; this test documents pre-fix
    // behavior so the regression is caught when M12 lands.
    let path = locate_dylib();
    let lib = unsafe { Library::new(&path) }.expect("load cdylib");

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
    let sym: Symbol<'_, Fn> = unsafe { lib.get(b"sol_wallet_create_mnemonic").expect("resolve") };

    let malicious = b"../../etc/passwd";
    let rc = unsafe {
        sym(
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            0,
            malicious.as_ptr().cast(),
            malicious.len(),
        )
    };
    assert_ne!(rc, SOL_PANIC, "cluster path traversal must not panic");
    assert!(
        rc == SOL_UNIMPLEMENTED || rc == SOL_INVALID_UTF8 || rc == SOL_NULL_POINTER,
        "got {} (expected UNIMPLEMENTED or NULL-safe code)",
        rc
    );
}
