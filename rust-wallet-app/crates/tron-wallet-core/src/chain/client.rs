//! Thin `reqwest` wrapper around the three TronGrid endpoints `tron-wallet-core`
//! uses in v0.1:
//!
//! - `POST /wallet/broadcasttransaction` — accept a signed payload.
//! - `GET  /walletsolidity/getnowblock` — TAPOS reference for the next tx.
//! - `POST /wallet/gettransactioninfobyid` — receipt shape, polled for
//!   confirmation.
//!
//! `TronGridClient` is a `serde_json` envelope adapter. The crate owns the
//! *types* the endpoints return (`crate::tx::broadcast::{BroadcastReceipt,
//! BlockHeader, TransactionInfo}`); this module bridges to `reqwest`.
//!
//! # SPKI pin policy
//!
//! Construction accepts an optional [`SpkiPin`]: if `Some`, every TLS
//! handshake is verified against that pin via [`SpkiPinnedVerifier`]. If
//! `None`, the system trust roots are used (Scenario B in the plan).
//!
//! Pinning is intentionally minimal in v0.1: it cannot enforce rotation
//! across multiple pins (future work). One pin = one TLS chain.

use core::time::Duration;
use serde::Serialize;

use crate::chain::constant_contract::ConstantContractCall;
use crate::chain::spki::{SpkiPin, SpkiPinnedVerifier};
use crate::error::{Error, Result};
use crate::tx::broadcast::{BlockHeader, BroadcastReceipt, TransactionInfo};

/// HTTP client for TronGrid (or any other TRON fullnode speaking the same
/// schema). Construction is cheap — the underlying `reqwest::Client` is the
/// expensive bit — so prefer one-per-workspace rather than per-request.
pub struct TronGridClient {
    rpc_url: String,
    http: reqwest::Client,
}

/// Body TronGrid expects at `wallet/broadcasttransaction`.
///
/// `raw_data_hex` and `signature_hex` are the two pieces a
/// [`crate::tx::sign::SignedTransaction`] hands back. The wire envelope is
/// what TronGrid inspects; we do not invent a different layout.
#[derive(Debug, Serialize)]
struct BroadcastBody<'a> {
    #[serde(rename = "raw_data_hex")]
    raw_data_hex: &'a str,
    #[serde(rename = "signature_hex")]
    signature_hex: &'a str,
    #[serde(rename = "visible")]
    visible: bool,
}

impl TronGridClient {
    /// Construct a client against an arbitrary RPC base URL (e.g.
    /// `https://api.trongrid.io`, `https://nile.trongrid.io`, or
    /// `http://127.0.0.1:8090` for a local TronBox).
    ///
    /// `spki_pin = None` ⇒ system trust roots (Scenario B). `Some(pin)` ⇒
    /// [`SpkiPinnedVerifier`] rejects any cert chain whose leaf SPKI SHA-256
    /// digest does not match.
    pub fn new(rpc_url: &str, spki_pin: Option<SpkiPin>) -> Result<Self> {
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("tron-wallet-core/", env!("CARGO_PKG_VERSION")));

        if let Some(pin) = spki_pin {
            let verifier =
                SpkiPinnedVerifier::new(pin).map_err(|e| Error::SpkiPin(e.to_string()))?;
            let rustls_cfg = verifier.into_client_config();
            builder = builder.use_preconfigured_tls(rustls_cfg);
        }

        let http = builder
            .build()
            .map_err(|e| Error::Node(format!("build reqwest client: {e}")))?;

        Ok(Self {
            rpc_url: rpc_url.trim_end_matches('/').to_owned(),
            http,
        })
    }

    /// `POST /wallet/broadcasttransaction`.
    ///
    /// A `success = false` return on HTTP 200 happens when the node accepted
    /// the request but rejected the chain state (e.g. balance check). Both
    /// surfaces are surfaced as [`Error::Node`] with the `code` field
    /// embedded, so the caller can branch on `is_success` if they need to.
    pub async fn broadcast(
        &self,
        raw_data_hex: &str,
        signature_hex: &str,
    ) -> Result<BroadcastReceipt> {
        let url = format!("{}/wallet/broadcasttransaction", self.rpc_url);
        let body = BroadcastBody {
            raw_data_hex,
            signature_hex,
            visible: false,
        };

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Node(format!("broadcast send: {e}")))?;

        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| Error::Node(format!("broadcast body read: {e}")))?;

        if !status.is_success() {
            return Err(Error::Node(format!(
                "broadcast HTTP {}: {}",
                status,
                String::from_utf8_lossy(&bytes)
            )));
        }

        serde_json::from_slice(&bytes)
            .map_err(|e| Error::NodeResponse(format!("broadcast decode: {e}")))
    }

    /// `GET /walletsolidity/getnowblock`.
    ///
    /// The *SolidityNode* endpoint (not `wallet/getnowblock`, which uses the
    /// looser `/wallet/` namespace) gives stronger finality guarantees —
    /// see plan Q7.
    pub async fn get_now_block(&self) -> Result<BlockHeader> {
        let url = format!("{}/walletsolidity/getnowblock", self.rpc_url);

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Node(format!("getnowblock send: {e}")))?;

        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| Error::Node(format!("getnowblock body read: {e}")))?;

        if !status.is_success() {
            return Err(Error::Node(format!(
                "getnowblock HTTP {}: {}",
                status,
                String::from_utf8_lossy(&bytes)
            )));
        }

        serde_json::from_slice(&bytes)
            .map_err(|e| Error::NodeResponse(format!("getnowblock decode: {e}")))
    }

    /// `POST /wallet/gettransactioninfobyid` — receipt probe.
    ///
    /// Polled by callers (`tx wait`) to detect confirmation. Returns the
    /// raw shape; interpretation of `block_number: None` as "still pending"
    /// is the caller's job.
    pub async fn get_tx_info(&self, txid_hex: &str) -> Result<TransactionInfo> {
        let url = format!("{}/wallet/gettransactioninfobyid", self.rpc_url);
        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(rename = "value")]
            value: &'a str,
        }
        let body = Body { value: txid_hex };

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Node(format!("gettransactioninfobyid send: {e}")))?;

        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| Error::Node(format!("gettransactioninfobyid body read: {e}")))?;

        // TronGrid returns HTTP 200 with `{}` for unknown txids on some
        // endpoint versions, so we do not reject on non-2xx alone here —
        // let the decoder decide whether the body is a `TransactionInfo`.
        if !status.is_success() && !bytes.is_empty() {
            return Err(Error::Node(format!(
                "gettransactioninfobyid HTTP {}: {}",
                status,
                String::from_utf8_lossy(&bytes)
            )));
        }

        if bytes.is_empty() || bytes.as_ref() == b"{}" {
            return Ok(TransactionInfo {
                id: None,
                block_number: None,
                contract_result: Vec::new(),
                fee: None,
            });
        }

        serde_json::from_slice(&bytes)
            .map_err(|e| Error::NodeResponse(format!("gettransactioninfobyid decode: {e}")))
    }

    /// `POST /wallet/triggerconstantcontract` — read-only contract call.
    ///
    /// `selector` is the 4-byte function selector, hex-encoded without the
    /// `0x` prefix (TronGrid takes the raw 4-byte hex). `args` is the
    /// ABI-encoded argument block **without the selector prefix** — the
    /// server prepends the selector to whatever bytes you send. Sending
    /// the selector twice is the wire-format bug the plan's Task 3.3 calls
    /// out (corrected via #410).
    pub async fn trigger_constant_contract(
        &self,
        contract: &str,
        selector: [u8; 4],
        args: &[u8],
    ) -> Result<ConstantContractCall> {
        let url = format!("{}/wallet/triggerconstantcontract", self.rpc_url);

        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(rename = "contract_address")]
            contract_address: &'a str,
            #[serde(rename = "function_selector")]
            function_selector: String,
            #[serde(rename = "parameter")]
            parameter: String,
            #[serde(rename = "visible")]
            visible: bool,
        }
        let body = Body {
            contract_address: contract,
            function_selector: hex::encode(selector),
            parameter: hex::encode(args),
            visible: true,
        };

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Node(format!("triggerconstantcontract send: {e}")))?;

        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| Error::Node(format!("triggerconstantcontract body read: {e}")))?;

        if !status.is_success() {
            return Err(Error::Node(format!(
                "triggerconstantcontract HTTP {}: {}",
                status,
                String::from_utf8_lossy(&bytes)
            )));
        }

        let parsed: ConstantContractCall = serde_json::from_slice(&bytes)
            .map_err(|e| Error::NodeResponse(format!("triggerconstantcontract decode: {e}")))?;

        if !parsed.result.result {
            // Server can return HTTP 200 with `result.result = false` when
            // the contract reverted or the network refused for another
            // reason (out-of-energy, OOG). Surface that explicitly so the
            // caller can branch.
            return Err(Error::Node(format!(
                "triggerconstantcontract({contract}) rejected: code={:?} message={:?}",
                parsed.result.code, parsed.result.message
            )));
        }

        Ok(parsed)
    }

    /// Estimate the Energy a contract call will consume.
    ///
    /// Thin wrapper over [`Self::trigger_constant_contract`] — the same
    /// endpoint populates `energy_used` when invoked with `visible: true`.
    /// Returns 0 if the network response omitted the field (some
    /// TronGrid variants do), which is the conservative "no overhead"
    /// value the caller can build on top of without crashing.
    pub async fn estimate_energy(
        &self,
        contract: &str,
        selector: [u8; 4],
        args: &[u8],
    ) -> Result<u64> {
        let resp = self
            .trigger_constant_contract(contract, selector, args)
            .await?;
        Ok(resp.energy_used.unwrap_or(0))
    }
}
