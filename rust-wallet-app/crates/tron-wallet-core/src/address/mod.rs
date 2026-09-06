//! T-address handling, wrapping `anychain_tron::TronAddress` (plan Task 1.3).
//!
//! A TRON address is 21 bytes: the `0x41` type prefix plus the low 20 bytes of
//! `keccak256` over the uncompressed public key. Displayed, it is base58check
//! over those 21 bytes, which is why every address begins with `T`.

use anychain_core::Address as _;
use anychain_tron::{TronAddress, TronFormat, TronPublicKey};
use core::fmt;
use core::str::FromStr;

use crate::error::{Error, Result};

/// Number of raw bytes in a TRON address: `0x41` plus 20 account bytes.
pub const ADDRESS_LEN: usize = 21;

/// A validated TRON address.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Address(TronAddress);

impl Address {
    /// Derives the address for a public key.
    pub fn from_public_key(public_key: &TronPublicKey) -> Result<Self> {
        TronAddress::from_public_key(public_key, &TronFormat::Standard)
            .map(Self)
            .map_err(|e| Error::Address(e.to_string()))
    }

    /// Base58check form — the `T...` string users copy and paste.
    pub fn to_base58(&self) -> String {
        self.0.to_base58()
    }

    /// Uppercase hex of the 21 raw bytes, starting `41`. This is the form the
    /// TRON HTTP API expects in most request bodies.
    pub fn to_hex(&self) -> String {
        self.0.to_hex()
    }

    /// The 21 raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Reports whether `candidate` parses as a TRON address.
    ///
    /// This is an associated function rather than the `&self` predicate the
    /// plan sketched: on a constructed [`Address`] the answer is always `true`,
    /// so a method would be a check that can never fail. What callers actually
    /// need is to screen untrusted input — a pasted recipient — before
    /// building one.
    pub fn is_valid(candidate: &str) -> bool {
        candidate.parse::<Self>().is_ok()
    }

    /// Access to the wrapped type, for the transaction builders in later
    /// phases that take `anychain` types directly.
    pub fn as_tron_address(&self) -> &TronAddress {
        &self.0
    }
}

/// Longest input any accepted form can have: `0x` plus 42 hex characters.
/// Anything longer is rejected before base58 decoding, so screening a pasted
/// recipient cannot be turned into unbounded work.
const MAX_ADDRESS_INPUT_LEN: usize = 44;

/// Inputs `anychain_tron::TronAddress::from_str` silently maps to the
/// all-zero burn address `T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb`.
///
/// Upstream treats these as a "null address" shorthand. That is indefensible
/// for a wallet: [`Address::is_valid`] exists to screen a pasted recipient, and
/// a stray `_` surviving that screen would send funds somewhere unrecoverable.
/// They are rejected here, and `rejects_upstream_burn_address_aliases` fails if
/// an `anychain` bump ever adds another one.
const UPSTREAM_BURN_ALIASES: [&str; 3] = ["_", "0x0", "/0"];

impl FromStr for Address {
    type Err = Error;

    /// Accepts base58check (`T...`), 42-char hex (`41...`), and `0x`-prefixed
    /// hex — the three forms `anychain_tron` recognises, minus its burn-address
    /// shorthands.
    fn from_str(s: &str) -> Result<Self> {
        if s.len() > MAX_ADDRESS_INPUT_LEN {
            return Err(Error::Address(format!(
                "input is {} characters; no TRON address form exceeds {MAX_ADDRESS_INPUT_LEN}",
                s.len()
            )));
        }

        if UPSTREAM_BURN_ALIASES.contains(&s) {
            return Err(Error::Address(
                "input is an anychain shorthand for the all-zero burn address, \
                 not a spendable recipient"
                    .to_string(),
            ));
        }

        // The input is not echoed back: this is the parser untrusted strings
        // reach, and an error message is the kind of thing that ends up in a
        // log line.
        TronAddress::from_str(s).map(Self).map_err(|e| {
            Error::Address(format!("not a TRON address ({} characters): {e}", s.len()))
        })
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_base58())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Address").field(&self.to_base58()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Published `anychain-tron` fixture (its `src/address.rs` unit tests).
    const KNOWN: &str = "TPhiVyQZ5xyvVK2KS2LTke8YvXJU5wxnbN";
    const KNOWN_HEX: &str = "4196a3bace5adacf637eb7cc79d5787f4247da4bbe";

    #[test]
    fn parses_the_published_fixture() {
        let address: Address = KNOWN.parse().expect("valid address");

        assert_eq!(address.to_base58(), KNOWN);
        assert_eq!(address.to_hex().to_lowercase(), KNOWN_HEX);
        assert_eq!(address.as_bytes().len(), ADDRESS_LEN);
    }

    #[test]
    fn base58_and_hex_forms_agree() {
        let from_base58: Address = KNOWN.parse().expect("valid address");
        let from_hex: Address = KNOWN_HEX.parse().expect("valid address");
        let from_0x: Address = format!("0x{KNOWN_HEX}").parse().expect("valid address");

        assert_eq!(from_base58, from_hex);
        assert_eq!(from_base58, from_0x);
    }

    #[test]
    fn display_renders_base58() {
        let address: Address = KNOWN.parse().expect("valid address");

        assert_eq!(address.to_string(), KNOWN);
        assert_eq!(format!("{address:?}"), format!("Address({KNOWN:?})"));
    }

    #[test]
    fn rejects_malformed_input() {
        for candidate in [
            "",
            "T",
            "TPhiVyQZ5xyvVK2KS2LTke8YvXJU5wxnb", // one char short
            "TPhiVyQZ5xyvVK2KS2LTke8YvXJU5wxnbNN", // one char long
            "0OIl0OIl0OIl0OIl0OIl0OIl0OIl0OIl00", // not in the base58 alphabet
        ] {
            assert!(
                !Address::is_valid(candidate),
                "should have rejected {candidate:?}"
            );
        }
    }

    #[test]
    fn rejects_a_mutated_checksum() {
        let mut chars: Vec<char> = KNOWN.chars().collect();
        chars[5] = if chars[5] == 'Z' { 'Y' } else { 'Z' };
        let mutated: String = chars.into_iter().collect();

        assert!(!Address::is_valid(&mutated), "accepted {mutated}");
    }

    #[test]
    fn rejects_a_non_tron_version_byte() {
        // Same 20 account bytes, Bitcoin's 0x00 version instead of 0x41.
        let wrong_prefix = format!("00{}", &KNOWN_HEX[2..]);

        assert!(!Address::is_valid(&wrong_prefix), "accepted {wrong_prefix}");
        // Control: the identical payload under 0x41 does parse, so the
        // rejection above is about the version byte and not the payload.
        assert!(Address::is_valid(KNOWN_HEX));
    }

    /// `anychain_tron::TronAddress::from_str` maps `"_"`, `"0x0"`, and `"/0"`
    /// to the all-zero burn address. `is_valid` is documented as the screen for
    /// a pasted recipient, so letting these through would mean a stray
    /// character routes funds somewhere unrecoverable.
    #[test]
    fn rejects_upstream_burn_address_aliases() {
        for alias in UPSTREAM_BURN_ALIASES {
            assert!(
                !Address::is_valid(alias),
                "accepted burn-address alias {alias:?}"
            );
        }
    }

    /// The burn address itself stays parseable — it is a real address, and
    /// refusing it would break reading historical transactions. Only the
    /// shorthand spellings are refused.
    #[test]
    fn the_burn_address_itself_still_parses() {
        let burn_hex = "410000000000000000000000000000000000000000";
        let burn: Address = burn_hex.parse().expect("burn address is a real address");

        assert_eq!(burn.to_base58(), "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb");
    }

    #[test]
    fn rejects_absurdly_long_input_without_decoding_it() {
        let huge = "T".repeat(100_000);

        assert!(!Address::is_valid(&huge));
    }

    #[test]
    fn parse_errors_do_not_echo_the_input() {
        // Error strings reach logs. An address is public, but this is the
        // parser untrusted input hits, so it should not quote it back.
        let err = "definitely-not-an-address".parse::<Address>().unwrap_err();

        assert!(
            !err.to_string().contains("definitely-not-an-address"),
            "error echoed its input: {err}"
        );
    }

    #[test]
    fn is_valid_accepts_every_recognised_form() {
        assert!(Address::is_valid(KNOWN));
        assert!(Address::is_valid(KNOWN_HEX));
        assert!(Address::is_valid(&format!("0x{KNOWN_HEX}")));
    }
}
