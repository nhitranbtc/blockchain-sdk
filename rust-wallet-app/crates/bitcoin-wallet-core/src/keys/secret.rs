//! `Secret<T>`: zeroize-on-drop wrapper for sensitive in-memory values.
//!
//! Per F53: a single scoped `unsafe` block in `into_inner` uses
//! `std::ptr::read` + `mem::forget` to move out before `ZeroizeOnDrop`
//! fires. Crate-level `#![deny(unsafe_code)]` still applies; the
//! `#[allow(unsafe_code)]` is scoped to the statement containing the
//! unsafe block, not the whole method.

use zeroize::{Zeroize, ZeroizeOnDrop};

/// Wrapper that wipes its inner value on drop.
///
/// `ZeroizeOnDrop` derive generates the `Drop` impl that calls
/// `T::zeroize()` on the inner value.
#[derive(ZeroizeOnDrop)]
pub struct Secret<T: Zeroize>(T);

/// Manual `Debug` impl that hides the inner value. Per L17:
///
/// - Auto-deriving `Debug` would leak the secret via `{:?}` formatting,
///   defeating the whole point of the wrapper.
/// - `finish_non_exhaustive()` renders as `Secret { .. }` (no field
///   names). Per CONTEXT.md hard rule #7, this also avoids field-name
///   collisions with the BIP-39 wordlist (e.g. "secret", "inner",
///   "key" are all valid BIP-39 words).
impl<T: Zeroize> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secret").finish_non_exhaustive()
    }
}

impl<T: Zeroize> Secret<T> {
    /// Wrap a value. The wrapper takes ownership and zeros on drop.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Borrow the inner value. The borrow lifetime keeps the secret alive;
    /// the value cannot be moved or copied.
    pub fn expose(&self) -> &T {
        &self.0
    }

    /// Move the inner value out. Caller takes responsibility for zeroizing.
    ///
    /// Per F53: the `unsafe` is scoped to the block, not the method.
    /// Crate-level `#![deny(unsafe_code)]` still applies — we use a
    /// per-statement `#[allow(unsafe_code)]` to permit exactly this block.
    pub fn into_inner(self) -> T {
        #[allow(unsafe_code)]
        let v = unsafe { std::ptr::read(&self.0) };
        std::mem::forget(self);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_round_trip() {
        let s: Secret<Vec<u8>> = Secret::new(vec![1, 2, 3, 4]);
        assert_eq!(s.expose(), &vec![1, 2, 3, 4]);
    }

    #[test]
    fn secret_into_inner_returns_value() {
        let s: Secret<Vec<u8>> = Secret::new(vec![5, 6, 7, 8]);
        let v = s.into_inner();
        assert_eq!(v, vec![5, 6, 7, 8]);
    }
}
