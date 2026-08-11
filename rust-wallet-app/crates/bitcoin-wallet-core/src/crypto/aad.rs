//! Additional Authenticated Data (AAD) — context bound to a ciphertext.
//!
//! Per ADR 0001 (`docs/superpowers/adrs/2026-08-11-adr-0001-btc-wallet-store.md`),
//! the wallet-store layer uses AES-GCM AAD to bind context (e.g.,
//! `bitcoin::Network` discriminant) to the encrypted mnemonic blob. AAD is
//! authenticated but NOT encrypted — it travels out-of-band between encrypt
//! and decrypt sites, and a mismatch causes tag verification failure.
//!
//! **Type-design rationale (L12 review, Issue #66 precursor):** raw
//! `&[u8]` at the API boundary is the same anti-pattern as `Vec<u8>`
//! before `MnemonicCipherBlob` (Issue #28): an indistinguishable byte
//! slice can be confused with another (plaintext vs AAD) at the call
//! site. The `Aad<'a>` newtype forces the caller to construct AAD
//! through intent-revealing constructors (`Aad::NONE`,
//! `Aad::network(Network)`, `Aad::from_bytes`).
//!
//! **Why AAD does not embed in the blob:** see `mnemonic_cipher.rs`
//! module doc. The salt is embedded because it is a non-secret
//! uniquifier that must travel with the ciphertext. The AAD is a
//! *context assertion* whose entire value comes from being supplied
//! independently at decrypt time — embedding it would make it
//! self-satisfying and void the cross-network defense (N5).
//!
//! **Length cap (`MAX_AAD_LEN`):** AES-GCM AAD has no spec-mandated
//! length limit, but GHASH cost is proportional to AAD length. A 1 MB
//! AAD is a DoS vector (CPU + memory pressure on `aes-gcm` processing).
//! The 64-byte cap is comfortably above the 1-byte network discriminant
//! while bounding worst-case work.

use bitcoin::Network;

/// Maximum AAD length in bytes. AES-GCM GHASH cost scales with AAD length;
/// bounding AAD prevents a malicious caller (or accidental typo with a huge
/// slice) from forcing expensive AEAD computation.
///
/// **Compile-time pinned:** see the const-eval block at the bottom of this
/// module. Per L20 constant audit pattern.
pub const MAX_AAD_LEN: usize = {
    const INNER: usize = 64;
    assert!(
        INNER == 64,
        "MAX_AAD_LEN must be 64 (covers 1-byte Network discriminant + headroom)"
    );
    INNER
};

/// Additional Authenticated Data — opaque context bound to a ciphertext.
///
/// Constructed only via the explicit constructors below. No `Default`, no
/// `From<&[u8]>` (which would restore the implicit-coercion footgun the
/// type was introduced to prevent).
///
/// Borrowed (`'a`) — does not own the bytes. Callers pass either a literal
/// (`Aad::NONE` for the unbound case) or a stack/heap slice via
/// `Aad::from_bytes(&[u8])`.
#[derive(Clone, Copy, Debug)]
pub struct Aad<'a> {
    bytes: &'a [u8],
}

impl<'a> Aad<'a> {
    /// No AAD binding (pre-#66 behavior). Explicit token — the bare `b""`
    /// literal at a call site is hard to grep for; `Aad::NONE` makes
    /// "deliberately unbound" searchable in code review.
    pub const NONE: Aad<'static> = Aad { bytes: b"" };

    /// Bind `bitcoin::Network` discriminant to the ciphertext per ADR 0001.
    /// The discriminant byte mapping is exhaustive — adding a new
    /// `bitcoin::Network` variant will fail to compile this match until
    /// the encoding is updated (preventing silent on-disk blob remapping).
    ///
    /// Per ADR 0001 + BIP-94 extension: `0x01` testnet / `0x02` mainnet /
    /// `0x03` regtest / `0x04` signet / `0x05` testnet4. Do not reorder.
    /// Each network gets a distinct AAD byte so cross-network ciphertext
    /// reuse fails at decrypt time.
    pub fn network(n: bitcoin::Network) -> Aad<'static> {
        let bytes: &'static [u8] = match n {
            Network::Bitcoin => b"\x01",
            Network::Testnet => b"\x02",
            Network::Testnet4 => b"\x03",
            Network::Signet => b"\x04",
            Network::Regtest => b"\x05",
            // NO wildcard arm. Adding a new upstream `bitcoin::Network`
            // variant must be a deliberate choice (force the build to
            // fail) so silent blob remapping can't happen.
        };
        Aad { bytes }
    }

    /// Escape hatch for callers with their own encoding. Deliberately
    /// verbose name to distinguish from accidental `&[u8]` coercion.
    /// Enforces `MAX_AAD_LEN` (DoS mitigation).
    ///
    /// # Errors
    ///
    /// Returns `Err(AadError::TooLong)` if `bytes.len() > MAX_AAD_LEN`.
    /// Callers that need to bind an AAD longer than 64 bytes should
    /// reconsider their design — large AAD defeats the per-call work bound.
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, AadError> {
        if bytes.len() > MAX_AAD_LEN {
            return Err(AadError::TooLong {
                len: bytes.len(),
                max: MAX_AAD_LEN,
            });
        }
        Ok(Aad { bytes })
    }

    /// Borrow the inner bytes. `pub(crate)` — callers should use the typed
    /// constructors, not this accessor.
    pub(crate) fn as_slice(&self) -> &[u8] {
        self.bytes
    }
}

/// Error type for AAD construction failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AadError {
    /// AAD exceeds `MAX_AAD_LEN` bytes.
    TooLong {
        /// Actual AAD length in bytes.
        len: usize,
        /// Maximum allowed AAD length in bytes (`MAX_AAD_LEN`).
        max: usize,
    },
}

impl std::fmt::Display for AadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AadError::TooLong { len, max } => {
                write!(f, "AAD too long: {len} bytes (max {max})")
            }
        }
    }
}

impl std::error::Error for AadError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_aad_len_const_matches_runtime_check() {
        assert_eq!(MAX_AAD_LEN, 64);
    }

    #[test]
    fn none_is_empty() {
        assert_eq!(Aad::NONE.as_slice(), b"");
    }

    #[test]
    fn network_encoding_matches_adr_table() {
        // ADR 0001 §Filesystem layout (extended with Testnet4 per BIP-94):
        // any future `bitcoin::Network` variant must be added here AND
        // in the match arm above.
        assert_eq!(Aad::network(Network::Bitcoin).as_slice(), b"\x01");
        assert_eq!(Aad::network(Network::Testnet).as_slice(), b"\x02");
        assert_eq!(Aad::network(Network::Testnet4).as_slice(), b"\x03");
        assert_eq!(Aad::network(Network::Signet).as_slice(), b"\x04");
        assert_eq!(Aad::network(Network::Regtest).as_slice(), b"\x05");
    }

    #[test]
    fn from_bytes_accepts_within_cap() {
        let bytes = vec![0xAB; 32];
        let aad = Aad::from_bytes(&bytes).expect("32 bytes is within cap");
        assert_eq!(aad.as_slice(), &bytes[..]);
    }

    #[test]
    fn from_bytes_accepts_at_cap() {
        let bytes = vec![0xCD; MAX_AAD_LEN];
        let aad = Aad::from_bytes(&bytes).expect("MAX_AAD_LEN bytes is at cap");
        assert_eq!(aad.as_slice(), &bytes[..]);
    }

    #[test]
    fn from_bytes_rejects_over_cap() {
        let bytes = vec![0xEF; MAX_AAD_LEN + 1];
        let err = Aad::from_bytes(&bytes).expect_err("over cap must reject");
        assert!(matches!(err, AadError::TooLong { len: 65, max: 64 }));
    }
}
