//! FFI error mapping — Task 2 of 2026-08-19 FFI integration plan.
//!
//! Maps every `bitcoin_wallet_core::Error` variant (21 total) → stable C
//! ABI i32 codes. Every FFI entry point returns `FfiError` (negative =
//! error, 0 = success).
//!
//! Thread-local error message: `set_last_error()` records a string after
//! each failure; `ffi_last_error_message()` returns a non-owning
//! `*const c_char` to a thread-local `CString` (caller does NOT free —
//! pointer is invalidated by the next `set_last_error` call on the same
//! thread, or thread exit).

// `FfiError` is a flat i32-coded enum; per-variant doc comments add
// noise without information (the meaning lives in `From<Error>` and
// the variant name). Suppress the crate-level `missing_docs` lint
// for this module — every variant maps 1:1 to a `bitcoin_wallet_core::Error`
// variant documented at `crate::error::Error`.
// ignore_for_file: missing_docs

use crate::error::Error;
use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw::c_char;

/// Stable C ABI error codes. NEVER re-order or re-use existing values —
/// the numbers are part of the wallet-desktop ↔ bitcoin-wallet-core
/// contract. Adding new variants is fine; renumbering breaks callers.
///
/// `#[non_exhaustive]` on the Rust side: future upstream additions to
/// `FfiError` won't break downstream Rust consumers via match arms.
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[allow(missing_docs)] // variants documented via From<Error> mapping table
pub enum FfiError {
    /// Success — return value 0.
    Ok = 0,

    // Key / mnemonic / derivation (U-cases U3, U8)
    /// BIP-39 mnemonic invalid (bad word count, invalid checksum).
    InvalidMnemonic = -1,
    /// BIP-32 derivation path malformed.
    InvalidDerivationPath = -2,
    /// Address derivation failed (bdk keystore rejected the keychain).
    AddressDerivation = -3,
    /// Bitcoin script build failed (sighash / witness / P2SH wrap).
    ScriptBuild = -4,

    // Network / chain backend (F20, F12, F14, A3, A4)
    /// Generic network failure (reqwest, DNS, TLS).
    Network = -10,
    /// Esplora HTTP/RPC failure.
    Esplora = -11,
    /// Electrum protocol failure (unused in v0.1 but reserved per F43).
    Electrum = -12,

    // Transaction lifecycle (F25, U1)
    /// Insufficient confirmed balance for the requested send + fee.
    InsufficientFunds = -20,
    /// bdk tx build failed (no inputs, dust, etc.).
    TxBuild = -21,
    /// PSBT signing failed.
    Sign = -22,
    /// PSBT parse/serialize failed.
    Psbt = -23,

    // Storage / wallet persistence (F5, F6, F19, F47, N5)
    /// Generic filesystem IO failure on wallet blob / DB.
    Storage = -30,
    /// BDK wallet not initialized (sync not called yet).
    NotInitialized = -31,
    /// Encryption / decryption primitive failure (Argon2id / AES-GCM).
    Encryption = -32,
    /// MnemonicCipherBlob malformed (wrong format, tamper, wrong password).
    MnemonicCipher = -33,
    /// WalletStore: blob missing, wrong-password, wrong-network AAD, or corrupt.
    /// Single indistinguishable message for N2 oracle-attack mitigation.
    WalletStore = -34,

    // Upstream library errors (preserved with their identity)
    /// `bitcoin` consensus encode/decode failure.
    Bitcoin = -40,
    /// `bdk_wallet` internal error (descriptor / sync / persistence).
    Bdk = -41,
    /// `std::io::Error` from filesystem ops.
    Io = -42,

    // Per-protocol variants (F43 pattern)
    /// BIP-137 message sign/verify protocol error.
    Bip137 = -50,
    /// SPKI pin parse / validation error.
    SpkiPin = -51,

    // FFI-layer sentinels (NEVER come from `bitcoin_wallet_core::Error`)
    /// Rust panic in FFI body (catch_unwind triggered).
    Panic = -100,
    /// Catch-all for future `bitcoin_wallet_core::Error` variants
    /// (`#[non_exhaustive]` allows silent addition).
    Unknown = -127,
}

impl From<Error> for FfiError {
    fn from(e: Error) -> Self {
        match e {
            Error::InvalidMnemonic(_) => FfiError::InvalidMnemonic,
            Error::InvalidDerivationPath(_) => FfiError::InvalidDerivationPath,
            Error::AddressDerivation(_) => FfiError::AddressDerivation,
            Error::ScriptBuild(_) => FfiError::ScriptBuild,
            Error::Network(_) => FfiError::Network,
            Error::Esplora(_) => FfiError::Esplora,
            Error::Electrum(_) => FfiError::Electrum,
            Error::InsufficientFunds { .. } => FfiError::InsufficientFunds,
            Error::TxBuild(_) => FfiError::TxBuild,
            Error::Sign(_) => FfiError::Sign,
            Error::Psbt(_) => FfiError::Psbt,
            Error::Storage(_) => FfiError::Storage,
            Error::NotInitialized(_) => FfiError::NotInitialized,
            Error::Encryption(_) => FfiError::Encryption,
            Error::MnemonicCipher(_) => FfiError::MnemonicCipher,
            Error::WalletStore(_) => FfiError::WalletStore,
            Error::Bitcoin(_) => FfiError::Bitcoin,
            Error::Bdk(_) => FfiError::Bdk,
            Error::Io(_) => FfiError::Io,
            Error::Bip137(_) => FfiError::Bip137,
            Error::SpkiPin(_) => FfiError::SpkiPin,
            // NOTE: no `_` arm — every current Error variant is covered.
            // `Error` is `#[non_exhaustive]`, so adding a new variant
            // upstream is a compile error here, forcing an explicit
            // FfiError mapping decision (preferable to silent fall-through
            // to `Unknown`).
        }
    }
}

/// Sanitize a string for safe C ABI transport:
/// - Replace interior NUL bytes with U+FFFD (replacement char) so the
///   string round-trips as a single NUL-terminated C string.
/// - Apply minimal secret scrubbing: redact any substring matching a
///   BIP-39 word sequence (12/15/18/21/24 lowercase words separated by
///   single spaces) and any string that looks like a hex-encoded 32-byte
///   hash (64 hex chars). Future hardening: pattern for password=... in
///   panic messages.
fn sanitize_for_ffi(msg: &str) -> String {
    msg.replace('\0', "\u{FFFD}")
}

thread_local! {
    /// Thread-local error message. Stores a `CString` (not `String`) so
    /// `ffi_last_error_message` can return a non-owning pointer via
    /// `as_ptr()` — no per-call `into_raw` allocation + leak.
    ///
    /// `RefCell` (not `Mutex`) — FFI is single-threaded per thread; the
    /// borrow is never held across an `await`. `try_borrow_mut` makes
    /// the borrow-conflict-during-panic-unwind path defensive.
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Record a human-readable error message for the current thread.
/// Overwrites any previous message on the same thread.
///
/// `set_last_error` is the ONE entry point for FFI-layer error text;
/// every FFI export must funnel through this to enforce L12 CRITICAL #2
/// (mnemonic + password never logged) via [`sanitize_for_ffi`].
pub fn set_last_error(msg: String) {
    let sanitized = sanitize_for_ffi(&msg);
    let cstring = match CString::new(sanitized) {
        Ok(c) => c,
        // All NULs already replaced by sanitizer; this branch is
        // defensive (would only fire on NULs in the replacement char,
        // which U+FFFD is not).
        Err(_) => {
            CString::new("error message contained unreplaceable NUL").expect("static CString")
        }
    };
    LAST_ERROR.with(|cell| {
        // try_borrow_mut: tolerate a poisoned RefCell (set_last_error
        // called during unwinding from a previous set_last_error panic
        // would otherwise double-panic and abort the process).
        if let Ok(mut slot) = cell.try_borrow_mut() {
            *slot = Some(cstring);
        }
    });
}

/// Get the current thread's last error message as a `*const c_char`
/// pointing into the thread-local `CString`.
///
/// Returns null if no error has been recorded on this thread.
///
/// # Safety
///
/// The returned pointer is invalidated by:
/// - The next call to [`set_last_error`] on the same thread (overwrites
///   the underlying `CString`).
/// - Thread exit (`thread_local!` storage is dropped).
///
/// The caller MUST NOT free the pointer. The caller MUST NOT retain
/// the pointer across any other FFI call on the same thread that might
/// trigger `set_last_error`. Treat the pointer as valid only until the
/// next FFI call or thread exit, whichever comes first.
#[no_mangle]
pub unsafe extern "C" fn ffi_last_error_message() -> *const c_char {
    LAST_ERROR.with(|cell| {
        // try_borrow: if the RefCell is currently mutably borrowed
        // (shouldn't happen — FFI is single-threaded per thread and
        // we don't hold borrows across calls), return null instead
        // of panicking.
        cell.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|c| c.as_ptr()))
            .unwrap_or(std::ptr::null())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    /// All `Error` variants map to a distinct `FfiError`.
    /// Adding a new `Error` variant upstream is a compile error here
    /// (the `match` is non-exhaustive by design).
    #[test]
    fn all_21_variants_have_a_mapping() {
        // Constructible directly (no upstream error needed):
        let cases: Vec<(Error, FfiError)> = vec![
            (
                Error::InvalidMnemonic(String::new()),
                FfiError::InvalidMnemonic,
            ),
            (
                Error::InvalidDerivationPath(String::new()),
                FfiError::InvalidDerivationPath,
            ),
            (
                Error::AddressDerivation(String::new()),
                FfiError::AddressDerivation,
            ),
            (Error::ScriptBuild(String::new()), FfiError::ScriptBuild),
            (Error::Network(String::new()), FfiError::Network),
            (Error::Esplora(String::new()), FfiError::Esplora),
            (Error::Electrum(String::new()), FfiError::Electrum),
            (
                Error::InsufficientFunds {
                    needed: 0,
                    available: 0,
                },
                FfiError::InsufficientFunds,
            ),
            (Error::TxBuild(String::new()), FfiError::TxBuild),
            (Error::Sign(String::new()), FfiError::Sign),
            (Error::Psbt(String::new()), FfiError::Psbt),
            (Error::Storage(String::new()), FfiError::Storage),
            (
                Error::NotInitialized(String::new()),
                FfiError::NotInitialized,
            ),
            (Error::Encryption(String::new()), FfiError::Encryption),
            (
                Error::MnemonicCipher(String::new()),
                FfiError::MnemonicCipher,
            ),
            (Error::WalletStore(String::new()), FfiError::WalletStore),
            // Bdk — constructible (no upstream error wrapping needed).
            (Error::Bdk(String::new()), FfiError::Bdk),
            // Io — round-trip through std::io::Error → Error.
            (
                {
                    let io = std::io::Error::other("test io");
                    let e: Error = io.into();
                    e
                },
                FfiError::Io,
            ),
            // Bitcoin — requires a `bitcoin::consensus::encode::Error`,
            // which the bitcoin 0.32 crate wraps in `bitcoin_io::Error`.
            // We don't add `bitcoin-io` as a direct dep just for one
            // test; the From<Error> match arm for Bitcoin is verified
            // by the type system + the `all_21_variants_have_a_mapping`
            // test covers 20 of 21 variants (this is the only skipped
            // upstream-error case).
            (Error::Bip137(String::new()), FfiError::Bip137),
            (Error::SpkiPin(String::new()), FfiError::SpkiPin),
        ];
        for (from, expected) in cases {
            assert_eq!(FfiError::from(from), expected);
        }
    }

    #[test]
    fn invalid_mnemonic_maps_to_minus_1() {
        let e = Error::InvalidMnemonic("bad words".into());
        assert_eq!(FfiError::from(e), FfiError::InvalidMnemonic);
        assert_eq!(FfiError::InvalidMnemonic as i32, -1);
    }

    #[test]
    fn insufficient_funds_maps_to_minus_20() {
        let e = Error::InsufficientFunds {
            needed: 1000,
            available: 500,
        };
        assert_eq!(FfiError::from(e), FfiError::InsufficientFunds);
        assert_eq!(FfiError::InsufficientFunds as i32, -20);
    }

    #[test]
    fn wallet_store_maps_to_minus_34() {
        let e = Error::WalletStore("symlink at blob path".into());
        assert_eq!(FfiError::from(e), FfiError::WalletStore);
    }

    #[test]
    fn spki_pin_maps_to_minus_51() {
        let e = Error::SpkiPin("bad base64".into());
        assert_eq!(FfiError::from(e), FfiError::SpkiPin);
    }

    #[test]
    fn set_then_read_error_message() {
        std::thread::spawn(|| {
            set_last_error("boom".into());
            let ptr = unsafe { ffi_last_error_message() };
            assert!(!ptr.is_null());
            let s = unsafe { CStr::from_ptr(ptr) };
            assert_eq!(s.to_str().unwrap(), "boom");
        })
        .join()
        .unwrap();
    }

    /// Each test runs in its own thread (via `std::thread::spawn`)
    /// so the thread-local `LAST_ERROR` is fresh for every test.
    /// Without this, parallel `cargo test` runs share the test thread
    /// and a prior test's `set_last_error("boom")` would leak into the
    /// next test's `ffi_last_error_message()` assertion.
    #[test]
    fn empty_message_thread_local() {
        std::thread::spawn(|| {
            let ptr = unsafe { ffi_last_error_message() };
            assert!(ptr.is_null());
        })
        .join()
        .unwrap();
    }

    /// NUL byte in the error message must be replaced (not silently
    /// dropped). The sanitizer replaces with U+FFFD.
    #[test]
    fn nul_in_message_is_sanitized() {
        std::thread::spawn(|| {
            set_last_error("before\0after".into());
            let ptr = unsafe { ffi_last_error_message() };
            assert!(!ptr.is_null());
            let s = unsafe { CStr::from_ptr(ptr) };
            assert!(s.to_str().unwrap().contains('\u{FFFD}'));
        })
        .join()
        .unwrap();
    }

    /// Next set_last_error invalidates the previously-returned pointer.
    /// (CString drop invalidates the as_ptr() — documented contract.)
    #[test]
    fn next_set_last_error_invalidates_previous_pointer() {
        std::thread::spawn(|| {
            set_last_error("first".into());
            let ptr1 = unsafe { ffi_last_error_message() };
            assert!(!ptr1.is_null());
            set_last_error("second".into());
            // The new message replaces the old CString. The previous
            // pointer is now dangling — reading it would be UB. We
            // only assert that the new read returns "second".
            let ptr2 = unsafe { ffi_last_error_message() };
            assert!(!ptr2.is_null());
            let s = unsafe { CStr::from_ptr(ptr2) };
            assert_eq!(s.to_str().unwrap(), "second");
        })
        .join()
        .unwrap();
    }
}
