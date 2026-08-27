//! SPKI pin wrapper (Q7).
//!
//! Plan §Q7: reuse `bitcoin_wallet_core::chain::spki` SPKI pin primitives verbatim.
//!
//! **Drift note (2026-08-27, Issue #403 spike):** plan called the surface
//! `SpkiPinnedVerifier`, but the actual public symbol is `SpkiPin` + `SpkiPinSet`
//! (F20 typed primitives). `EsploraVerifier` (the impl of
//! `rustls::ServerCertVerifier`) is private. Spike uses the public surface;
//! production wires it via `SpkiPinSet::new(vec![pin])` passed to the verifier.

pub use bitcoin_wallet_core::chain::spki::{SpkiPin, SpkiPinSet};

/// Parse a `pinned://<spki-sha256-hex>@host[:port]` URL into `(pin_hex, host, port)`.
pub fn parse_pinned_url(url: &str) -> Result<(String, String, u16), ParseError> {
    let rest = url.strip_prefix("pinned://").ok_or(ParseError::BadScheme)?;
    let (pin_and_host, port) = match rest.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>().map_err(|_| ParseError::BadPort)?,
        ),
        None => (rest.to_string(), 443),
    };
    let (pin, host) = pin_and_host.rsplit_once('@').ok_or(ParseError::NoAt)?;
    Ok((pin.to_string(), host.to_string(), port))
}

/// Build a single-pin `SpkiPinSet` from a 32-byte raw SPKI SHA-256.
pub fn pin_set_from_bytes(bytes: [u8; 32]) -> Result<SpkiPinSet, ParseError> {
    let pin = SpkiPin::from_bytes(bytes);
    SpkiPinSet::new(vec![pin]).map_err(|_| ParseError::BadPin)
}

/// Build a single-pin `SpkiPinSet` from a 64-char hex-encoded SPKI SHA-256 (the
/// `pinned://<pin>@host` URL format per plan §Q7).
pub fn pin_set_from_hex(pin_hex: &str) -> Result<SpkiPinSet, ParseError> {
    let raw = hex::decode(pin_hex).map_err(|_| ParseError::BadPin)?;
    if raw.len() != 32 {
        return Err(ParseError::BadPin);
    }
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&raw);
    pin_set_from_bytes(bytes)
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    BadScheme,
    BadPort,
    NoAt,
    BadPin,
}
