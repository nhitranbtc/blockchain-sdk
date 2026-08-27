//! Nile/mainnet JSON-RPC client (Q6).
//!
//! Plan §Q6: `eth_chainId` JSON-RPC method via TronGrid `/jsonrpc` returns
//! `0xcd8690dc` for Nile. `wallet/getchainid` returns HTTP 405 on TronGrid's HTTP
//! front, so the `/jsonrpc` path is required.

use serde::{Deserialize, Serialize};

/// Nile chain-id per plan §Q6 (corrected 2026-08-27 — prior doc had Shasta's
/// chain-id `0x94a9059e`).
pub const NILE_CHAIN_ID_HEX: &str = "0xcd8690dc";

/// JSON-RPC request envelope.
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest<'a> {
    pub jsonrpc: &'a str,
    pub method: &'a str,
    pub params: serde_json::Value,
    pub id: u32,
}

/// JSON-RPC response envelope.
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: u32,
    #[serde(default)]
    pub result: Option<T>,
    #[serde(default)]
    pub error: Option<serde_json::Value>,
}

/// SPKI-pinned JSON-RPC client to a TronGrid endpoint (Nile or mainnet).
///
/// Constructed from a `pinned://<pin-hex>@host[:port]` URL per plan §Q7. The
/// pin is the SHA-256 of the DER-encoded SubjectPublicKeyInfo of the host's
/// TLS certificate, lower-case hex. All outbound HTTP requests pass through
/// the SPKI pin verifier (V7 cross-crate reuse).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonRpcClient {
    pub host: String,
    pub port: u16,
    pub pin_hex: String,
}

/// Errors that can surface from `JsonRpcClient::new_pinned`.
#[derive(Debug, PartialEq, Eq)]
pub enum ClientParseError {
    /// URL did not begin with `pinned://`.
    BadScheme,
    /// URL had no `@` separating pin from host.
    MissingPinSeparator,
    /// Pin section was empty.
    EmptyPin,
    /// Pin was not 64 lowercase hex chars (32-byte SHA-256).
    BadPin,
    /// Host:port section was empty.
    EmptyHost,
    /// Port was not a valid `u16`.
    BadPort,
}

impl std::fmt::Display for ClientParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::BadScheme => "URL must start with `pinned://`",
            Self::MissingPinSeparator => "URL must contain `@` separating pin from host",
            Self::EmptyPin => "pin section before `@` must be non-empty",
            Self::BadPin => "pin must be 64 lowercase hex chars (SHA-256 of SPKI)",
            Self::EmptyHost => "host section after `@` must be non-empty",
            Self::BadPort => "port must be a valid u16",
        };
        f.write_str(s)
    }
}

impl std::error::Error for ClientParseError {}

/// Errors that can surface from JSON-RPC HTTP calls through `JsonRpcClient`.
#[derive(Debug)]
pub enum JsonRpcError {
    /// Underlying reqwest transport failure (DNS, TCP, TLS handshake).
    Transport(reqwest::Error),
    /// HTTP returned non-2xx.
    HttpStatus(u16),
    /// Response body was not valid JSON, or did not match the expected shape.
    Decode(serde_json::Error),
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "transport: {e}"),
            Self::HttpStatus(s) => write!(f, "http status {s}"),
            Self::Decode(e) => write!(f, "decode: {e}"),
        }
    }
}

impl std::error::Error for JsonRpcError {}

impl From<reqwest::Error> for JsonRpcError {
    fn from(e: reqwest::Error) -> Self {
        Self::Transport(e)
    }
}

impl From<serde_json::Error> for JsonRpcError {
    fn from(e: serde_json::Error) -> Self {
        Self::Decode(e)
    }
}

impl JsonRpcClient {
    /// Parse a `pinned://<pin-hex>@host[:port]` URL and return a client handle.
    ///
    /// Cycle 1: pure construction — no network. The returned struct holds
    /// the parsed components. HTTP wiring lands in cycle 3 (here).
    ///
    /// Note: SPKI pinning is recorded but **not yet enforced** on outbound
    /// HTTP — `post_trc20_constant` uses rustls default verification. Wiring
    /// the SPKI verifier (`bitcoin_wallet_core::chain::spki::SpkiPinnedVerifier`)
    /// into a custom reqwest `ClientBuilder` is follow-up work (issue #408
    /// ship-gate item).
    pub fn new_pinned(url: &str) -> Result<Self, ClientParseError> {
        let rest = url
            .strip_prefix("pinned://")
            .ok_or(ClientParseError::BadScheme)?;

        let (pin_hex, host_port) = rest
            .split_once('@')
            .ok_or(ClientParseError::MissingPinSeparator)?;

        if pin_hex.is_empty() {
            return Err(ClientParseError::EmptyPin);
        }
        if pin_hex.len() != 64
            || !pin_hex
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(ClientParseError::BadPin);
        }

        if host_port.is_empty() {
            return Err(ClientParseError::EmptyHost);
        }

        let (host, port) = match host_port.rsplit_once(':') {
            Some((h, p)) => {
                let port: u16 = p.parse().map_err(|_| ClientParseError::BadPort)?;
                (h.to_string(), port)
            }
            None => (host_port.to_string(), 443),
        };

        Ok(Self {
            host,
            port,
            pin_hex: pin_hex.to_string(),
        })
    }

    /// Base URL with scheme. Defaults to HTTPS since TronGrid endpoints are TLS.
    pub fn base_url(&self) -> String {
        format!("https://{}:{}", self.host, self.port)
    }

    /// POST `/wallet/triggerconstantcontract` with the given JSON body and
    /// decode the response into `T`. Used for read-only TRC-20 queries
    /// (`balanceOf`, `decimals`, etc.) — does NOT broadcast a transaction.
    pub async fn post_trc20_constant<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<T, JsonRpcError> {
        let url = format!("{}/wallet/triggerconstantcontract", self.base_url());
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        let resp = client.post(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(JsonRpcError::HttpStatus(status.as_u16()));
        }
        let text = resp.text().await?;
        Ok(serde_json::from_str(&text)?)
    }

    /// POST `/wallet/triggersmartcontract` with the given JSON body and
    /// decode the response into `T`. Used for state-changing TRC-20 calls
    /// (`transfer`, `approve`, etc.) — returns the unsigned envelope that
    /// the caller must sign locally before broadcasting.
    pub async fn post_triggersmartcontract<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<T, JsonRpcError> {
        let url = format!("{}/wallet/triggersmartcontract", self.base_url());
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        let resp = client.post(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(JsonRpcError::HttpStatus(status.as_u16()));
        }
        let text = resp.text().await?;
        Ok(serde_json::from_str(&text)?)
    }

    /// POST `/wallet/broadcasttransaction` with the given JSON body and
    /// decode the response into `T`. Returns the broadcast receipt (tx_id +
    /// result flag + optional error).
    pub async fn post_broadcasttransaction<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<T, JsonRpcError> {
        let url = format!("{}/wallet/broadcasttransaction", self.base_url());
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        let resp = client.post(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(JsonRpcError::HttpStatus(status.as_u16()));
        }
        let text = resp.text().await?;
        Ok(serde_json::from_str(&text)?)
    }

    /// POST `/wallet/gettransactionbyid` with the given JSON body and
    /// decode the response into `T`. Returns the transaction record if the
    /// network has seen it, or an empty result field when the tx is still
    /// in-flight / not yet visible.
    pub async fn post_gettransactionbyid<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<T, JsonRpcError> {
        let url = format!("{}/wallet/gettransactionbyid", self.base_url());
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        let resp = client.post(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(JsonRpcError::HttpStatus(status.as_u16()));
        }
        let text = resp.text().await?;
        Ok(serde_json::from_str(&text)?)
    }

    /// POST `/wallet/gettransactioninfobyid` with the given JSON body and
    /// decode the response into `T`. Returns the **transaction receipt**
    /// (block number, energy usage, contract result). Unlike
    /// `gettransactionbyid`, this endpoint echoes the tx id as the top-level
    /// `id` field — letting the caller bind the response to the request
    /// without trusting the fullnode to filter by URL `value=<tx_id>` alone.
    pub async fn post_gettransactioninfobyid<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<T, JsonRpcError> {
        let url = format!("{}/wallet/gettransactioninfobyid", self.base_url());
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        let resp = client.post(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(JsonRpcError::HttpStatus(status.as_u16()));
        }
        let text = resp.text().await?;
        Ok(serde_json::from_str(&text)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Valid 64-char lowercase hex pin (zero-bytes SHA-256) for happy-path fixtures.
    const VALID_PIN: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn new_pinned_parses_url_with_explicit_port() {
        let c = JsonRpcClient::new_pinned(&format!("pinned://{VALID_PIN}@nile.trongrid.io:443"))
            .expect("valid pinned URL with port");
        assert_eq!(c.host, "nile.trongrid.io");
        assert_eq!(c.port, 443);
        assert_eq!(c.pin_hex, VALID_PIN);
    }

    #[test]
    fn new_pinned_parses_url_with_default_https_port() {
        let c = JsonRpcClient::new_pinned(&format!("pinned://{VALID_PIN}@nile.trongrid.io"))
            .expect("valid pinned URL without port implicit");
        assert_eq!(c.host, "nile.trongrid.io");
        assert_eq!(c.port, 443);
        assert_eq!(c.pin_hex, VALID_PIN);
    }

    #[test]
    fn new_pinned_accepts_full_64_char_pin() {
        let url = format!("pinned://{VALID_PIN}@nile.trongrid.io:443");
        let c = JsonRpcClient::new_pinned(&url).expect("valid full-length pin");
        assert_eq!(c.pin_hex.len(), 64);
    }

    #[test]
    fn new_pinned_rejects_non_pinned_scheme() {
        assert_eq!(
            JsonRpcClient::new_pinned("https://example.com"),
            Err(ClientParseError::BadScheme)
        );
    }

    #[test]
    fn new_pinned_rejects_missing_at_separator() {
        assert_eq!(
            JsonRpcClient::new_pinned("pinned://nile.trongrid.io"),
            Err(ClientParseError::MissingPinSeparator)
        );
    }

    #[test]
    fn new_pinned_rejects_empty_pin() {
        assert_eq!(
            JsonRpcClient::new_pinned("pinned://@nile.trongrid.io"),
            Err(ClientParseError::EmptyPin)
        );
    }

    #[test]
    fn new_pinned_rejects_non_hex_pin() {
        let bad_pin = "Z".repeat(64);
        let url = format!("pinned://{bad_pin}@nile.trongrid.io");
        assert_eq!(
            JsonRpcClient::new_pinned(&url),
            Err(ClientParseError::BadPin)
        );
    }

    #[test]
    fn new_pinned_rejects_empty_host() {
        let url = format!("pinned://{VALID_PIN}@");
        assert_eq!(
            JsonRpcClient::new_pinned(&url),
            Err(ClientParseError::EmptyHost)
        );
    }

    #[test]
    fn new_pinned_rejects_non_u16_port() {
        let url = format!("pinned://{VALID_PIN}@nile.trongrid.io:99999");
        assert_eq!(
            JsonRpcClient::new_pinned(&url),
            Err(ClientParseError::BadPort)
        );
    }
}
