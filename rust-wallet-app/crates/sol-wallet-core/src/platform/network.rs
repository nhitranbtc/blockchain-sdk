//! `sol-wallet-core` — `NetworkClient` PAL.
//!
//! Phase 5 already implements the reqwest client in
//! ` crate::chain::client`. Phase 6.1 owns the trait shim so
//! `WalletManager` can call the network without depending on
//! Phase 5's reqwest types directly.

/// Minimal HTTP client trait used by `WalletManager` / Phase 7 CLI.
/// Phase 5's `RpcClient` implements this.
pub trait NetworkClient: Send + Sync {
    /// GET `url` and return the response body as bytes.
    fn get(&self, url: &str) -> std::result::Result<Vec<u8>, std::io::Error>;
    /// POST `url` with `body` and return the response body.
    fn post(&self, url: &str, body: &[u8]) -> std::result::Result<Vec<u8>, std::io::Error>;
}

/// Thin wrapper that delegates to `crate::chain::client` (Phase 5).
/// Currently stubbed with explicit V0.1 wire-up; Phase 7 CLI will
/// replace these `Err`s with a real forwarding call.
#[derive(Debug, Clone)]
pub struct ReqwestClient;

impl NetworkClient for ReqwestClient {
    fn get(&self, _url: &str) -> std::result::Result<Vec<u8>, std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "ReqwestClient::get — wire to chain::client::ReqwestClient in Phase 7",
        ))
    }
    fn post(&self, _url: &str, _body: &[u8]) -> std::result::Result<Vec<u8>, std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "ReqwestClient::post — wire to chain::client::ReqwestClient in Phase 7",
        ))
    }
}
