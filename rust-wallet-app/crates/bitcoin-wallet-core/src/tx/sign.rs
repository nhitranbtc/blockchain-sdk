//! Transaction signing (Task 12 + 13, Story 5 / Issue #118).
//!
//! Wraps `bdk_wallet::Wallet::sign` (signs all PSBT inputs that the
//! wallet owns a private key for) and `Psbt::extract_tx` (finalizes
//! the signed PSBT into a broadcast-ready `Transaction`).
//!
//! **Security:**
//! - F25 (signing) — only path to ECDSA signing for tx inputs.
//! - F21 (typed Sighash) — upstream of bdk's PSBT handling; we trust
//!   bdk for the signature material binding (bdk internally calls
//!   `sighash::SighashCache` per input).
//! - `trust_witness_utxo: true` is the canonical default for SegWit
//!   inputs (per bdk docs); legacy inputs would need
//!   `trust_witness_utxo: false` + explicit UTXO lookup, but our
//!   wallet is BIP-84 (native segwit) only.
//!
//! **Error sanitization:** bdk's `SignerError` Display is safe (no
//! descriptor echo), but we wrap with `Error::Sign(...)` for type
//! discrimination.

use bdk_wallet::bitcoin::{Psbt, Transaction};
use bdk_wallet::SignOptions;
use bdk_wallet::Wallet as BdkWallet;

use crate::error::{Error, Result};

/// Sign all PSBT inputs for which the wallet holds a private key.
/// `psbt` is mutated in place — each input's `partial_sigs` is
/// populated with the ECDSA signature.
///
/// Uses [`SignOptions::default`] with `trust_witness_utxo = true`
/// (SegWit canonical). Legacy / non-SegWit input handling is out of
/// scope (the wallet is BIP-84 native-segwit only).
///
/// # Errors
///
/// - `Error::Sign` on bdk `SignerError` (missing key, invalid
///   sighash, etc.)
pub fn sign_psbt(bdk: &BdkWallet, psbt: &mut Psbt) -> Result<()> {
    let opts = SignOptions {
        trust_witness_utxo: true,
        ..Default::default()
    };
    bdk.sign(psbt, opts)
        .map_err(|e| Error::Sign(format!("sign failed: {e}")))?;
    Ok(())
}

/// Finalize a signed PSBT into a broadcast-ready [`Transaction`].
///
/// Consumes the PSBT (rust-bitcoin's `Psbt::extract_tx` takes
/// `self`; we clone internally so callers can pass `&Psbt`).
///
/// # Errors
///
/// - `Error::Psbt` on extraction failure (missing signatures,
///   invalid scriptSigs, etc.)
pub fn extract_tx(psbt: &Psbt) -> Result<Transaction> {
    psbt.clone()
        .extract_tx()
        .map_err(|e| Error::Psbt(format!("extract_tx failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-check + signature pin. Full sign + extract roundtrip
    /// coverage requires a funded bdk wallet and is exercised by the
    /// testcontainers regtest smoke (Issue #115 deferred) + L29 live
    /// testnet gates — not unit tests (the tprv placeholder strings
    /// are not valid 64/66/130-byte xpub material).
    #[test]
    fn sign_and_extract_signatures_compile() {
        let _: fn(&BdkWallet, &mut Psbt) -> Result<()> = sign_psbt;
        let _: fn(&Psbt) -> Result<Transaction> = extract_tx;
    }
}
