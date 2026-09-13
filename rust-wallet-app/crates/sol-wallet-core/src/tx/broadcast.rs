//! `tx::broadcast` — `send_and_confirm` (Phase 5.1).
//!
//! Single send + poll with exponential backoff. V0.1 does NOT retry on
//! stale blockhash (deferred to V0.1.5 retry-on-stale-hash follow-up).
//!
//! ## Confirm semantics (grilled decision Q9)
//!
//! `wait_for_confirm` (internal) returns:
//! - `Ok(TransactionStatus)` on confirm
//! - `Err(ConfirmPending { signature, commitment, elapsed_ms })` at
//!   `timeout / 2` if SOME status was returned but commitment not yet
//!   reached (the tx is still valid, just slow)
//! - `Err(ConfirmTimeout { signature, waited_ms })` at full `timeout` if
//!   no status was ever returned (or no commitment reached)
//!
//! For `--finalized` on a stalled cluster, the tx is `Pending` (still
//! valid) at 15s, `Timeout` at 30s. CLI surfaces "tx pending — check
//! <explorer>" for `Pending`, "tx may or may not land — check <explorer>"
//! for `Timeout`.
//!
//! ## Why no retry on stale hash
//!
//! V0.1 ships single-send; if the cluster drops the blockhash (rare on
//! devnet, ~1%), the user re-runs `sol send`. V0.1.5 ships
//! `send_with_retry` with 3 attempts, exponential backoff 100ms→200ms→400ms,
//! and `BlockhashCache` (V0.1.5). Plan §Phase 5 Task 5.1 deferred.

use std::time::{Duration, Instant};

use crate::chain::account::TransactionStatus;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::Transaction;

use crate::chain::account::{
    get_balance, get_latest_blockhash, get_signature_status, send_transaction,
};
use crate::chain::client::RpcClient;
use crate::error::{Error, Result};

/// Default max-attempts for `send_and_confirm`. V0.1 = 1 (no retry);
/// V0.1.5 = 3 with exponential backoff.
pub const DEFAULT_SEND_MAX_ATTEMPTS: u32 = 1;

/// Default confirm timeout for `send_and_confirm`. 30s covers
/// `confirmed` (~1 slot = ~400ms) + headroom; `Finalized` sends
/// (~12 slots = ~5s) also fit comfortably. Stalled cluster: see
/// `ConfirmPending` (returned at `timeout / 2`).
pub const DEFAULT_CONFIRM_TIMEOUT: Duration = Duration::from_secs(30);

/// Base backoff between confirm-polls (doubles each iteration, capped at 2s).
const CONFIRM_POLL_BASE: Duration = Duration::from_millis(200);
const CONFIRM_POLL_CAP: Duration = Duration::from_secs(2);

/// Send `tx` signed by the wallet, then poll for confirmation.
///
/// `commitment` defaults to `Confirmed` (per plan doc open design
/// questions 2026-09-10 session-end). `timeout` defaults to
/// `DEFAULT_CONFIRM_TIMEOUT` (30s). On `Confirmed`, returns
/// `Ok(TransactionStatus)` once the sig reaches the requested level.
/// On `Finalized`, the poll waits up to ~12 slots (cluster-dependent).
pub async fn send_and_confirm(
    rpc: &RpcClient,
    tx: &Transaction,
    commitment: CommitmentConfig,
    timeout: Duration,
) -> Result<Signature> {
    // Phase 5.1: single send. V0.1.5 wraps this in a 3-attempt loop
    // (see plan §Phase 5 Q7 + grilled decision R1-Q1).
    let (_blockhash, _slot) = get_latest_blockhash(rpc).await?;

    // Single send attempt. Caller is responsible for signing the
    // message with the fetched blockhash (Phase 7 CLI handler).
    let sig = send_transaction(rpc, tx).await?;

    // Poll for confirmation
    let _status = wait_for_confirm(rpc, &sig, commitment, timeout).await?;
    Ok(sig)
}

/// Wait for `signature` to reach the requested commitment, polling
/// `getSignatureStatus` with exponential backoff (200ms→400ms→...→2s).
///
/// Returns:
/// - `Ok(TransactionStatus)` on confirm
/// - `Err(ConfirmPending { signature, commitment, elapsed_ms })` at
///   `timeout / 2` if some status returned but commitment not reached
/// - `Err(ConfirmTimeout { signature, waited_ms })` at full `timeout`
///   if no status returned or no commitment reached
pub async fn wait_for_confirm(
    rpc: &RpcClient,
    signature: &Signature,
    poll_commitment: CommitmentConfig,
    timeout: Duration,
) -> Result<TransactionStatus> {
    let deadline = Instant::now() + timeout;
    let half_deadline = Instant::now() + timeout / 2;
    let mut poll_backoff = CONFIRM_POLL_BASE;
    let start = Instant::now();

    loop {
        let statuses = match get_signature_status(rpc, signature).await? {
            Some(s) => vec![Some(s)],
            None => vec![None],
        };
        if let Some(Some(status)) = statuses.into_iter().next() {
            let reached = status_satisfies_commitment(&status, &poll_commitment);
            if reached {
                return Ok(status);
            }
        }
        let now = Instant::now();
        if now >= half_deadline {
            // Status was returned but commitment not yet reached —
            // surface `ConfirmPending` so the CLI can offer "check
            // explorer" instead of treating it as a hard failure.
            return Err(Error::ConfirmPending {
                signature: signature.to_string(),
                commitment: poll_commitment,
                elapsed_ms: now.duration_since(start).as_millis() as u64,
            });
        }
        if now >= deadline {
            return Err(Error::ConfirmTimeout {
                signature: signature.to_string(),
                waited_ms: timeout.as_millis() as u64,
            });
        }
        tokio::time::sleep(poll_backoff).await;
        poll_backoff = (poll_backoff * 2).min(CONFIRM_POLL_CAP);
    }
}

/// Phase 6.4 Step 3 — authoritative landing proof: wait for cluster
/// confirmation (tolerating `ConfirmPending` / `ConfirmTimeout`) AND
/// poll `get_balance` until the sender's lamport balance decreases by
/// at least `expected_delta_lamports`. The balance-delta poll is the
/// ground-truth landing proof — the cluster can report `confirmed`
/// while the actual SOL debit has not propagated yet (rare on devnet,
/// common on stalled testnet/mainnet-beta during congestion).
///
/// Args:
/// - `pre_balance`: sender's SOL balance captured BEFORE broadcast.
/// - `expected_delta_lamports`: minimum lamport decrease to accept as
///   landed (= transfer amount + 5000 lamport base fee for native SOL;
///   for SPL transfers set to 5000 to detect fee-only debits, then
///   inspect the SPL balance separately).
/// - `timeout`: upper bound on the WHOLE flow (confirm + balance poll).
///
/// Returns the post-balance (sender's lamports) on landing. Returns
/// `Error::ConfirmTimeout` if the deadline elapses before balance
/// decreased — caller can re-poll or surface the explorer URL.
///
/// Security properties (audit 2026-09-13):
/// - **No panic / no manual RPC**: wraps `wait_for_confirm` +
///   `chain::account::get_balance`. Callers cannot forget to set the
///   deadline or skip the balance poll.
/// - **Error taxonomy preserved**: `ConfirmPending` / `ConfirmTimeout`
///   from the inner confirm are tolerated — the balance poll is the
///   ground truth. A hard timeout still surfaces as `ConfirmTimeout`
///   so the CLI / FFI can present a consistent error to the user.
pub async fn wait_for_landing(
    rpc: &RpcClient,
    signature: &Signature,
    sender_pubkey: &Pubkey,
    pre_balance: u64,
    expected_delta_lamports: u64,
    timeout: Duration,
) -> Result<u64> {
    // Step 1: cluster confirmation. Tolerate `Err` — the tx may still
    // land; the balance-delta poll below is authoritative.
    let _ = wait_for_confirm(rpc, signature, CommitmentConfig::confirmed(), timeout).await;
    // Step 2: poll balance until `pre_balance - cur >= expected_delta`.
    let deadline_at = Instant::now() + timeout;
    let target_threshold = pre_balance.saturating_sub(expected_delta_lamports);
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let cur = get_balance(rpc, sender_pubkey).await?;
        if cur <= target_threshold {
            return Ok(cur);
        }
        if Instant::now() >= deadline_at {
            return Err(Error::ConfirmTimeout {
                signature: signature.to_string(),
                waited_ms: timeout.as_millis() as u64,
            });
        }
    }
}

/// Map `CommitmentLevel` to a numeric rank so we can compare levels
/// without relying on `PartialOrd` (which Anza 4.x `CommitmentLevel`
/// does not implement).
fn level_rank(level: solana_commitment_config::CommitmentLevel) -> u8 {
    use solana_commitment_config::CommitmentLevel as L;
    match level {
        L::Processed => 0,
        L::Confirmed => 1,
        L::Finalized => 2,
    }
}

/// Check whether a `TransactionStatus` satisfies the requested commitment.
fn status_satisfies_commitment(status: &TransactionStatus, commitment: &CommitmentConfig) -> bool {
    let level = level_rank(commitment.commitment);
    if let Some(ref conf_status) = status.confirmation_status {
        let status_level = match conf_status {
            crate::chain::account::ConfirmationStatus::Processed => 0,
            crate::chain::account::ConfirmationStatus::Confirmed => 1,
            crate::chain::account::ConfirmationStatus::Finalized => 2,
        };
        // Higher commitment levels satisfy lower ones (Finalized is
        // also Confirmed and Processed; Confirmed is also Processed).
        return status_level >= level;
    }
    // No confirmation_status field (e.g. very old format or
    // `getTransaction` result) — fall back to `status.confirmations`
    // (None + signature root = "finalized" per legacy semantics).
    status.confirmations.is_none()
}
