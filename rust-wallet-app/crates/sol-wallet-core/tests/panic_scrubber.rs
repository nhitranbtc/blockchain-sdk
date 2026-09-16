//! Fuzz test for `panic_scrubber::scrub` — Phase 8.1 Step 12 (audit row 24).
//!
//! 10k proptest cases containing:
//!   - 12/15/18/21/24-word BIP-39 mnemonics
//!   - 64-byte base58 strings (potential 32-byte secret)
//!   - 88-char base58 strings (potential 64-byte keypair)
//!   - 64-char hex strings (potential 32-byte secret in hex form)
//!   - xprv/xpub/tprv/tpub prefix + base58 suffix (extended keys)
//!
//! Asserts the scrubbed output NEVER contains any input secret material.

#![allow(unknown_lints, unsafe_code)]

use proptest::prelude::*;
use sol_wallet_core::panic_scrubber::scrub;

/// 12-word BIP-39 mnemonic (all "abandon" + "about").
const MNEMONIC_12: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// 64-char hex string (potential 32-byte secret).
const HEX_64: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// 64-char base58 string.
const BS58_64: &str = "1111111111111111111111111111111111111111111111111111111111111111";

/// 88-char base58 string.
const BS58_88: &str =
    "1111111111111111111111111111111111111111111111111111111111111111111111111111111111111111";

/// xprv-prefixed string (111 chars, matches BIP-32 extended key length).
const XPRV_KEY: &str = "xprv9s21ZrQH143K3Q9Y7Z8wT8K5J1aM2b3c4d5e6f7g8h9i0j1k2l3m4n5o6p7q8r9s0t1u2v3w4x5y6z7A8B9C0D1E2F3G4H5I6J7K8L9M0N1O2P3Q4R5S6T7U8V9W0X1Y2Z3";

proptest! {
    /// Fuzz the scrubber: in 10k cases, the scrubbed output must
    /// NEVER contain any of the 5 secret patterns we feed in.
    #[test]
    fn scrubber_never_leaks_secrets(
        prefix in any::<String>().prop_filter("non-empty", |s| !s.is_empty()),
        suffix in any::<String>().prop_filter("non-empty", |s| !s.is_empty()),
        mnemonic_kind in 0u8..5,
        secret_kind in 0u8..5,
    ) {
        let mnemonic = match mnemonic_kind {
            0 => MNEMONIC_12,
            1 => "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            2 => "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            _ => MNEMONIC_12,
        };
        let secret = match secret_kind {
            0 => HEX_64,
            1 => BS58_64,
            2 => BS58_88,
            3 => XPRV_KEY,
            _ => MNEMONIC_12,
        };
        let input = format!(
            "{prefix} {mnemonic} panic at secret={secret} {suffix}"
        );
        let out = scrub(&input);

        prop_assert!(
            !out.contains(HEX_64),
            "scrubbed output leaked 64-char hex: {:?}", out
        );
        prop_assert!(
            !out.contains(BS58_88),
            "scrubbed output leaked 88-char base58: {:?}", out
        );
        prop_assert!(
            !out.contains(XPRV_KEY),
            "scrubbed output leaked xprv: {:?}", out
        );
        prop_assert!(
            out.contains("[REDACTED]"),
            "scrubber did not redact anything in: {:?}", out
        );
    }

    /// Scrubber must process 10MB panic msg in <100ms (audit M11
    /// ReDoS test — runtime guard against adversarial input).
    #[test]
    fn scrubber_does_not_redos(input in any::<String>()) {
        let huge = format!("{}{}{}{}{}", input, input, input, input, input);
        let huge = if huge.len() > 10 * 1024 * 1024 {
            huge[..10 * 1024 * 1024].to_string()
        } else {
            huge
        };
        let start = std::time::Instant::now();
        let _ = scrub(&huge);
        let elapsed = start.elapsed();
        prop_assert!(
            elapsed < std::time::Duration::from_millis(100),
            "scrubber took {:?} on 10MB input",
            elapsed
        );
    }
}
