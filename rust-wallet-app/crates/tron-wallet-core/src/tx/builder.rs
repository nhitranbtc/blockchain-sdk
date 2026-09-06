//! Transaction parameter builders.
//!
//! Wraps `anychain_tron::trx::build_*_contract` so the rest of the crate only
//! deals in `TronTransactionParameters`. That type's proto fields are not part
//! of our public API — we re-export only the constructor-style wrappers we
//! actually need for v0.1.
//!
//! Each setter is a 1:1 delegate over `TronTransactionParameters::*`. They
//! exist to give call sites a name rather than letting raw field assignment
//! drift across the crate.

use anychain_tron::{trx, TronTransactionParameters};
use ethereum_types::U256;

use crate::error::{Error, Result};

/// Recommended `fee_limit` for a TRC-20 token transfer: 130 TRX (130 000 000 SUN).
///
/// Sized per Spike V5 — a held-recipient USDT transfer costs ~65 000 Energy;
/// an empty-recipient transfer (first-time receive) costs ~130 000 Energy.
/// The DEM `max_factor` 3.4× is applied by [`crate::resource::recommended_fee_limit`]
/// rather than here; this constant is the *baseline* for a single call.
pub const DEFAULT_TRC20_FEE_LIMIT_SUN: i64 = 130_000_000;

/// Build a `TronTransactionParameters` for a native TRX transfer.
///
/// `owner` and `recipient` are T-base58check addresses. `amount_sun` is in
/// SUN (1 TRX = 1_000_000 SUN). The caller is responsible for setting the
/// reference block, fee limit, timestamp, and expiration on the returned
/// value before signing — those belong to TAPOS and the validity window,
/// not the contract itself.
pub fn trx_transfer(
    owner: &str,
    recipient: &str,
    amount_sun: u64,
) -> Result<TronTransactionParameters> {
    // anychain-tron's parser is strict — surface invalid input via our
    // variant, not by letting the upstream `anychain_core::Error` escape.
    let amount_str = amount_sun.to_string();
    let contract = trx::build_transfer_contract(owner, recipient, &amount_str)
        .map_err(|e| Error::TransactionBuild(format!("transfer contract: {e}")))?;

    let mut params = TronTransactionParameters::default();
    params.set_contract(contract);
    Ok(params)
}

/// Set the reference block (the two-byte height + eight-byte hash TRON's
/// TAPOS rule pins against). `block_number` is the head block height;
/// `block_id_hex` is the 32-byte block id as hex.
pub fn set_ref_block(
    params: &mut TronTransactionParameters,
    block_number: i64,
    block_id_hex: &str,
) -> Result<()> {
    if block_id_hex.len() != 64 {
        return Err(Error::TransactionBuild(format!(
            "ref block id must be 32-byte hex (64 chars), got {}",
            block_id_hex.len()
        )));
    }
    // anychain_tron::set_ref_block panics on malformed hex; the length check
    // above is necessary but not sufficient. Keep that panic-prone call off
    // the hot path; this layer cannot fix it without forking.
    params.set_ref_block(block_number, block_id_hex);
    Ok(())
}

/// Set the smart-contract energy ceiling in SUN. Use 0 for native TRX
/// transfers (bandwidth-only).
pub fn set_fee_limit(params: &mut TronTransactionParameters, fee_limit_sun: i64) {
    params.set_fee_limit(fee_limit_sun);
}

/// Set the transaction timestamp in milliseconds since the Unix epoch.
pub fn set_timestamp(params: &mut TronTransactionParameters, ts_ms: i64) {
    params.set_timestamp(ts_ms);
}

/// Set the expiration window (milliseconds after the timestamp) before the
/// network stops accepting the transaction. `anychain-tron`'s default is
/// 5 minutes, which is what you get if you never call this.
pub fn set_expiration(params: &mut TronTransactionParameters, ttl_ms: i64) {
    params.set_expiration(ttl_ms);
}

/// Build a `TronTransactionParameters` for a TRC-20 `transfer(address,uint256)`.
///
/// `owner` is the holder whose tokens move. `contract` is the TRC-20 token
/// contract (e.g. `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` for mainnet USDT).
/// `recipient` is the destination. `amount` is the token amount in the
/// token's smallest unit (decimals depend on the contract — `trc20::decimals`
/// reads the value at runtime).
///
/// The returned parameters carry the contract envelope and the
/// [`DEFAULT_TRC20_FEE_LIMIT_SUN`] baseline. The caller still owns
/// reference block, timestamp, expiration, and any fee-limit override.
///
/// Returns `Error::TransactionBuild` if `anychain-tron` rejects the inputs
/// (unparsable addresses, amount overflow).
pub fn trc20_transfer(
    owner: &str,
    contract: &str,
    recipient: &str,
    amount: U256,
) -> Result<TronTransactionParameters> {
    let amount_str = amount.to_string();
    let contract_enum = trx::build_trc20_transfer_contract(owner, contract, recipient, &amount_str)
        .map_err(|e| Error::TransactionBuild(format!("trc20 transfer contract: {e}")))?;

    let mut params = TronTransactionParameters::default();
    params.set_contract(contract_enum);
    // Apply the recommended baseline. `set_fee_limit` is a no-op when the
    // value is already 0; here we explicitly set the TRC-20 baseline so
    // a caller that forgets to size it gets the documented default
    // rather than the native-transfer shape (0 = bandwidth-only).
    params.set_fee_limit(DEFAULT_TRC20_FEE_LIMIT_SUN);
    Ok(params)
}

/// Build a `TronTransactionParameters` for a TRC-20 `approve(address,uint256)`.
///
/// `spender` is the address authorised to pull tokens on behalf of `owner`.
/// The approval is unbounded in time and amount-limited — the caller is
/// responsible for picking an amount that matches the use case (often
/// `U256::MAX` for one-shot infinite approvals).
///
/// Same baseline fee-limit as [`trc20_transfer`]; the caller may override.
pub fn trc20_approve(
    owner: &str,
    contract: &str,
    spender: &str,
    amount: U256,
) -> Result<TronTransactionParameters> {
    let amount_str = amount.to_string();
    let contract_enum = trx::build_trc20_approve_contract(owner, contract, spender, &amount_str)
        .map_err(|e| Error::TransactionBuild(format!("trc20 approve contract: {e}")))?;

    let mut params = TronTransactionParameters::default();
    params.set_contract(contract_enum);
    params.set_fee_limit(DEFAULT_TRC20_FEE_LIMIT_SUN);
    Ok(params)
}
