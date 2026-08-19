//! FFI surface — version helpers only.
//!
//! `wallet_list` (Task 1 spike) was replaced by the proper ABI in
//! `wallet_ops.rs` per Task 4. `ffi_version` stays here as the symbol-
//! lookup sanity check.

use std::ffi::CString;
use std::os::raw::c_char;

/// Return the rust crate version as a C string. Sanity check that
/// symbol lookup works.
///
/// # Safety
///
/// Caller must free the returned pointer with `ffi_version_free` (or
/// `CString::from_raw`). Returns null on internal failure (env var
/// conversion failure — extremely unlikely).
#[no_mangle]
pub unsafe extern "C" fn ffi_version() -> *mut c_char {
    let s = match CString::new(env!("CARGO_PKG_VERSION")) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    s.into_raw()
}

/// Free a string returned by `ffi_version`.
///
/// # Safety
///
/// `ptr` must either be null (no-op) or a pointer that was previously
/// returned by `ffi_version` AND has not yet been freed. Double-free or
/// freeing an arbitrary pointer causes undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn ffi_version_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    #[test]
    fn ffi_version_returns_non_null() {
        let ptr = unsafe { super::ffi_version() };
        assert!(!ptr.is_null());
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "0.2.0");
        unsafe { super::ffi_version_free(ptr) };
    }
}
