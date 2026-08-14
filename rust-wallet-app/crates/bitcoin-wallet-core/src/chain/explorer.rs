//! Block explorer URL builders (Task 27 / Story 7 / Issue #120).
//!
//! Pure URL formatting — no HTTP, no parsing. The base URL is just
//! a string (e.g., `"https://blockstream.info/testnet/api"`); the
//! path component is appended with a leading `/`.
//!
//! **Out of scope:** fetching from blockstream/mempool/etc. directly.
//! Story 7 displays URLs the operator can click in their browser
//! after running `btc tx list` (which enumerates via the existing
//! EsploraClient).
//!
//! **Threat-model coverage:** none (read-only URL builder; no I/O).

use bitcoin::{Address, Txid};

/// Format a block-explorer URL for a single tx:
/// `<base>/tx/<txid>`. Trailing `/api` is preserved; the builder
/// appends `/tx/<txid>` directly.
///
/// # Examples
///
/// ```
/// use bitcoin_wallet_core::chain::explorer::tx_url;
/// let url = tx_url("https://blockstream.info/testnet/api", "deadbeef".parse().unwrap());
/// assert_eq!(url, "https://blockstream.info/testnet/api/tx/deadbeef");
/// ```
pub fn tx_url(base: &str, txid: Txid) -> String {
    format!("{base}/tx/{txid}")
}

/// Format a block-explorer URL for an address:
/// `<base>/address/<address>`. Mirrors [`tx_url`] for symmetry.
pub fn address_url(base: &str, address: &Address) -> String {
    format!("{base}/address/{address}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tx_url_appends_path() {
        let txid: Txid = "0000000000000000000000000000000000000000000000000000000000000001"
            .parse()
            .unwrap();
        assert_eq!(
            tx_url("https://blockstream.info/testnet/api", txid),
            "https://blockstream.info/testnet/api/tx/0000000000000000000000000000000000000000000000000000000000000001"
        );
    }

    #[test]
    fn tx_url_preserves_trailing_slash() {
        let txid: Txid = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
            .parse()
            .unwrap();
        // Caller is responsible for trailing-slash normalization;
        // this function preserves whatever it gets.
        assert!(tx_url("https://example.com/", txid).contains("/tx/"));
    }

    #[test]
    fn address_url_appends_path() {
        let addr: Address = "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx"
            .parse::<bdk_wallet::bitcoin::Address<_>>()
            .unwrap()
            .require_network(bdk_wallet::bitcoin::Network::Testnet)
            .unwrap();
        assert!(address_url("https://blockstream.info/testnet/api", &addr)
            .starts_with("https://blockstream.info/testnet/api/address/tb1"));
    }
}
