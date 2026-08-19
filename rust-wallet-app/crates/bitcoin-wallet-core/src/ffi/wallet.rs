//! Minimal sync FFI: `wallet_list` only.
//!
//! Returns wallet IDs as a single newline-separated UTF-8 string. Caller
//! (Dart) parses and splits. Simplest possible FFI shape — no array
//! allocation, no UUID byte conversion. Phase 1 Task 4 replaces this
//! with a proper array-returning C ABI.

use crate::error::Error;
use crate::wallet::{data_dir, list_wallets};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Network byte → bitcoin::Network. Other values → -1.
const NETWORK_TESTNET: u8 = 1;

/// List all wallets for the given network as a newline-separated UTF-8
/// string of UUIDs. Caller frees the returned pointer with
/// `wallet_list_free`.
///
/// `network`: 1 = Testnet (others TBD in Phase 1).
///
/// Returns:
/// - 0 on success (even with empty list — `*out_ptr` is then null)
/// - 1 on error
#[no_mangle]
pub extern "C" fn wallet_list(network: u8, out_ptr: *mut *mut c_char) -> i32 {
    if out_ptr.is_null() {
        return 1;
    }
    let net = match network {
        NETWORK_TESTNET => bitcoin::Network::Testnet,
        _ => return 1,
    };

    let result: Result<Vec<String>, Error> = (|| {
        let base = data_dir()?;
        let ids = list_wallets(&base, net)?;
        Ok::<_, Error>(ids.iter().map(|id| id.to_string()).collect())
    })();

    let ids: Vec<String> = match result {
        Ok(v) => v,
        Err(_) => return 1,
    };

    let joined = ids.join("\n");
    let c_string = match CString::new(joined) {
        Ok(s) => s,
        Err(_) => return 1,
    };

    unsafe {
        *out_ptr = c_string.into_raw();
    }
    0
}

/// Free a string returned by `wallet_list`.
#[no_mangle]
pub extern "C" fn wallet_list_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Return the rust crate version as a C string. Sanity check that
/// symbol lookup works.
#[no_mangle]
pub extern "C" fn ffi_version() -> *mut c_char {
    let s = match CString::new(env!("CARGO_PKG_VERSION")) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    s.into_raw()
}

/// Free a string returned by `ffi_version`.
#[no_mangle]
pub extern "C" fn ffi_version_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Read the returned string back (for Rust-side tests).
#[cfg(test)]
pub fn wallet_list_for_test(network: u8) -> Result<Vec<String>, i32> {
    let net = match network {
        NETWORK_TESTNET => bitcoin::Network::Testnet,
        _ => return Err(1),
    };
    let base = data_dir().map_err(|_| 1)?;
    list_wallets(&base, net)
        .map_err(|_| 1)
        .map(|ids| ids.iter().map(|id| id.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_version_returns_non_null() {
        let ptr = ffi_version();
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "0.2.0");
        ffi_version_free(ptr);
    }

    #[test]
    fn wallet_list_testnet_succeeds() {
        let ids = wallet_list_for_test(NETWORK_TESTNET);
        assert!(ids.is_ok());
    }
}
