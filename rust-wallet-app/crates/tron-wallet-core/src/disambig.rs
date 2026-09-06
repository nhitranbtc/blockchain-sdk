//! Cross-network guards.
//!
//! These exist so that "I sent 1000 TRX to a Nile faucet while my wallet was
//! configured for mainnet" never results in a real-value transaction to
//! the wrong network. The plan's Q12 calls these out as compile-time
//! constants; we model them as runtime checks that fire at the boundary
//! where the value direction is known.
//!
//! ## Footguns
//!
//! - **Mainnet → Nile send.** Caller meant Mainnet, picked a Nile address
//!   out of an old note. The check refuses the transaction.
//! - **Nile → Mainnet send.** Less common but symmetric. Refused.
//! - **TRC-20 `transfer` to a non-Tron address.** The contract exists on
//!   Ethereum too — a malformed argument could route USDT to an EVM address.
//!   Refused.

use crate::config::Network;
use crate::error::{Error, Result};

/// A coarse network tag attached to an address at the point of import.
/// None of these checks can prove the address *belongs* to a network —
/// TRON address space is single-prefix `0x41` — but they can catch a
/// caller passing the wrong config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressNetwork {
    Mainnet,
    Nile,
    Shasta,
    Local,
}

/// Refuse a send if the address's recorded network does not match the
/// active network config. Used immediately before signing.
pub fn ensure_same_network(caller: Network, addr_recorded: AddressNetwork) -> Result<()> {
    let caller_tag = match caller {
        Network::Mainnet => AddressNetwork::Mainnet,
        Network::Nile => AddressNetwork::Nile,
        Network::Shasta => AddressNetwork::Shasta,
        Network::Local => AddressNetwork::Local,
    };
    if caller_tag != addr_recorded {
        return Err(Error::Disambiguation(format!(
            "active network is {caller:?}, address belongs to {addr_recorded:?}; refuse to sign"
        )));
    }
    Ok(())
}

/// Refuse a TRC-20 `transfer` that targets what looks like a non-Tron
/// address (the hex form would have a 0x prefix and 20 bytes; a T-base58
/// address has neither). Best-effort — a hand-built bytes payload can
/// still defeat this — but it catches the common paste-from-EVM-explorer
/// error.
pub fn ensure_tron_style_address(addr: &str) -> Result<()> {
    if addr.starts_with("0x") || addr.starts_with("0X") {
        return Err(Error::Disambiguation(format!(
            "recipient `{addr}` looks like an EVM address; refuse TRC-20 transfer"
        )));
    }
    if !addr.starts_with('T') {
        return Err(Error::Disambiguation(format!(
            "recipient `{addr}` is not a TRON T-base58check address; refuse TRC-20 transfer"
        )));
    }
    Ok(())
}

// Re-exported from `crate::config` — the canonical home is there.
/// The hex form was previously a `pub const MAINNET_SPKI_PIN_HEX`
/// in `crate::config`, but moved to `tokens/network.json` per the
/// Phase 3 config refactor (operator-tunable, no Rust recompile
/// needed for pin rotation). The re-export now points at the
/// function, so existing callers that did `use ... MAINNET_SPKI_PIN_HEX`
/// will see a compile error pointing them at `mainnet_spki_pin_hex()`.
pub use crate::config::mainnet_spki_pin;
pub use crate::config::mainnet_spki_pin_hex as MAINNET_SPKI_PIN_HEX;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_0x_recipients() {
        let err = ensure_tron_style_address("0xdAC17F958D2ee523a2206206994597C13D831ec7");
        assert!(matches!(err, Err(Error::Disambiguation(_))));
    }

    #[test]
    fn refuses_lowercase_recipients() {
        let err = ensure_tron_style_address("txyzopuvdm45dlt6eyceq8nx6fvf2hu1z");
        assert!(matches!(err, Err(Error::Disambiguation(_))));
    }

    #[test]
    fn accepts_t_prefixed() {
        ensure_tron_style_address("TG7jQ7eGsns6nmQNfcKNgZKyKBFkx7CvXr")
            .expect("T-base58 addresses are accepted");
    }

    #[test]
    fn mainnet_pin_hex_matches_runtime_default() {
        assert_eq!(
            hex::encode(mainnet_spki_pin().as_bytes()),
            MAINNET_SPKI_PIN_HEX()
        );
    }

    #[test]
    fn rejects_cross_network() {
        let err = ensure_same_network(Network::Mainnet, AddressNetwork::Nile);
        assert!(matches!(err, Err(Error::Disambiguation(_))));
    }
}
