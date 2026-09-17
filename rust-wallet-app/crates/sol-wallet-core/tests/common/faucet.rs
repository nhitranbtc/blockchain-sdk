//! Test-only airdrop helper against a surfpool RPC endpoint.
//!
//! Bypasses the deleted `chain::account::request_airdrop` wrapper and
//! posts the JSON-RPC `requestAirdrop` method directly. Intended for
//! local surfpool validators only — caller MUST pin `rpc_url` to a
//! localhost endpoint; the devnet allowlist guard from the old wrapper
//! is intentionally not enforced here (this helper is test-only and
//! surfpool rejects mainnet traffic at the transport level).

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sol_wallet_core::chain::client::RpcClient;
use solana_sdk::{pubkey::Pubkey, signature::Signature};

const AIRDROP_DEADLINE: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Errors that can surface from `airdrop_to_keypair`.
#[derive(Debug)]
pub enum FaucetError {
    Client(String),
    Airdrop(String),
    Timeout,
}

impl std::fmt::Display for FaucetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Client(e) => write!(f, "RpcClient::new failed: {e}"),
            Self::Airdrop(e) => write!(f, "requestAirdrop RPC failed: {e}"),
            Self::Timeout => write!(f, "airdrop did not settle within {AIRDROP_DEADLINE:?}"),
        }
    }
}

impl std::error::Error for FaucetError {}

/// Request `lamports` for `recipient` from the surfpool faucet.
pub async fn airdrop_to_keypair(
    rpc_url: &str,
    recipient: &Pubkey,
    lamports: u64,
) -> Result<Signature, FaucetError> {
    let rpc = RpcClient::new(rpc_url).map_err(|e| FaucetError::Client(e.to_string()))?;
    let deadline = Instant::now() + AIRDROP_DEADLINE;
    loop {
        match raw_request_airdrop(&rpc, recipient, lamports).await {
            Ok(sig) => return Ok(sig),
            Err(e) if Instant::now() >= deadline => {
                return Err(FaucetError::Airdrop(format!("{e} (deadline elapsed)")));
            }
            Err(_) => {
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
    }
}

/// Post `requestAirdrop` JSON-RPC directly. Replaces the deleted
/// `chain::account::request_airdrop` wrapper; localhost-only by
/// convention (see module docs).
async fn raw_request_airdrop(
    rpc: &RpcClient,
    recipient: &Pubkey,
    lamports: u64,
) -> Result<Signature, String> {
    let raw: Value = rpc
        .post("requestAirdrop", json!([recipient.to_string(), lamports]))
        .await
        .map_err(|e| e.to_string())?;
    let sig_str = raw
        .as_str()
        .ok_or_else(|| "requestAirdrop: result not a string".to_string())?;
    sig_str
        .parse::<Signature>()
        .map_err(|e| format!("requestAirdrop: parse Signature: {e}"))
}

/// Credit `lamports` and wait for the recipient balance to reflect the credit.
pub async fn airdrop_and_wait(
    rpc_url: &str,
    recipient: &Pubkey,
    lamports: u64,
    pre_balance: u64,
) -> Result<(Signature, u64), FaucetError> {
    let sig = airdrop_to_keypair(rpc_url, recipient, lamports).await?;
    let rpc = RpcClient::new(rpc_url).map_err(|e| FaucetError::Client(e.to_string()))?;
    let target = pre_balance.saturating_add(lamports);
    let deadline = Instant::now() + AIRDROP_DEADLINE;
    loop {
        match sol_wallet_core::chain::account::get_balance(&rpc, recipient).await {
            Ok(b) if b >= target => return Ok((sig, b)),
            Ok(_) => {}
            Err(_) => {}
        }
        if Instant::now() >= deadline {
            return Err(FaucetError::Timeout);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
