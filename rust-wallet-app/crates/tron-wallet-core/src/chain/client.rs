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
use crate::tx::broadcast::{
    parse_block_header_response, BlockHeader, BroadcastReceipt, TransactionInfo,
};

/// HTTP client for TronGrid (or any other TRON fullnode speaking the same
/// schema). Construction is cheap — the underlying `reqwest::Client` is the
/// expensive bit — so prefer one-per-workspace rather than per-request.
pub struct TronGridClient {
    rpc_url: String,
    http: reqwest::Client,
}

/// Body TronGrid expects at `wallet/broadcasthex`.
///
/// `transaction` is the hex-encoded full `TronTransaction` envelope
/// (raw_data + signature inside), the exact byte-exact payload that
/// `anychain_tron::TronTransaction::sign(...)` returns. We use
/// `/wallet/broadcasthex` (single-blob) rather than the older
/// `/wallet/broadcasttransaction` (split-form raw_data_hex +
/// signature_hex) because the split-form endpoint triggers a NPE in
/// TronGrid's Java gateway for our wire format (see plan §Q13 +
/// issue #540 investigation).
#[derive(Debug, Serialize)]
struct BroadcastBody<'a> {
    #[serde(rename = "transaction")]
    transaction: &'a str,
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

    /// `POST /wallet/broadcasthex`.
    ///
    /// A `success = false` return on HTTP 200 happens when the node accepted
    /// the request but rejected the chain state (e.g. balance check). Both
    /// surfaces are surfaced as [`Error::Node`] with the `code` field
    /// embedded, so the caller can branch on `is_success` if they need to.
    ///
    /// The single-blob `/wallet/broadcasthex` endpoint is preferred over the
    /// split-form `/wallet/broadcasttransaction` because the split-form
    /// endpoint triggers a NPE in TronGrid's Java gateway for our wire
    /// format (plan §Q13 / issue #540). The full `TronTransaction`
    /// envelope (`SignedTransaction::signed_envelope_hex`) is byte-exact
    /// what TronGrid's parser expects.
    pub async fn broadcast(&self, signed_envelope_hex: &str) -> Result<BroadcastReceipt> {
        let url = format!("{}/wallet/broadcasthex", self.rpc_url);
        let body = BroadcastBody {
            transaction: signed_envelope_hex,
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

        parse_block_header_response(&bytes)
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
    /// `function` is the **human-readable Solidity signature** of the
    /// function being called (e.g. `"decimals()"`, `"balanceOf(address)"`),
    /// NOT the 4-byte hex selector. TronGrid resolves the signature
    /// against the contract's published ABI to compute the actual 4-byte
    /// selector at call time; sending a hex selector here triggers a
    /// silent path where the server derives a different 4-byte prefix
    /// from your hex bytes, every simulated call reverts with
    /// `"REVERT opcode executed"`, and the response's `constant_result`
    /// is empty.
    ///
    /// `args` is the ABI-encoded argument block — empty for `decimals()` /
    /// `symbol()` / `name()`, 32 bytes for `balanceOf(address)` (a
    /// 32-byte slot holding the 21-byte T-address with 11 zero bytes of
    /// left padding — see [`crate::trc20::balance_of_args`]).
    ///
    /// `owner_address` is a required field on TronGrid's request body
    /// (the server uses it as the simulated caller for the read-only
    /// call — for `view` functions the value does not affect the result,
    /// but the field must be a valid T-address or the server rejects with
    /// `owner_address isn't set`). Pass any valid wallet from the test
    /// fixture for view-only calls.
    pub async fn trigger_constant_contract(
        &self,
        contract: &str,
        owner_address: &str,
        function: &str,
        args: &[u8],
    ) -> Result<ConstantContractCall> {
        let url = format!("{}/wallet/triggerconstantcontract", self.rpc_url);

        #[derive(Serialize)]
        struct Body<'a> {
            #[serde(rename = "contract_address")]
            contract_address: &'a str,
            #[serde(rename = "owner_address")]
            owner_address: &'a str,
            #[serde(rename = "function_selector")]
            function_selector: &'a str,
            #[serde(rename = "parameter")]
            parameter: String,
            #[serde(rename = "visible")]
            visible: bool,
        }
        let body = Body {
            contract_address: contract,
            owner_address,
            function_selector: function,
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
    ///
    /// `function` is the Solidity signature (see
    /// [`Self::trigger_constant_contract`]).
    pub async fn estimate_energy(
        &self,
        contract: &str,
        owner_address: &str,
        function: &str,
        args: &[u8],
    ) -> Result<u64> {
        let resp = self
            .trigger_constant_contract(contract, owner_address, function, args)
            .await?;
        Ok(resp.energy_used.unwrap_or(0))
    }

    /// `POST /wallet/getaccount` — native TRX balance and account presence.
    ///
    /// An address that has never received funds is not an error: TronGrid
    /// answers HTTP 200 with `{}`, which this maps to
    /// `AccountInfo { address: None, balance_sun: 0 }` (so
    /// `AccountInfo::exists()` returns `false`). Collapsing that into
    /// an error would make "empty wallet" indistinguishable from "node down",
    /// and those two need different operator responses.
    pub async fn get_account(&self, address: &str) -> Result<AccountInfo> {
        let url = format!("{}/wallet/getaccount", self.rpc_url);
        #[derive(Serialize)]
        struct Body<'a> {
            address: &'a str,
            visible: bool,
        }
        let resp = self
            .http
            .post(&url)
            .json(&Body {
                address,
                visible: true,
            })
            .send()
            .await
            .map_err(|e| Error::Node(format!("getaccount send: {e}")))?;

        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| Error::Node(format!("getaccount body read: {e}")))?;
        if !status.is_success() {
            return Err(Error::Node(format!(
                "getaccount HTTP {}: {}",
                status,
                String::from_utf8_lossy(&bytes)
            )));
        }
        parse_account_response(&bytes)
    }

    /// `POST /wallet/gettransactionbyid` — the *transaction*, not its receipt.
    ///
    /// Distinct from [`Self::get_tx_info`], which returns the execution
    /// receipt. This one carries `raw_data.contract`, which is what a
    /// fee-limit bump needs in order to rebuild an equivalent transaction.
    pub async fn get_transaction_by_id(&self, txid_hex: &str) -> Result<OriginalCall> {
        let url = format!("{}/wallet/gettransactionbyid", self.rpc_url);
        #[derive(Serialize)]
        struct Body<'a> {
            value: &'a str,
            visible: bool,
        }
        let resp = self
            .http
            .post(&url)
            .json(&Body {
                value: txid_hex,
                visible: true,
            })
            .send()
            .await
            .map_err(|e| Error::Node(format!("gettransactionbyid send: {e}")))?;

        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| Error::Node(format!("gettransactionbyid body read: {e}")))?;
        if !status.is_success() {
            return Err(Error::Node(format!(
                "gettransactionbyid HTTP {}: {}",
                status,
                String::from_utf8_lossy(&bytes)
            )));
        }
        parse_transaction_by_id(&bytes)
    }
}

/// Native-account view returned by `getaccount`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountInfo {
    /// T-address the node reported, when present. `None` for an account the
    /// chain has never seen.
    pub address: Option<String>,
    /// Native balance in SUN (1 TRX = 1_000_000 SUN).
    pub balance_sun: u64,
}

impl AccountInfo {
    /// Whether the chain holds a record for this address at all.
    ///
    /// An activated account holding exactly 0 TRX still has a record, so
    /// `address.is_some()` is the right proxy — keeping a separate
    /// `exists: bool` field would let the two drift out of sync (PR #545
    /// review finding).
    pub fn exists(&self) -> bool {
        self.address.is_some()
    }
}

/// The contract call inside an already-broadcast transaction, reduced to the
/// two shapes v0.1 can rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginalCall {
    /// Native TRX transfer.
    Transfer {
        owner: String,
        to: String,
        amount_sun: u64,
    },
    /// Smart-contract call (TRC-20 transfer/approve and anything else).
    TriggerSmartContract {
        owner: String,
        contract: String,
        /// ABI calldata as hex, selector included.
        data_hex: String,
    },
}

/// Pure decoder for `getaccount`, split out so it is testable without a node.
fn parse_account_response(bytes: &[u8]) -> Result<AccountInfo> {
    if bytes.is_empty() || bytes == b"{}" {
        return Ok(AccountInfo {
            address: None,
            balance_sun: 0,
        });
    }
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| Error::NodeResponse(format!("getaccount decode: {e}")))?;
    if let Some(err) = value.get("Error").and_then(|e| e.as_str()) {
        return Err(Error::Node(format!("getaccount: {err}")));
    }
    // `balance` is absent on an activated account holding exactly 0 TRX.
    let balance_sun = match value.get("balance") {
        Some(b) => b
            .as_u64()
            .ok_or_else(|| Error::NodeResponse(format!("getaccount balance not a u64: {b}")))?,
        None => 0,
    };
    Ok(AccountInfo {
        address: value
            .get("address")
            .and_then(|a| a.as_str())
            .map(str::to_owned),
        balance_sun,
    })
}

/// Pure decoder for `gettransactionbyid`, reduced to [`OriginalCall`].
fn parse_transaction_by_id(bytes: &[u8]) -> Result<OriginalCall> {
    if bytes.is_empty() || bytes == b"{}" {
        return Err(Error::Node(
            "gettransactionbyid: unknown txid (empty response)".into(),
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| Error::NodeResponse(format!("gettransactionbyid decode: {e}")))?;

    let contract = value
        .pointer("/raw_data/contract/0")
        .ok_or_else(|| Error::NodeResponse("gettransactionbyid: no raw_data.contract[0]".into()))?;
    let kind = contract
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| Error::NodeResponse("gettransactionbyid: contract has no type".into()))?;
    let params = contract
        .pointer("/parameter/value")
        .ok_or_else(|| Error::NodeResponse("gettransactionbyid: no parameter.value".into()))?;

    let field = |name: &str| -> Result<String> {
        params
            .get(name)
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .ok_or_else(|| {
                Error::NodeResponse(format!("gettransactionbyid: {kind} missing {name}"))
            })
    };

    match kind {
        "TransferContract" => Ok(OriginalCall::Transfer {
            owner: field("owner_address")?,
            to: field("to_address")?,
            amount_sun: params
                .get("amount")
                .and_then(|a| a.as_u64())
                .ok_or_else(|| Error::NodeResponse("TransferContract missing amount".into()))?,
        }),
        "TriggerSmartContract" => Ok(OriginalCall::TriggerSmartContract {
            owner: field("owner_address")?,
            contract: field("contract_address")?,
            data_hex: field("data")?,
        }),
        other => Err(Error::TransactionBuild(format!(
            "cannot rebuild a {other} transaction: only TransferContract and \
             TriggerSmartContract are supported"
        ))),
    }
}

#[cfg(test)]
mod response_decoding_tests {
    use super::*;

    #[test]
    fn unseen_account_is_zero_balance_not_an_error() {
        let info = parse_account_response(b"{}").expect("empty body is a valid answer");
        assert!(!info.exists());
        assert_eq!(info.balance_sun, 0);
    }

    #[test]
    fn activated_account_with_no_balance_field_reads_as_zero() {
        let info =
            parse_account_response(br#"{"address":"TAbc","create_time":1}"#).expect("decode");
        assert!(
            info.exists(),
            "the node returned a record, so the account exists"
        );
        assert_eq!(info.balance_sun, 0);
    }

    #[test]
    fn funded_account_reports_sun() {
        let info =
            parse_account_response(br#"{"address":"TAbc","balance":1500000}"#).expect("decode");
        assert_eq!(info.balance_sun, 1_500_000);
        assert_eq!(info.address.as_deref(), Some("TAbc"));
    }

    /// Regression for Finding K: `exists()` is a method, derived from
    /// `address.is_some()`, never a stored field — PR #545 review.
    #[test]
    fn account_info_exists_derives_from_address() {
        let seen = AccountInfo {
            address: Some("TAbc".into()),
            balance_sun: 0,
        };
        assert!(seen.exists(), "address.is_some() implies exists()");

        let unseen = AccountInfo {
            address: None,
            balance_sun: 0,
        };
        assert!(!unseen.exists(), "address.is_none() implies !exists()");
    }

    #[test]
    fn node_side_error_field_is_surfaced() {
        let err = parse_account_response(br#"{"Error":"invalid address"}"#)
            .expect_err("Error field must not be read as a balance");
        assert!(matches!(err, Error::Node(_)));
    }

    #[test]
    fn transfer_contract_round_trips_into_original_call() {
        let body = br#"{"raw_data":{"contract":[{"type":"TransferContract",
            "parameter":{"value":{"owner_address":"TFrom","to_address":"TTo","amount":42}}}]}}"#;
        assert_eq!(
            parse_transaction_by_id(body).expect("decode"),
            OriginalCall::Transfer {
                owner: "TFrom".into(),
                to: "TTo".into(),
                amount_sun: 42,
            }
        );
    }

    #[test]
    fn trigger_smart_contract_round_trips_into_original_call() {
        let body = br#"{"raw_data":{"contract":[{"type":"TriggerSmartContract",
            "parameter":{"value":{"owner_address":"TFrom","contract_address":"TUsdt","data":"a9059cbb"}}}]}}"#;
        assert_eq!(
            parse_transaction_by_id(body).expect("decode"),
            OriginalCall::TriggerSmartContract {
                owner: "TFrom".into(),
                contract: "TUsdt".into(),
                data_hex: "a9059cbb".into(),
            }
        );
    }

    #[test]
    fn unknown_txid_is_an_error_not_a_default_transaction() {
        // Rebuilding a "default" transaction here would sign a transfer the
        // operator never made.
        assert!(parse_transaction_by_id(b"{}").is_err());
    }

    #[test]
    fn unsupported_contract_type_is_refused() {
        let body = br#"{"raw_data":{"contract":[{"type":"FreezeBalanceV2Contract",
            "parameter":{"value":{"owner_address":"TFrom"}}}]}}"#;
        assert!(matches!(
            parse_transaction_by_id(body),
            Err(Error::TransactionBuild(_))
        ));
    }
}
