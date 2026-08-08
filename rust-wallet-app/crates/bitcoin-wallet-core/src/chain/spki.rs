//! SPKI pin: SHA-256 digest of a TLS server's `SubjectPublicKeyInfo` DER.
//!
//! Per F20 / B2 / U2: every Esplora TLS connection MUST verify the leaf
//! cert's SPKI hash against a configured pin (or pin set). This module
//! provides the typed pin primitive and the typed pin set used by
//! `TlsPolicy::Pinned`.
//!
//! **Threat-model coverage:**
//!
//! - F20 (SPKI pubkey pinning per U2) — defended by [`SpkiPinSet`]
//!   + [`SpkiPin::from_spki_der`] (hashes the full SPKI DER per RFC 7469).
//! - F43 (per-protocol error variant) — defended: `Error::SpkiPin`
//!   distinct from `Error::Esplora` so callers can distinguish config
//!   errors from runtime errors.
//!
//! **Drift from plan §Task 7** (L12 folded):
//!
//! | Plan said | This implementation | Why |
//! |---|---|---|
//! | `Option<SpkiPin>` always optional | `SpkiPinSet` (≥1 pin) wrapping `Vec<SpkiPin>` | `Option<SpkiPin>` defaults to CA-trust → defeats F20; pin set supports cert rotation (H-3) |
//! | `SpkiPin::from_base64` infallible length | Strict exact-32-byte check; `Error::SpkiPin` on mismatch | M-1: operator misconfig → unhelpful TLS-layer error |
//! | `from_sha256(bytes: [u8; 32])` | Renamed to `from_bytes` | TD-16: name overclaims (type can't verify SHA-256 origin) |
//! | No serde impl | Hand-written `Serialize`/`Deserialize` delegating to `from_base64` | TD-02: derived serde emits 32-byte array, breaks operator config |
//! | Raw `Option<[u8; 32]>` | Named private field `digest: [u8; 32]` | TD-17: tuple-struct field one keyword from `pub` |
//! | `Debug` hex | `Debug`/`Display` base64 (matches input format) | TD-15: copy-paste from Debug back into config works |

use std::fmt;
use std::str::FromStr;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// SHA-256 SPKI pin. The inner field is the digest of the full
/// `SubjectPublicKeyInfo` DER (algorithm identifier + subjectPublicKey
/// BIT STRING per RFC 7469), NOT just the raw key bytes.
///
/// `Copy` is safe — the inner `[u8; 32]` is small and the pin is
/// operator-public (its whole purpose is to be published in config).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpkiPin {
    digest: [u8; 32],
}

impl SpkiPin {
    /// Wrap a 32-byte digest. Caller asserts SHA-256 origin; this layer
    /// cannot verify it (renamed from `from_sha256` per TD-16).
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { digest: bytes }
    }

    /// Parse a base64-encoded SHA-256 SPKI pin. Accepts standard base64
    /// (NOT URL-safe). Decoded length must be exactly 32 bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SpkiPin`] if the input is not valid base64 or
    /// the decoded length is not exactly 32 bytes. The input string is
    /// NOT echoed in the error (avoid pin disclosure in logs).
    pub fn from_base64(s: &str) -> Result<Self> {
        let bytes = STANDARD
            .decode(s)
            .map_err(|e| Error::SpkiPin(format!("invalid base64: {e}")))?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|v: Vec<u8>| Error::SpkiPin(format!("expected 32 bytes, got {}", v.len())))?;
        Ok(Self { digest: bytes })
    }

    /// Hash the full `SubjectPublicKeyInfo` DER via SHA-256 and wrap.
    /// This is the path the cert verifier uses internally.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SpkiPin`]. Currently infallible; the `Result`
    /// return type is for forward compatibility with future SPKI parsers.
    pub fn from_spki_der(der: &[u8]) -> Result<Self> {
        let digest: [u8; 32] = Sha256::digest(der).into();
        Ok(Self { digest })
    }

    /// Borrow the inner 32-byte digest (read-only; no interior mutability).
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.digest
    }
}

impl fmt::Display for SpkiPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", STANDARD.encode(self.digest))
    }
}

impl fmt::Debug for SpkiPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SpkiPin({self})")
    }
}

impl FromStr for SpkiPin {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::from_base64(s)
    }
}

impl Serialize for SpkiPin {
    fn serialize<S: Serializer>(&self, ser: S) -> std::result::Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SpkiPin {
    fn deserialize<D: Deserializer<'de>>(de: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(de).map_err(<D::Error as serde::de::Error>::custom)?;
        Self::from_base64(&s).map_err(<D::Error as serde::de::Error>::custom)
    }
}

/// A set of one or more SPKI pins. Used to support cert rotation: an
/// operator can configure two pins (current + next) so the wallet
/// accepts either during the rollover window.
///
/// # Invariants
///
/// - Always contains at least one pin.
/// - Order is not significant for matching (any-pin-matches).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpkiPinSet(Vec<SpkiPin>);

impl SpkiPinSet {
    /// Construct a pin set from a non-empty `Vec`. Returns
    /// [`Error::SpkiPin`] if the vec is empty.
    ///
    /// # Errors
    ///
    /// `Error::SpkiPin("pin set must be non-empty")` on empty input.
    pub fn new(pins: Vec<SpkiPin>) -> Result<Self> {
        if pins.is_empty() {
            return Err(Error::SpkiPin("pin set must be non-empty".into()));
        }
        Ok(Self(pins))
    }

    /// Single-pin constructor (convenience).
    #[must_use]
    pub fn from_one(pin: SpkiPin) -> Self {
        Self(vec![pin])
    }

    /// Number of pins in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True if the set contains no pins. Should never be true after
    /// construction; provided for completeness.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over the pins.
    pub fn iter(&self) -> std::slice::Iter<'_, SpkiPin> {
        self.0.iter()
    }

    /// True if `candidate` matches any pin in the set (constant-time
    /// per-pin compare via `subtle::ConstantTimeEq`).
    #[must_use]
    pub fn matches(&self, candidate: &[u8; 32]) -> bool {
        let candidate_slice: &[u8] = candidate.as_slice();
        self.0.iter().any(|p| {
            let pin_slice: &[u8] = p.as_bytes().as_slice();
            bool::from(subtle::ConstantTimeEq::ct_eq(pin_slice, candidate_slice))
        })
    }
}

impl IntoIterator for SpkiPinSet {
    type Item = SpkiPin;
    type IntoIter = std::vec::IntoIter<SpkiPin>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Base64 of the SHA-256 of 32 zero bytes. Verified locally:
    /// `python3 -c "import hashlib, base64; print(base64.b64encode(hashlib.sha256(bytes(32)).digest()).decode())"`.
    const ZEROS_HASH_B64: &str = "Zmh6rfhivXdsj8GLjp+OIAiXFIVu4jOzkCpZHQ1fKSU=";

    /// Base64 of 32 zero bytes (Display/Deserialize tests).
    const ZEROS_BYTES_B64: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

    #[test]
    fn from_bytes_round_trip() {
        let pin = SpkiPin::from_bytes([0u8; 32]);
        assert_eq!(pin.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn from_base64_standard_zero_hash() {
        let pin = SpkiPin::from_base64(ZEROS_HASH_B64).unwrap();
        // Verify the SHA-256 of 32 zero bytes (not [0u8; 32] itself).
        let expected: [u8; 32] = [
            0x66, 0x68, 0x7a, 0xad, 0xf8, 0x62, 0xbd, 0x77, 0x6c, 0x8f, 0xc1, 0x8b, 0x8e, 0x9f,
            0x8e, 0x20, 0x08, 0x97, 0x14, 0x85, 0x6e, 0xe2, 0x33, 0xb3, 0x90, 0x2a, 0x59, 0x1d,
            0x0d, 0x5f, 0x29, 0x25,
        ];
        assert_eq!(pin.as_bytes(), &expected);
    }

    #[test]
    fn from_base64_rejects_invalid_base64() {
        let err = SpkiPin::from_base64("!!!not-base64!!!").unwrap_err();
        assert!(matches!(err, Error::SpkiPin(_)));
    }

    #[test]
    fn from_base64_rejects_wrong_length() {
        // 16 bytes => 24 base64 chars.
        let short = STANDARD.encode([0u8; 16]);
        let err = SpkiPin::from_base64(&short).unwrap_err();
        assert!(matches!(err, Error::SpkiPin(_)));
        assert!(err.to_string().contains("expected 32 bytes"));
    }

    #[test]
    fn from_base64_rejects_too_long() {
        // 64 bytes => 88 base64 chars.
        let long = STANDARD.encode([0u8; 64]);
        let err = SpkiPin::from_base64(&long).unwrap_err();
        assert!(matches!(err, Error::SpkiPin(_)));
    }

    #[test]
    fn from_spki_der_hashes_full_der() {
        // Distinct inputs produce distinct pins (smoke test that the
        // hash function is actually applied).
        let pin_a = SpkiPin::from_spki_der(b"foo").unwrap();
        let pin_b = SpkiPin::from_spki_der(b"bar").unwrap();
        assert_ne!(pin_a, pin_b);
    }

    #[test]
    fn from_spki_der_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let pin = SpkiPin::from_spki_der(b"").unwrap();
        let expected: [u8; 32] = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(pin.as_bytes(), &expected);
    }

    #[test]
    fn display_is_base64() {
        let pin = SpkiPin::from_bytes([0u8; 32]);
        assert_eq!(pin.to_string(), ZEROS_BYTES_B64);
    }

    #[test]
    fn debug_includes_base64() {
        let pin = SpkiPin::from_bytes([0u8; 32]);
        let dbg = format!("{pin:?}");
        assert!(dbg.contains("SpkiPin"));
        assert!(dbg.contains(ZEROS_BYTES_B64));
    }

    #[test]
    fn from_str_matches_from_base64() {
        let a = SpkiPin::from_str(ZEROS_HASH_B64).unwrap();
        let b = SpkiPin::from_base64(ZEROS_HASH_B64).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn serialize_emits_base64_string() {
        let pin = SpkiPin::from_bytes([0u8; 32]);
        let json = serde_json::to_string(&pin).unwrap();
        assert_eq!(json, format!("\"{ZEROS_BYTES_B64}\""));
    }

    #[test]
    fn deserialize_accepts_base64_string() {
        let json = format!("\"{ZEROS_BYTES_B64}\"");
        let pin: SpkiPin = serde_json::from_str(&json).unwrap();
        assert_eq!(pin.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn deserialize_rejects_32_byte_array() {
        // TD-02: derived serde would emit a 32-byte array. Hand-written
        // must reject it.
        let json = format!("[0{}]", ",0".repeat(31));
        let result: serde_json::Result<SpkiPin> = serde_json::from_str(&json);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_rejects_invalid_base64() {
        let json = "\"!!!not-base64!!!\"";
        let result: serde_json::Result<SpkiPin> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_rejects_wrong_length() {
        let short = STANDARD.encode([0u8; 16]);
        let json = format!("\"{short}\"");
        let result: serde_json::Result<SpkiPin> = serde_json::from_str(&json);
        assert!(result.is_err());
    }

    #[test]
    fn pin_set_new_rejects_empty() {
        let err = SpkiPinSet::new(vec![]).unwrap_err();
        assert!(matches!(err, Error::SpkiPin(_)));
    }

    #[test]
    fn pin_set_from_one_has_one() {
        let pin = SpkiPin::from_bytes([0u8; 32]);
        let set = SpkiPinSet::from_one(pin);
        assert_eq!(set.len(), 1);
        assert!(!set.is_empty());
    }

    #[test]
    fn pin_set_matches_returns_true_for_pinned() {
        let pin = SpkiPin::from_bytes([0x42u8; 32]);
        let set = SpkiPinSet::from_one(pin);
        assert!(set.matches(&[0x42u8; 32]));
    }

    #[test]
    fn pin_set_matches_returns_false_for_non_pinned() {
        let pin = SpkiPin::from_bytes([0x42u8; 32]);
        let set = SpkiPinSet::from_one(pin);
        assert!(!set.matches(&[0x00u8; 32]));
    }

    #[test]
    fn pin_set_matches_any_pin_for_rotation() {
        let pin_a = SpkiPin::from_bytes([0xaau8; 32]);
        let pin_b = SpkiPin::from_bytes([0xbbu8; 32]);
        let set = SpkiPinSet::new(vec![pin_a, pin_b]).unwrap();
        assert!(set.matches(&[0xaau8; 32]));
        assert!(set.matches(&[0xbbu8; 32]));
        assert!(!set.matches(&[0xccu8; 32]));
    }

    #[test]
    fn pin_set_iter_and_into_iter() {
        let pin_a = SpkiPin::from_bytes([0x01u8; 32]);
        let pin_b = SpkiPin::from_bytes([0x02u8; 32]);
        let set = SpkiPinSet::new(vec![pin_a, pin_b]).unwrap();
        let collected: Vec<SpkiPin> = set.iter().copied().collect();
        assert_eq!(collected, vec![pin_a, pin_b]);
        let set2 = SpkiPinSet::new(vec![pin_a, pin_b]).unwrap();
        let moved: Vec<SpkiPin> = set2.into_iter().collect();
        assert_eq!(moved, vec![pin_a, pin_b]);
    }
}
