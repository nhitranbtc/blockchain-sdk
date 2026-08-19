//! FFI panic safety wrapper — Task 2 of 2026-08-19 FFI integration plan.
//!
//! A Rust panic that crosses an FFI boundary is undefined behavior in
//! the C ABI contract — it can corrupt the C caller's stack frame and
//! leave mutexes held, allocations leaked, etc. Every FFI entry point
//! MUST wrap its body in [`ffi_catch_unwind`] so a panic becomes a
//! typed `FfiError::Panic` instead of UB.
//!
//! **L12 CRITICAL #2** (mnemonic + password never logged): panic
//! messages flow through Dart via `ffi_last_error_message`. Without
//! scrubbing, a panic like `"failed: password='hunter2'"` would leak the
//! password to the UI / logs. [`scrub_panic_message`] applies a
//! defensive scrub before the message reaches Dart.
//!
//! Usage:
//!
//! ```ignore
//! #[no_mangle]
//! pub unsafe extern "C" fn my_export(...) -> FfiError {
//!     ffi_catch_unwind(|| {
//!         // ... body that may panic ...
//!         FfiError::Ok
//!     })
//! }
//! ```
//!
//! Limitations:
//! - `AssertUnwindSafe` is used because the body is `UnwindSafe`-tricky
//!   (raw pointers, &mut references to opaque FFI state). The body MUST
//!   not carry `&mut` references across the unwind boundary that the
//!   caller depends on being released (no mutex guards, no borrowed
//!   locks). The wrapped closure runs inside a function whose only
//!   responsibility is to return a status code; all persistent state
//!   is owned by the FfiError + thread-local.
//! - `UnwindSafe` is a marker; if the closure body has types that
//!   aren't unwind-safe by default, the compiler will reject the
//!   `AssertUnwindSafe` wrap. That's a build error, not a runtime one.

use std::panic::{self, AssertUnwindSafe};

use super::error::{set_last_error, FfiError};

/// Strip patterns that look like secrets from a panic message before it
/// surfaces to Dart via `ffi_last_error_message`. Best-effort, not
/// exhaustive — future secret-bearing patterns need to be added here.
///
/// Patterns currently scrubbed:
/// - `password=...` / `password: ...` / `Password: ...` (case-insensitive)
///   → replaced with `password=<redacted>`
/// - `mnemonic=...` / `mnemonic: ...` → replaced with `mnemonic=<redacted>`
/// - `secret=...` / `secret: ...` → replaced with `secret=<redacted>`
/// - 12/15/18/21/24-lowercase-word sequences (BIP-39 mnemonic shape)
///   → replaced with `<redacted-mnemonic>`
///
/// The regex-based scrubber is intentionally conservative — false
/// positives (redacting a non-secret) are acceptable; false negatives
/// (leaking a secret) are not. L12 CRITICAL #2 prefers the former.
pub(crate) fn scrub_panic_message(msg: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;

    // (case-insensitive password / mnemonic / secret = value OR ": value")
    static SECRET_KV: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)\b(password|mnemonic|secret)\s*([:=])\s*\S+").unwrap());
    // BIP-39 mnemonic shape: 12/15/18/21/24 lowercase words, single spaces.
    // We use a sliding-window approach because the `regex` crate's
    // greedy matching with `{n,m}` quantifiers over `\s` had unreliable
    // behavior in tests (see git history); this is a 3-line manual
    // scanner that is robust.

    let s = SECRET_KV
        .replace_all(msg, |caps: &regex::Captures<'_>| {
            format!("{}{}<redacted>", &caps[1], &caps[2])
        })
        .into_owned();

    // BIP-39 mnemonic scrub — manual scanner (regex crate's
    // quantifier behavior on `\s` was unreliable in tests; this is
    // explicit and auditable). Find runs of 12..=24 consecutive
    // lowercase ASCII words separated by single spaces.
    // quantifier behavior on `\s` was unreliable in tests; this is
    // explicit and auditable). Find runs of 12..=24 consecutive
    // lowercase ASCII words separated by single spaces.
    fn scrub_mnemonic(s: &str) -> String {
        const MIN_WORDS: usize = 12;
        const MAX_WORDS: usize = 24;
        let mut out = String::with_capacity(s.len());
        let mut i = 0;
        let bytes = s.as_bytes();
        while i < bytes.len() {
            // Skip non-lowercase prefix.
            if !bytes[i].is_ascii_lowercase() {
                // Find the next lowercase letter; copy everything before
                // it to out, then start counting.
                let mut j = i;
                while j < bytes.len() && !bytes[j].is_ascii_lowercase() {
                    j += 1;
                }
                out.push_str(&s[i..j]);
                i = j;
                continue;
            }
            // We're at a lowercase letter. Count consecutive words.
            let run_start = i;
            let mut word_count = 0;
            let mut j = i;
            while j < bytes.len() {
                // Read one word.
                while j < bytes.len() && bytes[j].is_ascii_lowercase() {
                    j += 1;
                }
                word_count += 1;
                if word_count > MAX_WORDS {
                    break;
                }
                // Word must be followed by exactly one space + another
                // lowercase letter, OR end-of-string (for the last word).
                if j == bytes.len() {
                    break;
                }
                if bytes[j] != b' ' {
                    // Not a word boundary — abort this run.
                    break;
                }
                // Check the char after the space.
                if j + 1 >= bytes.len() || !bytes[j + 1].is_ascii_lowercase() {
                    break;
                }
                j += 1; // consume the space
            }
            if (MIN_WORDS..=MAX_WORDS).contains(&word_count) {
                // REPLACE the run with `<redacted-mnemonic>`: push
                // everything BEFORE the run, then the placeholder, then
                // skip past the run. (Earlier version pushed the run
                // itself, which appended the placeholder AFTER the words
                // — the bug surfaced in the mnemonic test.)
                out.push_str(&s[i..run_start]);
                out.push_str("<redacted-mnemonic>");
                i = j;
            } else {
                // Not enough words — emit char-by-char until we find
                // a non-lowercase boundary or exceed the budget.
                let mut j = i;
                let mut wc = 0;
                while j < bytes.len() && wc < MAX_WORDS {
                    while j < bytes.len() && bytes[j].is_ascii_lowercase() {
                        j += 1;
                    }
                    wc += 1;
                    if j == bytes.len()
                        || bytes[j] != b' '
                        || j + 1 >= bytes.len()
                        || !bytes[j + 1].is_ascii_lowercase()
                    {
                        break;
                    }
                    j += 1;
                }
                out.push_str(&s[i..j]);
                i = j;
            }
        }
        out
    }

    scrub_mnemonic(&s)
}

/// Run `f`, catching any panic and converting it to [`FfiError::Panic`].
/// The panic message is scrubbed (L12 CRITICAL #2) and recorded in the
/// thread-local error buffer so the Dart side can retrieve it via
/// `ffi_last_error_message()`.
pub fn ffi_catch_unwind<F>(f: F) -> FfiError
where
    F: FnOnce() -> FfiError + panic::UnwindSafe,
{
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(rc) => rc,
        Err(payload) => {
            // payload is `Box<dyn Any + Send>`. Downcast to `&str` (the
            // common case for `panic!("...")`); fall back to "unknown"
            // for non-string payloads.
            let raw = if let Some(s) = payload.downcast_ref::<&'static str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "rust panic in FFI (non-string payload)".to_string()
            };
            let scrubbed = scrub_panic_message(&raw);
            set_last_error(scrubbed);
            FfiError::Panic
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;

    #[test]
    fn no_panic_returns_normal_code() {
        let rc = ffi_catch_unwind(|| FfiError::Ok);
        assert_eq!(rc, FfiError::Ok);
    }

    #[test]
    fn panic_returns_panic_code() {
        let rc = ffi_catch_unwind(|| panic!("intentional test panic"));
        assert_eq!(rc, FfiError::Panic);
        // Error message should be in the thread-local buffer.
        let ptr = unsafe { super::super::error::ffi_last_error_message() };
        assert!(!ptr.is_null());
        let s = unsafe { std::ffi::CStr::from_ptr(ptr) };
        assert!(s.to_str().unwrap().contains("intentional test panic"));
    }

    #[test]
    fn panic_with_string_payload() {
        let s = String::from("owned panic");
        let rc = ffi_catch_unwind(|| panic!("{}", s));
        assert_eq!(rc, FfiError::Panic);
    }

    #[test]
    fn panic_inside_nested_call_propagates() {
        fn inner() -> FfiError {
            panic!("nested");
        }
        let rc = ffi_catch_unwind(inner);
        assert_eq!(rc, FfiError::Panic);
    }

    #[test]
    fn recovered_from_panic_can_continue() {
        // After a caught panic, the thread is still usable.
        let _ = ffi_catch_unwind(|| panic!("first"));
        let rc = ffi_catch_unwind(|| FfiError::Ok);
        assert_eq!(rc, FfiError::Ok);
    }

    // ---- L12 CRITICAL #2: panic messages must NOT leak secrets ----

    #[test]
    fn password_in_panic_message_is_scrubbed() {
        std::thread::spawn(|| {
            let _ = ffi_catch_unwind(|| panic!("decryption failed: password=hunter2 was wrong"));
            let ptr = unsafe { super::super::error::ffi_last_error_message() };
            let s = unsafe { std::ffi::CStr::from_ptr(ptr) };
            let msg = s.to_str().unwrap();
            assert!(msg.contains("password=<redacted>"), "got: {msg}");
            assert!(!msg.contains("hunter2"), "password leaked: {msg}");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn mnemonic_in_panic_message_is_scrubbed() {
        std::thread::spawn(|| {
            let _ = ffi_catch_unwind(|| {
                panic!(
                    "wallet create failed: mnemonic=abandon abandon abandon \
                     abandon abandon abandon abandon abandon abandon abandon \
                     abandon abandon about"
                )
            });
            let ptr = unsafe { super::super::error::ffi_last_error_message() };
            let s = unsafe { std::ffi::CStr::from_ptr(ptr) };
            let msg = s.to_str().unwrap();
            assert!(msg.contains("<redacted-mnemonic>"), "got: {msg}");
            assert!(!msg.contains("about"), "mnemonic leaked: {msg}");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn secret_kv_in_panic_message_is_scrubbed() {
        std::thread::spawn(|| {
            let _ = ffi_catch_unwind(|| panic!("API key: secret=abc123def456"));
            let ptr = unsafe { super::super::error::ffi_last_error_message() };
            let s = unsafe { std::ffi::CStr::from_ptr(ptr) };
            let msg = s.to_str().unwrap();
            assert!(msg.contains("secret=<redacted>"), "got: {msg}");
            assert!(!msg.contains("abc123def456"), "secret leaked: {msg}");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn non_secret_panic_message_preserved() {
        // A panic with NO secret-bearing content must pass through
        // unchanged (or with NUL→FFFD replacement only).
        std::thread::spawn(|| {
            let _ = ffi_catch_unwind(|| panic!("disk full on /dev/sda"));
            let ptr = unsafe { super::super::error::ffi_last_error_message() };
            let s = unsafe { std::ffi::CStr::from_ptr(ptr) };
            assert!(s.to_str().unwrap().contains("disk full"));
        })
        .join()
        .unwrap();
    }
}
