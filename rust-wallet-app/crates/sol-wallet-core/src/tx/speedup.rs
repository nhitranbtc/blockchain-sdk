//! `tx::speedup` — Phase 8.5 fee-bump resubmission (deep-dive row 35).
//!
//! Solana has no RBF. The "speed up" workflow is: re-sign the same
//! instructions with a higher Compute Budget priority fee and a fresh
//! `recent_blockhash`, then broadcast. The signature cannot match the
//! original because (a) the blockhash changed and (b) the fee ix changed.
//!
//! Blockhash policy: **always fresh** per user grill 2026-09-14. The
//! caller surfaces `expires_at_slot` so the UI can warn when the
//! returned sig is about to age out.

use std::time::Duration;

use serde::Serialize;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::{v0::Message as V0Message, VersionedMessage},
    pubkey::Pubkey,
    signature::Signature,
    signer::Signer,
    transaction::VersionedTransaction,
};

use crate::wallet::Wallet;

use crate::chain::{account::get_latest_blockhash, client::RpcClient};
use crate::tx::broadcast::{default_send_options, send_and_confirm};
use crate::tx::builder::compute_budget_instructions;
use crate::Error;

/// Compute Budget program — well-known address
/// `ComputeBudget111111111111111111111111111111`. `solana_sdk::pubkey!`
/// parses the base58 string at compile time (no runtime cost).
fn compute_budget_program_id() -> Pubkey {
    solana_sdk::pubkey!("ComputeBudget111111111111111111111111111111")
}

/// Default Compute Unit limit budget — mirrors the Q8 plan default
/// (150_000). Bumped fee is the only knob the surface exposes; CU limit
/// stays at the same safety margin.
pub const DEFAULT_COMPUTE_UNIT_LIMIT: u32 = 150_000;

/// Maximum priority fee the surface will accept. Plan Q8 (V0.1)
/// decision — caps a misuse before it broadcasts.
pub const PRIORITY_FEE_CEILING_MICRO_LAMPORTS: u64 = 10_000_000;

/// Error variants surfaced by `speedup_transfer`.
#[derive(Debug, Serialize)]
pub enum SpeedupError {
    InsufficientBump {
        current_fee: u64,
        requested_fee: u64,
    },
    PriorityFeeExceedsCeiling {
        requested: u64,
        ceiling: u64,
    },
    Decode {
        message: String,
    },
    RpcTransport(String),
    BroadcastFailed(String),
    Other(String),
}

impl std::fmt::Display for SpeedupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientBump {
                current_fee,
                requested_fee,
            } => write!(
                f,
                "speedup: requested priority fee {requested_fee} <= original {current_fee}"
            ),
            Self::PriorityFeeExceedsCeiling { requested, ceiling } => write!(
                f,
                "speedup: requested priority fee {requested} exceeds ceiling {ceiling}"
            ),
            Self::Decode { message } => write!(f, "speedup: decode failed — {message}"),
            Self::RpcTransport(m) => write!(f, "speedup: RPC transport — {m}"),
            Self::BroadcastFailed(m) => write!(f, "speedup: broadcast — {m}"),
            Self::Other(m) => write!(f, "speedup: {m}"),
        }
    }
}

impl std::error::Error for SpeedupError {}

impl From<Error> for SpeedupError {
    fn from(e: Error) -> Self {
        match e {
            Error::Transport(m) => Self::RpcTransport(m),
            Error::Rpc { .. } => Self::RpcTransport(format!("RPC error envelope: {e}")),
            Error::BroadcastFailed { .. } => Self::BroadcastFailed(e.to_string()),
            other => Self::Other(other.to_string()),
        }
    }
}

/// Caller-facing request to bump an on-chain transaction's priority
/// fee. The caller passes the already-decoded original
/// `VersionedTransaction` (via `VersionedTransaction::try_from_slice` on
/// the wire bytes returned by `getTransaction`). Blockhash reuse is
/// out of scope — `speedup_transfer` always fetches a fresh one.
#[derive(Clone, Debug)]
pub struct SpeedupRequest {
    pub original_signature: Signature,
    pub original_transaction: VersionedTransaction,
    pub new_priority_fee_micro_lamports: u64,
    pub commitment: CommitmentConfig,
    pub timeout: Duration,
}

/// Result returned to caller (and surfaced to the FFI / CLI layers).
#[derive(Clone, Debug)]
pub struct SpeedupResult {
    pub new_signature: Signature,
    pub expires_at_slot: u64,
}

/// Resubmit `original_transaction` (sans Compute Budget ixes) with the
/// given priority fee and a fresh blockhash.
pub async fn speedup_transfer(
    rpc: &RpcClient,
    wallet: &Wallet,
    request: SpeedupRequest,
) -> std::result::Result<SpeedupResult, SpeedupError> {
    if request.new_priority_fee_micro_lamports > PRIORITY_FEE_CEILING_MICRO_LAMPORTS {
        return Err(SpeedupError::PriorityFeeExceedsCeiling {
            requested: request.new_priority_fee_micro_lamports,
            ceiling: PRIORITY_FEE_CEILING_MICRO_LAMPORTS,
        });
    }

    // Decode the original's Compute Budget priority fee.
    let original_priority_fee = extract_compute_unit_price(&request.original_transaction)
        .ok_or_else(|| SpeedupError::Decode {
            message: "no Compute Budget priority-fee ix in original tx".to_string(),
        })?;

    if request.new_priority_fee_micro_lamports <= original_priority_fee {
        return Err(SpeedupError::InsufficientBump {
            current_fee: original_priority_fee,
            requested_fee: request.new_priority_fee_micro_lamports,
        });
    }

    // Resolve the original transaction's instructions back to
    // `Instruction` form (resolving account indexes via `account_keys`)
    // and strip Compute Budget ixes (program == ComputeBudget).
    let resolved_ixs = decompile_instructions(&request.original_transaction)?;
    let cb_pid = compute_budget_program_id();
    let stripped: Vec<Instruction> = resolved_ixs
        .into_iter()
        .filter(|ix| ix.program_id != cb_pid)
        .collect();

    // Fetch a fresh blockhash.
    let (blockhash, last_valid_slot) = get_latest_blockhash(rpc)
        .await
        .map_err(SpeedupError::from)?;

    // Re-prepend Compute Budget ixes with the bumped fee.
    let new_budget = compute_budget_instructions(
        DEFAULT_COMPUTE_UNIT_LIMIT,
        request.new_priority_fee_micro_lamports,
    );
    let mut all_ixs = Vec::with_capacity(stripped.len() + new_budget.len());
    all_ixs.extend_from_slice(&new_budget);
    all_ixs.extend(stripped);

    // Build V0 message + sign via the wallet's internal keypair.
    let v0_msg =
        V0Message::try_compile(&wallet.public_key(), &all_ixs, &[], blockhash).map_err(|e| {
            SpeedupError::Decode {
                message: format!("v0::Message::try_compile: {e}"),
            }
        })?;
    let versioned_tx =
        VersionedTransaction::try_new(VersionedMessage::V0(v0_msg), &[wallet.keypair()]).map_err(
            |e| SpeedupError::Decode {
                message: format!("VersionedTransaction::try_new: {e}"),
            },
        )?;

    let new_signature = send_and_confirm(
        rpc,
        &versioned_tx,
        default_send_options(),
        request.commitment,
        request.timeout,
    )
    .await
    .map_err(SpeedupError::from)?;

    let _ = request.original_signature;
    Ok(SpeedupResult {
        new_signature,
        expires_at_slot: last_valid_slot,
    })
}

/// Resolve `VersionedTransaction::message` instructions back to
/// `Instruction` form by looking up `program_id_index` and account
/// indexes via `account_keys`.
fn decompile_instructions(
    tx: &VersionedTransaction,
) -> std::result::Result<Vec<Instruction>, SpeedupError> {
    let (static_keys, instructions) = match &tx.message {
        VersionedMessage::Legacy(m) => (m.account_keys.clone(), m.instructions.clone()),
        VersionedMessage::V0(m) => (m.account_keys.clone(), m.instructions.clone()),
        // V1 transactions (Anza 4.x) include lookup-table addresses
        // in `static_keys` already; the V0 decompilation path applies.
        VersionedMessage::V1(m) => (m.account_keys.clone(), m.instructions.clone()),
    };
    let mut out = Vec::with_capacity(instructions.len());
    for ci in instructions {
        let pidx = ci.program_id_index as usize;
        if pidx >= static_keys.len() {
            return Err(SpeedupError::Decode {
                message: format!("program_id_index {pidx} out of bounds"),
            });
        }
        let program_id = static_keys[pidx];
        let mut accounts = Vec::with_capacity(ci.accounts.len());
        for &ai in &ci.accounts {
            if ai as usize >= static_keys.len() {
                return Err(SpeedupError::Decode {
                    message: format!("account index {ai} out of bounds"),
                });
            }
            // Derive signer + writable flags from the message header.
            // `CompiledInstruction::accounts` carries only indexes;
            // flags live in `MessageHeader`. Without these the rebuilt
            // ix can't carry a valid signature because `try_compile`
            // won't mark the payer as a signer.
            let key_idx = ai as usize;
            let key = static_keys[key_idx];
            let header = match &tx.message {
                VersionedMessage::Legacy(m) => m.header,
                VersionedMessage::V0(m) => m.header,
                VersionedMessage::V1(m) => m.header,
            };
            let nrs = header.num_required_signatures as usize;
            let nrs_ro = header.num_readonly_signed_accounts as usize;
            let nru_ro = header.num_readonly_unsigned_accounts as usize;
            let total = static_keys.len();
            let is_signer = key_idx < nrs;
            let is_writable = if is_signer {
                // Signed writable: `[nrs - nrs_ro, nrs)`.
                key_idx >= nrs - nrs_ro
            } else {
                // Unsigned writable: `[nrs, total - nru_ro)`.
                key_idx < total - nru_ro
            };
            accounts.push(AccountMeta {
                pubkey: key,
                is_signer,
                is_writable,
            });
        }
        out.push(Instruction {
            program_id,
            accounts,
            data: ci.data.clone(),
        });
    }
    let _ = static_keys; // moved ownership
    Ok(out)
}

/// Decode the `SetComputeUnitPrice` ix data (tag=3 u32 LE + 8 bytes price
/// LE — total 12 bytes). Returns `None` if no `SetComputeUnitPrice`
/// ix is present.
fn extract_compute_unit_price(tx: &VersionedTransaction) -> Option<u64> {
    let cb_pid = compute_budget_program_id();
    let (instructions, account_keys): (Vec<_>, &[Pubkey]) = match &tx.message {
        VersionedMessage::Legacy(m) => (m.instructions.clone(), &m.account_keys),
        VersionedMessage::V0(m) => (m.instructions.clone(), &m.account_keys),
        VersionedMessage::V1(m) => (m.instructions.clone(), &m.account_keys),
    };
    for ci in &instructions {
        // Anza ComputeBudgetInstruction wire format (bincode with single-byte
        // variant tag): byte 0 = tag (2 = SetLimit, 3 = SetPrice), bytes 1..9 =
        // u64 little-endian micro_lamports (SetPrice only).
        if ci.data.is_empty() || ci.data[0] != 3 || ci.data.len() < 9 {
            continue;
        }
        let pidx = ci.program_id_index as usize;
        if pidx >= account_keys.len() || account_keys[pidx] != cb_pid {
            continue;
        }
        let price_bytes: [u8; 8] = ci.data[1..9].try_into().ok()?;
        return Some(u64::from_le_bytes(price_bytes));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::Keypair;

    fn fixture_versioned_with_price(price: u64) -> VersionedTransaction {
        let keypair = Keypair::new();
        let recipient = Pubkey::new_unique();
        let mut data = Vec::with_capacity(12);
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&price.to_le_bytes());
        let cb_ix = Instruction {
            program_id: compute_budget_program_id(),
            accounts: vec![],
            data,
        };
        // System program ID last byte = 1.
        let mut sys = [0u8; 32];
        sys[31] = 1;
        let mut xfer = Vec::with_capacity(12);
        xfer.extend_from_slice(&2u32.to_le_bytes());
        xfer.extend_from_slice(&1_000u64.to_le_bytes());
        let xfer_ix = Instruction {
            program_id: Pubkey::new_from_array(sys),
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(keypair.pubkey(), true),
                solana_sdk::instruction::AccountMeta::new(recipient, false),
            ],
            data: xfer,
        };
        let v0 = V0Message::try_compile(
            &keypair.pubkey(),
            &[cb_ix, xfer_ix],
            &[],
            solana_sdk::hash::Hash::from([7u8; 32]),
        )
        .expect("v0 compile");
        VersionedTransaction::try_new(VersionedMessage::V0(v0), &[&keypair]).expect("sign v0")
    }

    #[test]
    fn decode_priority_fee() {
        let tx = fixture_versioned_with_price(5_000);
        assert_eq!(extract_compute_unit_price(&tx), Some(5_000));
    }

    #[test]
    fn decode_priority_fee_zero_is_visible() {
        let tx = fixture_versioned_with_price(0);
        assert_eq!(extract_compute_unit_price(&tx), Some(0));
    }

    #[test]
    fn priority_fee_ceiling_constant_matches_q8() {
        assert_eq!(PRIORITY_FEE_CEILING_MICRO_LAMPORTS, 10_000_000);
    }

    #[test]
    fn default_compute_unit_limit_matches_q8() {
        assert_eq!(DEFAULT_COMPUTE_UNIT_LIMIT, 150_000);
    }

    #[test]
    fn decompile_resolves_program_and_accounts() {
        let tx = fixture_versioned_with_price(7);
        let ixs = decompile_instructions(&tx).expect("decompile");
        assert_eq!(ixs.len(), 2, "CB + System transfer");
        assert_eq!(
            ixs[0].program_id,
            compute_budget_program_id(),
            "first ix is CB"
        );
        // The System transfer ix should be present and not the CB.
        assert_ne!(
            ixs[1].program_id,
            compute_budget_program_id(),
            "second ix is System"
        );
    }
}
