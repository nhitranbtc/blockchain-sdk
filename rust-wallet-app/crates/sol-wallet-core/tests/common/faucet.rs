//! Test-only airdrop helper against a surfpool RPC endpoint.

use std::time::{Duration, Instant};

use sol_wallet_core::chain::{account::request_airdrop, client::RpcClient};
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
            Self::Airdrop(e) => write!(f, "request_airdrop failed: {e}"),
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
        match request_airdrop(&rpc, recipient, lamports).await {
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
