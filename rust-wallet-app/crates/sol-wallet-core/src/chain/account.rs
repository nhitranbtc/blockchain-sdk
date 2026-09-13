//! `chain::account` — 15 HTTP RPC method thin wrappers (Phase 5.1 critical path).
//!
//! Every method here delegates to `RpcClient::post()` (private). No
//! `serde_json::Value` indexing in the wrappers; the typed `RpcResponse<T>`
//! envelope (Tier 2 finding #7) is decoded by `post()` itself.
//!
//! Coverage matrix (plan doc §Phase 5 Table 5.1):
//!
//! | # | Method | V0.1 surface | Used by |
//! |---|--------|--------------|---------|
//! | 1  | `getLatestBlockhash`              | `get_latest_blockhash() -> Result<(Hash, u64)>`               | send, send-token |
//! | 2  | `sendTransaction`                 | `send_transaction(tx: &Transaction) -> Result<Signature>`  | send, send-token |
//! | 3  | `getSignatureStatuses`            | `get_signature_status(sig) -> Result<Option<TransactionStatus>>` | wait, send (confirm), history, tx |
//! | 4  | `simulateTransaction`             | `simulate_transaction(tx) -> Result<SimulateResult>` (Tier 4: result is a HINT, not a guarantee) | send (dry-run), send-token (dry-run) |
//! | 5  | `getBalance`                      | `get_balance(pubkey) -> Result<u64>`                       | balance |
//! | 6  | `getAccountInfo`                  | `get_account_info(pubkey) -> Result<Option<Account>>`     | mint decimals, token-info |
//! | 7  | `getMultipleAccounts`             | `get_multiple_accounts(pubkeys) -> Result<Vec<Option<Account>>>` | list-tokens (batched) |
//! | 8  | `getTokenAccountBalance`          | `get_token_account_balance(pubkey) -> Result<UiTokenAmount>` | balance --token, send-token preflight |
//! | 9  | `getTokenAccountsByOwner`         | `get_token_accounts_by_owner(owner, program) -> Result<Vec<RpcKeyedAccount>>` | list-tokens |
//! | 10 | `getTokenSupply`                  | `get_token_supply(mint) -> Result<UiTokenAmount>`          | token-info |
//! | 11 | `getMinimumBalanceForRentExemption` | `get_minimum_balance_for_rent_exemption(data_len) -> Result<u64>` | rent, send-token auto-ATA |
//! | 12 | `getRecentPrioritizationFees`     | `get_recent_prioritization_fees(addresses) -> Result<Vec<RpcPrioritizationFee>>` | send --priority-fee auto |
//! | 13 | `getVersion`                      | `get_version() -> Result<Version>`                        | info |
//! | 14 | `getEpochInfo`                    | `get_epoch_info() -> Result<EpochInfo>`                   | info |
//! | 15 | `getHealth`                       | `get_health() -> Result<()>`                               | info (probe) |
//!
//! `requestAirdrop` (Task 5.2 — landed) + `getTransaction` (Task 5.3 —
//! pending) are added in their own tasks; `accountSubscribe` etc. (5 WS
//! subscribes) are V0.1.5 watch mode.
//!
//! Importers: `tx::broadcast` (send_and_confirm uses
//! `latest_blockhash` + `send_transaction`; wait_for_confirm uses
//! `signature_statuses`); `tests/rpc_methods_mock.rs`; Phase 7 CLI.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use solana_account::Account;
use solana_sdk::epoch_info::EpochInfo;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::Transaction;

// =============================================================================
// V0.1 minimal wire-format structs
// =============================================================================
//
// Anza's `solana-sdk 4.1.0` umbrella re-exports `solana_message` (which
// has `Message`) but does NOT re-export `TransactionStatus` (from the
// 1.18 `solana-transaction-status` family — incompatible with 4.1.0's
// 4.x sub-crate family). Rather than drag in the 1.18 family (which
// conflicts on `zeroize ^1.0` vs `^1.5`), V0.1 defines minimal local
// structs for the wire shapes we need.
//
// `EpochInfo` is re-exported by Anza 4.1.0 (use the upstream).

/// Subset of Anza's `TransactionStatus` for V0.1.
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionStatus {
    pub slot: u64,
    pub confirmations: Option<u64>,
    pub confirmation_status: Option<ConfirmationStatus>,
    pub err: Option<serde_json::Value>,
    pub block_time: Option<i64>,
}

#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConfirmationStatus {
    Processed,
    Confirmed,
    Finalized,
}

/// Subset of Anza's `UiTokenAmount` for V0.1 (mint decimals + base units).
/// `ui_amount` is omitted from `PartialEq` because `f64` doesn't impl
/// `Eq` — equality is on `amount` (string) + `decimals` + `ui_amount_string`.
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UiTokenAmount {
    pub amount: String, // u64 as string to avoid JSON i64 overflow
    pub decimals: u8,
    pub ui_amount: Option<f64>,
    pub ui_amount_string: Option<String>,
}

#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    #[serde(rename = "solana-core")]
    pub solana_core: String,
    #[serde(rename = "feature-set")]
    pub feature_set: Option<u32>,
}

#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcPrioritizationFee {
    pub slot: u64,
    pub prioritization_fee: u64,
}

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use crate::chain::client::RpcClient;
use crate::error::{Error, Result};

// =============================================================================
// 1 — getLatestBlockhash
// =============================================================================
/// Get the latest blockhash + the slot it was produced at.
pub async fn get_latest_blockhash(client: &RpcClient) -> Result<(Hash, u64)> {
    let raw: Value = client
        .post("getLatestBlockhash", json!([{"commitment": "confirmed"}]))
        .await?;
    let hash_str = raw
        .get("value")
        .and_then(|v| v.get("blockhash"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            Error::Transport("getLatestBlockhash: missing value.blockhash".to_string())
        })?;
    let hash = hash_str
        .parse::<Hash>()
        .map_err(|e| Error::Transport(format!("getLatestBlockhash: parse Hash: {e}")))?;
    let slot = raw
        .get("context")
        .and_then(|c| c.get("slot"))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::Transport("getLatestBlockhash: missing context.slot".to_string()))?;
    Ok((hash, slot))
}

// =============================================================================
// 2 — sendTransaction
// =============================================================================
/// Submit a signed transaction; returns the first signature.
pub async fn send_transaction(client: &RpcClient, tx: &Transaction) -> Result<Signature> {
    send_transaction_with_options(client, tx, json!({"encoding": "base64"})).await
}

/// Submit a signed `Transaction` with explicit `sendTransaction` options
/// (e.g. `{"encoding": "base64", "replaceRecentBlockhash": true}`).
///
/// The library's 2-arg `send_transaction` uses a fixed options payload.
/// Use this overload when the cluster requires `replaceRecentBlockhash`
/// (devnet/testnet validators may have inconsistent views of the recent
/// blockhash).
pub async fn send_transaction_with_options(
    client: &RpcClient,
    tx: &Transaction,
    options: Value,
) -> Result<Signature> {
    // Tier 2 finding #8: bincode wire format with `bincode::config::legacy()`
    // for Anza RPC server compatibility. The version is pinned to 1.3.3 in
    // workspace dependencies.
    //
    // Anza RPC `sendTransaction` expects `VersionedTransaction` (legacy is
    // wrapped as `VersionedTransaction::Legacy`). Wrap before serialize.
    let versioned = solana_sdk::transaction::VersionedTransaction::from(tx.clone());
    let wire = bincode::serialize(&versioned)
        .map_err(|e| Error::Transport(format!("sendTransaction: bincode serialize: {e}")))?;
    let wire_b64 = BASE64.encode(&wire);
    let raw: Value = client
        .post("sendTransaction", json!([wire_b64, options]))
        .await?;
    let sig_str = raw
        .as_str()
        .ok_or_else(|| Error::Transport("sendTransaction: result not a string".to_string()))?;
    sig_str
        .parse::<Signature>()
        .map_err(|e| Error::Transport(format!("sendTransaction: parse Signature: {e}")))
}

// =============================================================================
// 3 — getSignatureStatuses
// =============================================================================
/// Poll the signature status (used by `tx::broadcast::wait_for_confirm`).
pub async fn get_signature_status(
    client: &RpcClient,
    sig: &Signature,
) -> Result<Option<TransactionStatus>> {
    let raw: Value = client
        .post(
            "getSignatureStatuses",
            json!([vec![sig.to_string()], {"searchTransactionHistory": true}]),
        )
        .await?;
    let entry = raw
        .get("value")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first());
    match entry {
        None => Ok(None),
        Some(v) if v.is_null() => Ok(None),
        Some(v) => serde_json::from_value(v.clone()).map(Some).map_err(|e| {
            Error::Transport(format!(
                "getSignatureStatuses: parse TransactionStatus: {e}"
            ))
        }),
    }
}

// =============================================================================
// 4 — simulateTransaction
// =============================================================================
/// Simulate a signed transaction without broadcasting.
///
/// **Tier 4 finding #4:** the returned `units_consumed` and error
/// information are a HINT, not a guarantee. Cluster state may change
/// between this call and the subsequent `sendTransaction`, and the CU
/// computation could overflow. V0.1 surfaces `Error::ComputeBudgetExceeded`
/// when `units_consumed > cu_limit`, but does NOT retry the simulation
/// after signing.
pub async fn simulate_transaction(
    client: &RpcClient,
    tx: &Transaction,
) -> Result<SolanaSimulateResult> {
    let wire = bincode::serialize(tx)
        .map_err(|e| Error::Transport(format!("simulateTransaction: bincode serialize: {e}")))?;
    let wire_b64 = BASE64.encode(&wire);
    let raw: Value = client
        .post(
            "simulateTransaction",
            json!([wire_b64, {"encoding": "base64", "replaceRecentBlockhash": true, "commitment": "confirmed"}]),
        )
        .await?;
    let value = raw
        .get("value")
        .ok_or_else(|| Error::Transport("simulateTransaction: missing value".to_string()))?;
    serde_json::from_value::<SolanaSimulateResult>(value.clone())
        .map_err(|e| Error::Transport(format!("simulateTransaction: parse: {e}")))
}

/// Subset of Anza's `RpcSimulateTransactionResult` that V0.1 uses.
/// `units_consumed` drives the `Error::ComputeBudgetExceeded` check;
/// `err` is surfaced verbatim so the CLI can display a useful message.
#[allow(missing_docs)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SolanaSimulateResult {
    pub err: Option<serde_json::Value>,
    pub logs: Option<Vec<String>>,
    pub units_consumed: Option<u64>,
    #[serde(default)]
    pub units_consumed_details: Option<SolanaUnitsConsumedDetails>,
}

#[allow(missing_docs)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SolanaUnitsConsumedDetails {
    // Map of program-id string -> compute units consumed.
    // V0.1 doesn't act on this; surfaced for V0.1.5 detailed reporting.
    #[serde(flatten)]
    pub _rest: std::collections::BTreeMap<String, u64>,
}

// =============================================================================
// 5 — getBalance
// =============================================================================
/// Get an account's SOL balance in lamports.
pub async fn get_balance(client: &RpcClient, pubkey: &Pubkey) -> Result<u64> {
    let raw: Value = client
        .post("getBalance", json!([pubkey.to_string()]))
        .await?;
    raw.get("value")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::Transport("getBalance: missing value".to_string()))
}

// =============================================================================
// 6 — getAccountInfo
// =============================================================================
/// Fetch one account (mint, ATA, system account, program account).
pub async fn get_account_info(client: &RpcClient, pubkey: &Pubkey) -> Result<Option<Account>> {
    let raw: Value = client
        .post(
            "getAccountInfo",
            json!([pubkey.to_string(), {"encoding": "base64"}]),
        )
        .await?;
    let value = raw.get("value");
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            // The result is `{"data": ["<base64>", "base64"], "executable": bool,
            // "lamports": u64, "owner": "<base58>", "rentEpoch": u64, "space": u64}`.
            let lamports = v
                .get("lamports")
                .and_then(|x| x.as_u64())
                .ok_or_else(|| Error::Transport("getAccountInfo: missing lamports".to_string()))?;
            let owner_str = v
                .get("owner")
                .and_then(|x| x.as_str())
                .ok_or_else(|| Error::Transport("getAccountInfo: missing owner".to_string()))?;
            let owner: Pubkey = owner_str.parse().map_err(|e| {
                Error::Transport(format!("getAccountInfo: parse owner Pubkey: {e}"))
            })?;
            let executable = v
                .get("executable")
                .and_then(|x| x.as_bool())
                .unwrap_or(false);
            let rent_epoch = v.get("rentEpoch").and_then(|x| x.as_u64()).unwrap_or(0);
            let _space = v.get("space").and_then(|x| x.as_u64()).unwrap_or(0);
            // Decode data
            let data = v
                .get("data")
                .and_then(|x| x.as_array())
                .and_then(|arr| arr.first())
                .and_then(|x| x.as_str())
                .map(|s| {
                    BASE64.decode(s).map_err(|e| {
                        Error::Transport(format!("getAccountInfo: base64 decode data: {e}"))
                    })
                })
                .transpose()?
                .unwrap_or_default();
            Ok(Some(Account {
                lamports,
                data,
                owner,
                executable,
                rent_epoch,
            }))
        }
    }
}

// =============================================================================
// 7 — getMultipleAccounts
// =============================================================================
/// Batch-fetch multiple accounts in one round-trip.
pub async fn get_multiple_accounts(
    client: &RpcClient,
    pubkeys: &[Pubkey],
) -> Result<Vec<Option<Account>>> {
    let raw: Value = client
        .post(
            "getMultipleAccounts",
            json!([pubkeys.iter().map(|p| p.to_string()).collect::<Vec<_>>(), {"encoding": "base64"}]),
        )
        .await?;
    let arr = raw
        .get("value")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::Transport("getMultipleAccounts: missing value array".to_string()))?;
    let mut out = Vec::with_capacity(arr.len());
    for v in arr {
        if v.is_null() {
            out.push(None);
            continue;
        }
        let lamports = v
            .get("lamports")
            .and_then(|x| x.as_u64())
            .ok_or_else(|| Error::Transport("getMultipleAccounts: missing lamports".to_string()))?;
        let owner_str = v
            .get("owner")
            .and_then(|x| x.as_str())
            .ok_or_else(|| Error::Transport("getMultipleAccounts: missing owner".to_string()))?;
        let owner: Pubkey = owner_str
            .parse()
            .map_err(|e| Error::Transport(format!("getMultipleAccounts: parse owner: {e}")))?;
        let executable = v
            .get("executable")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        let rent_epoch = v.get("rentEpoch").and_then(|x| x.as_u64()).unwrap_or(0);
        let _space = v.get("space").and_then(|x| x.as_u64()).unwrap_or(0);
        let data = v
            .get("data")
            .and_then(|x| x.as_array())
            .and_then(|arr| arr.first())
            .and_then(|x| x.as_str())
            .map(|s| {
                BASE64
                    .decode(s)
                    .map_err(|e| Error::Transport(format!("getMultipleAccounts: base64: {e}")))
            })
            .transpose()?
            .unwrap_or_default();
        out.push(Some(Account {
            lamports,
            data,
            owner,
            executable,
            rent_epoch,
        }));
    }
    Ok(out)
}

// =============================================================================
// 8 — getTokenAccountBalance
// =============================================================================
/// Get a token account's balance (UI amount + raw u64 base units +
/// the mint's decimals).
pub async fn get_token_account_balance(
    client: &RpcClient,
    pubkey: &Pubkey,
) -> Result<UiTokenAmount> {
    let raw: Value = client
        .post("getTokenAccountBalance", json!([pubkey.to_string()]))
        .await?;
    let value = raw
        .get("value")
        .ok_or_else(|| Error::Transport("getTokenAccountBalance: missing value".to_string()))?;
    serde_json::from_value::<UiTokenAmount>(value.clone())
        .map_err(|e| Error::Transport(format!("getTokenAccountBalance: parse: {e}")))
}

// =============================================================================
// 9 — getTokenAccountsByOwner
// =============================================================================
/// Enumerate every token account owned by `owner`. Used by
/// V0.1.5 `wallet list-tokens` (Phase 7).
pub async fn get_token_accounts_by_owner(
    client: &RpcClient,
    owner: &Pubkey,
    program_id: &Pubkey,
) -> Result<Vec<RpcKeyedAccount>> {
    let raw: Value = client
        .post(
            "getTokenAccountsByOwner",
            json!([
                owner.to_string(),
                {"programId": program_id.to_string()},
                {"encoding": "base64"}
            ]),
        )
        .await?;
    let arr = raw
        .get("value")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::Transport("getTokenAccountsByOwner: missing value".to_string()))?;
    serde_json::from_value::<Vec<RpcKeyedAccount>>(Value::Array(arr.clone()))
        .map_err(|e| Error::Transport(format!("getTokenAccountsByOwner: parse: {e}")))
}

/// Subset of Anza's `RpcKeyedAccount`. `pubkey` is the token account
/// address; `account` mirrors the `Account` struct shape.
#[allow(missing_docs)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RpcKeyedAccount {
    pub pubkey: String,
    pub account: AccountJson,
}

/// Wire-level account shape as returned by Anza's RPC (separate from
/// the in-memory `solana_sdk::Account` so we don't deserialize
/// rentEpoch/space redundantly).
#[allow(missing_docs)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AccountJson {
    pub lamports: u64,
    pub data: (String, String), // (base64-data, encoding)
    pub owner: String,
    pub executable: bool,
    pub rent_epoch: u64,
    pub space: u64,
}

// =============================================================================
// 10 — getTokenSupply
// =============================================================================
/// Get a mint's total supply (UI amount + raw base units).
pub async fn get_token_supply(client: &RpcClient, mint: &Pubkey) -> Result<UiTokenAmount> {
    let raw: Value = client
        .post("getTokenSupply", json!([mint.to_string()]))
        .await?;
    let value = raw
        .get("value")
        .ok_or_else(|| Error::Transport("getTokenSupply: missing value".to_string()))?;
    serde_json::from_value::<UiTokenAmount>(value.clone())
        .map_err(|e| Error::Transport(format!("getTokenSupply: parse: {e}")))
}

// =============================================================================
// 11 — getMinimumBalanceForRentExemption
// =============================================================================
/// Compute the lamports required to make an account of `data_len`
/// bytes rent-exempt. Phase 4 SPL builder uses this for the
/// auto-ATA-create preflight.
pub async fn get_minimum_balance_for_rent_exemption(
    client: &RpcClient,
    data_len: usize,
) -> Result<u64> {
    let raw: Value = client
        .post("getMinimumBalanceForRentExemption", json!([data_len]))
        .await?;
    raw.as_u64()
        .ok_or_else(|| Error::Transport("getMinimumBalanceForRentExemption: not u64".to_string()))
}

// =============================================================================
// 12 — getRecentPrioritizationFees
// =============================================================================
/// Get recent prioritization-fee observations (used by V0.1.5
/// priority-fee auto-estimation; V0.1 returns raw list, Phase 7 CLI
/// implements the auto-clamp against `--max-priority-fee`).
pub async fn get_recent_prioritization_fees(
    client: &RpcClient,
    addresses: &[Pubkey],
) -> Result<Vec<RpcPrioritizationFee>> {
    let params = if addresses.is_empty() {
        json!([])
    } else {
        json!([addresses.iter().map(|a| a.to_string()).collect::<Vec<_>>()])
    };
    let raw: Value = client.post("getRecentPrioritizationFees", params).await?;
    let arr = raw.as_array().ok_or_else(|| {
        Error::Transport("getRecentPrioritizationFees: result not array".to_string())
    })?;
    serde_json::from_value::<Vec<RpcPrioritizationFee>>(Value::Array(arr.clone()))
        .map_err(|e| Error::Transport(format!("getRecentPrioritizationFees: parse: {e}")))
}

// (RpcPrioritizationFee defined above at the top with the other V0.1
// minimal wire-format structs)

// =============================================================================
// 13 — getVersion
// =============================================================================
/// Get the cluster's Solana version (cached at startup by Phase 5.1
/// for cluster-detection).
pub async fn get_version(client: &RpcClient) -> Result<Version> {
    let raw: Value = client.post("getVersion", json!([])).await?;
    let value = raw
        .get("value")
        .ok_or_else(|| Error::Transport("getVersion: missing value".to_string()))?;
    serde_json::from_value::<Version>(value.clone())
        .map_err(|e| Error::Transport(format!("getVersion: parse: {e}")))
}

// =============================================================================
// 14 — getEpochInfo
// =============================================================================
/// Get the current epoch info (slot, block height, epoch, etc).
pub async fn get_epoch_info(client: &RpcClient) -> Result<EpochInfo> {
    let raw: Value = client.post("getEpochInfo", json!([])).await?;
    let value = raw
        .get("value")
        .ok_or_else(|| Error::Transport("getEpochInfo: missing value".to_string()))?;
    serde_json::from_value::<EpochInfo>(value.clone())
        .map_err(|e| Error::Transport(format!("getEpochInfo: parse: {e}")))
}

// =============================================================================
// 15 — getHealth
// =============================================================================
/// Probe the cluster's health endpoint — returns `Ok(())` if the
/// node is `ok`, otherwise surfaces the JSON-RPC error verbatim
/// (typically `code: -32005` "Node is unhealthy").
pub async fn get_health(client: &RpcClient) -> Result<()> {
    let _: Value = client.post("getHealth", json!([])).await?;
    Ok(())
}

// =============================================================================
// Stubs for future Phase 5.2 / 5.3 / V0.1.5 methods
// =============================================================================
//
// `requestAirdrop` ships in Task 5.2 with the devnet host allowlist.
// `getTransaction` ships in Task 5.3 with `EncodedTransaction` decoding.
// The 5 WS subscribes (account_subscribe etc.) ship in V0.1.5.

/// Legacy alias for callers that used the prior Anza-`RpcClient`
/// naming. New code should use the free functions in this module.
pub type SolanaClient = RpcClient;

/// Stub for the prior `map_client_error` helper (no longer needed —
/// `RpcClient::post` returns `Error::Rpc` directly). Kept for source
/// compatibility with the stashed Phase 5 source.
#[allow(dead_code)]
pub fn map_client_error<E: std::fmt::Display>(_err: E) -> Error {
    Error::Transport("legacy helper: use RpcClient::post's typed errors".to_string())
}

// =============================================================================
// 16 — requestAirdrop (Task 5.2 — devnet-only helper)
// =============================================================================
//
// Hosts permitted to receive airdrops. Mainnet rejects `requestAirdrop`;
// this is a CLUSTER-level restriction (Tier 3 finding #6).
//
// Per grilled decision Q8: PER-METHOD host check (not a RpcClient mode
// flag). `request_airdrop` checks `rpc.host()` against this allowlist;
// `send` / `sendTransaction` work on any host (mainnet, devnet, testnet,
// local).
/// Hosts where `requestAirdrop` is permitted (devnet/testnet + local
/// devnet). Mainnet rejects airdrops; calling it accidentally is a
/// loss-of-funds + DoS vector — enforced per-method by
/// [`request_airdrop`].
pub const DEVNET_HOST_ALLOWLIST: &[&str] = &[
    "api.devnet.solana.com",
    "api.testnet.solana.com",
    "localhost",
    "127.0.0.1",
];

/// Devnet-only airdrop helper. Refuses mainnet / unknown hosts (Tier 3
/// finding #6 — mainnet rejects airdrops; calling it accidentally is a
/// loss-of-funds + DoS vector).
///
/// Returns the airdrop transaction signature on success. The actual
/// SOL appears after a few seconds (Anza convention) — callers should
/// `wait_for_confirm` if they need to know landing time.
pub async fn request_airdrop(rpc: &RpcClient, pubkey: &Pubkey, lamports: u64) -> Result<Signature> {
    let host = rpc.host();
    if !DEVNET_HOST_ALLOWLIST.contains(&host) {
        return Err(Error::Transport(format!(
            "requestAirdrop: host '{host}' not in devnet allowlist (mainnet rejects airdrop); allowed: {}",
            DEVNET_HOST_ALLOWLIST.join(", ")
        )));
    }
    let raw: Value = rpc
        .post("requestAirdrop", json!([pubkey.to_string(), lamports]))
        .await?;
    let sig_str = raw
        .as_str()
        .ok_or_else(|| Error::Transport("requestAirdrop: result not a string".to_string()))?;
    sig_str
        .parse::<Signature>()
        .map_err(|e| Error::Transport(format!("requestAirdrop: parse Signature: {e}")))
}

// =============================================================================
// 17 — getTransaction (Task 5.3 — full log decode for `sol tx`)
// =============================================================================

/// Local minimal wire struct for `getTransaction` response (Task 5.3).
///
/// The JSON-RPC `result` field has the shape:
/// ```json
/// {
///   "slot": 1234,
///   "blockTime": 1700000000,
///   "transaction": ...,
///   "meta": {...}
/// }
/// ```
///
/// Anza 4.x doesn't re-export the full `EncodedConfirmedTransactionWithStatusMeta`
/// from the 1.18-series `solana-transaction-status` family. V0.1 surfaces
/// just the slot + blockTime + an opaque `meta` JSON (Phase 7 `sol tx`
/// renders the inner fields lazily). Full decode (logs + token-balance
/// deltas + inner instructions) deferred to V0.1.5.
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponse {
    pub slot: u64,
    pub block_time: Option<i64>,
    /// Raw `meta` object — Phase 7 CLI formats selected fields (fee,
    /// status, log-truncation); full struct decode deferred.
    pub meta: serde_json::Value,
}

/// Fetch a confirmed transaction by signature, returning slot + blockTime
/// + raw `meta` JSON.
///
/// V0.1 ships a minimal struct (slot + blockTime + opaque meta) so Phase 7
/// `sol tx <signature>` can render the basics. Full `UiTransaction`
/// decode (logs, inner instructions, token-balance deltas, loaded
/// addresses) deferred to V0.1.5 when the 1.18-series wire types can
/// land without the #555 sub-dep conflict.
///
/// Params: `[signature, {"encoding": "json", "commitment": "confirmed", "maxSupportedTransactionVersion": 0}]`.
pub async fn get_transaction(
    rpc: &RpcClient,
    signature: &Signature,
) -> Result<Option<TransactionResponse>> {
    let raw: Value = rpc
        .post(
            "getTransaction",
            json!([
                signature.to_string(),
                {
                    "encoding": "json",
                    "commitment": "confirmed",
                    "maxSupportedTransactionVersion": 0
                }
            ]),
        )
        .await?;
    if raw.is_null() {
        return Ok(None);
    }
    serde_json::from_value::<TransactionResponse>(raw)
        .map(Some)
        .map_err(|e| Error::Transport(format!("getTransaction: parse TransactionResponse: {e}")))
}
