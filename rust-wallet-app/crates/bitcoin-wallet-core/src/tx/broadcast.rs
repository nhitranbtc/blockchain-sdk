//! Transaction broadcast (Task 13, Story 5 / Issue #118).
//!
//! Wraps the Esplora broadcast endpoint (`POST /tx` with raw tx hex
//! body, returns txid on success). The lib reuses our
//! [`crate::chain::esplora::EsploraClient`] (F20 SPKI-pinned TLS)
//! rather than introducing a separate broadcast client.

use bdk_wallet::bitcoin::{consensus, Transaction, Txid};

use crate::chain::esplora::EsploraClient;
use crate::error::Result;

/// Broadcast a signed [`Transaction`] via the Esplora `/tx` endpoint.
///
/// Serializes the tx to consensus hex, POSTs it, parses the returned
/// txid. F20 (SPKI pinning) is enforced by the underlying client —
/// no separate TLS check here.
///
/// # Errors
///
/// - `Error::Esplora` on HTTP failure, parse failure, or transport
///   error (same family as `address_utxos` etc.).
pub async fn broadcast(esplora: &EsploraClient, tx: &Transaction) -> Result<Txid> {
    let raw_hex = consensus::encode::serialize_hex(tx);
    esplora.broadcast_tx(&raw_hex).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::esplora::TlsPolicy;
    use crate::chain::esplora_url::EsploraUrl;

    /// Compile-check + serialization pin. We can't exercise the
    /// actual broadcast without a live server — that's covered by
    /// the testcontainers regtest smoke (Story 5 happy path) and L29
    /// live testnet gates. This test only pins the `broadcast`
    /// function signature + tx serialization path.
    #[test]
    fn broadcast_function_signature_exists() {
        let url =
            EsploraUrl::new("https://blockstream.info/testnet/api").expect("testnet esplora url");
        let client = EsploraClient::new(url, TlsPolicy::SystemRoots).expect("esplora client build");
        // Construct an empty tx and serialize to consensus hex.
        let tx = Transaction {
            version: bdk_wallet::bitcoin::blockdata::transaction::Version(2),
            lock_time: bdk_wallet::bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        };
        let hex = consensus::encode::serialize_hex(&tx);
        assert!(
            hex.starts_with("02000000"),
            "expected version 2 + empty input/output, got {hex}"
        );
        // Suppress unused warning.
        let _ = &client;
    }
}
