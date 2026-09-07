//! End-to-end transaction submission — build, sign, broadcast, confirm.
//!
//! Phase 2/3 carry-over closed for plan §Phase 6. The pieces already existed
//! separately ([`crate::tx::builder`] shapes parameters, [`crate::tx::sign`]
//! signs, [`crate::chain::TronGridClient::broadcast`] ships); what was missing
//! was one place that sequences them. That sequence belongs in the core rather
//! than in the CLI so the CLI and the eventual FFI cannot drift on TAPOS
//! handling, fee-limit defaults, or the expiration window.
//!
//! ## The three-step shape
//!
//! Every mutation follows: `prepare_*` (contract + ref-block + timestamp +
//! fee limit) → [`sign_prepared`] → [`broadcast_signed`]. `submit_*` runs all
//! three. Callers that want `--dry-run` stop after `prepare_*`; callers that
//! want `--sign-only` stop after [`sign_prepared`].
//!
//! ## Broadcast rejection is not an error here
//!
//! [`Submitted`] carries the node's [`BroadcastReceipt`] verbatim, including a
//! `success: false` rejection. That is deliberate: a rejection ("insufficient
//! balance", "bandwidth") is a *chain-state answer* about a transaction that
//! was signed correctly, and it arrives with a txid the operator may need. The
//! caller decides how loudly to fail.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anychain_tron::TronTransactionParameters;
use ethereum_types::U256;
use zeroize::Zeroizing;

use crate::chain::{OriginalCall, TronGridClient};
use crate::error::{Error, Result};
use crate::keys::SECRET_KEY_LEN;
use crate::trc20;
use crate::tx::broadcast::{BroadcastReceipt, TransactionInfo};
use crate::tx::builder;
use crate::tx::sign::{sign_tx, SignedTransaction};

/// Default validity window: 60 s. Anychain's own default is 5 minutes, which
/// leaves a signed transfer replayable for far longer than a CLI invocation
/// needs.
pub const DEFAULT_EXPIRATION_MS: i64 = 60_000;

/// Per-call knobs that are not part of the contract itself.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubmitOptions {
    /// Energy ceiling in SUN. `None` keeps the builder's default: 0 for native
    /// TRX (bandwidth only), [`builder::DEFAULT_TRC20_FEE_LIMIT_SUN`] for
    /// contract calls.
    pub fee_limit_sun: Option<i64>,
    /// Validity window in ms after the timestamp. `None` ⇒
    /// [`DEFAULT_EXPIRATION_MS`].
    pub expiration_ms: Option<i64>,
}

/// A signed transaction plus whatever the node said about it.
#[derive(Debug, Clone)]
pub struct Submitted {
    /// The signed envelope. `signed.txid_hex()` is the id to poll.
    pub signed: SignedTransaction,
    /// The node's answer. Check `receipt.is_success()`.
    pub receipt: BroadcastReceipt,
}

/// Wall-clock milliseconds. A transaction whose timestamp is in the future
/// (clock skew) is rejected by the network, so this is not something a caller
/// should be asked to supply.
fn now_ms() -> Result<i64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::TransactionBuild(format!("system clock before epoch: {e}")))?
        .as_millis();
    i64::try_from(millis)
        .map_err(|_| Error::TransactionBuild("system clock past i64 milliseconds".into()))
}

/// Applies TAPOS (reference block), timestamp, expiration, and any fee-limit
/// override to freshly built parameters.
async fn finalise(
    rpc: &TronGridClient,
    params: &mut TronTransactionParameters,
    opts: SubmitOptions,
) -> Result<()> {
    let head = rpc.get_now_block().await?;
    let height = i64::try_from(head.block_number).map_err(|_| {
        Error::TransactionBuild(format!("block height {} exceeds i64", head.block_number))
    })?;
    builder::set_ref_block(params, height, &head.block_id)?;
    builder::set_timestamp(params, now_ms()?);
    builder::set_expiration(params, opts.expiration_ms.unwrap_or(DEFAULT_EXPIRATION_MS));
    if let Some(limit) = opts.fee_limit_sun {
        if limit < 0 {
            return Err(Error::TransactionBuild(format!(
                "fee limit must not be negative, got {limit}"
            )));
        }
        builder::set_fee_limit(params, limit);
    }
    Ok(())
}

/// Build a native TRX transfer, ready to sign.
pub async fn prepare_trx(
    rpc: &TronGridClient,
    owner: &str,
    to: &str,
    amount_sun: u64,
    opts: SubmitOptions,
) -> Result<TronTransactionParameters> {
    if amount_sun == 0 {
        return Err(Error::TransactionBuild(
            "amount must be greater than zero".into(),
        ));
    }
    let mut params = builder::trx_transfer(owner, to, amount_sun)?;
    finalise(rpc, &mut params, opts).await?;
    Ok(params)
}

/// Build a TRC-20 `transfer(address,uint256)`, ready to sign.
pub async fn prepare_trc20(
    rpc: &TronGridClient,
    owner: &str,
    contract: &str,
    to: &str,
    amount: U256,
    opts: SubmitOptions,
) -> Result<TronTransactionParameters> {
    if amount.is_zero() {
        return Err(Error::TransactionBuild(
            "amount must be greater than zero".into(),
        ));
    }
    let mut params = builder::trc20_transfer(owner, contract, to, amount)?;
    finalise(rpc, &mut params, opts).await?;
    Ok(params)
}

/// Build a TRC-20 `approve(address,uint256)`, ready to sign.
///
/// A zero amount is legal here — it is the revocation form.
pub async fn prepare_trc20_approve(
    rpc: &TronGridClient,
    owner: &str,
    contract: &str,
    spender: &str,
    amount: U256,
    opts: SubmitOptions,
) -> Result<TronTransactionParameters> {
    let mut params = builder::trc20_approve(owner, contract, spender, amount)?;
    finalise(rpc, &mut params, opts).await?;
    Ok(params)
}

/// Sign prepared parameters. Thin alias over [`crate::tx::sign::sign_tx`], kept
/// so callers of this module never need two imports for one flow.
pub fn sign_prepared(
    secret: &Zeroizing<[u8; SECRET_KEY_LEN]>,
    params: &TronTransactionParameters,
) -> Result<SignedTransaction> {
    sign_tx(secret, params)
}

/// Ship a signed envelope.
pub async fn broadcast_signed(
    rpc: &TronGridClient,
    signed: &SignedTransaction,
) -> Result<BroadcastReceipt> {
    rpc.broadcast(&signed.signed_envelope_hex).await
}

/// Native TRX transfer: prepare → sign → broadcast.
pub async fn submit_trx(
    rpc: &TronGridClient,
    secret: &Zeroizing<[u8; SECRET_KEY_LEN]>,
    owner: &str,
    to: &str,
    amount_sun: u64,
    opts: SubmitOptions,
) -> Result<Submitted> {
    let params = prepare_trx(rpc, owner, to, amount_sun, opts).await?;
    let signed = sign_prepared(secret, &params)?;
    let receipt = broadcast_signed(rpc, &signed).await?;
    Ok(Submitted { signed, receipt })
}

/// TRC-20 transfer: prepare → sign → broadcast.
pub async fn submit_trc20(
    rpc: &TronGridClient,
    secret: &Zeroizing<[u8; SECRET_KEY_LEN]>,
    owner: &str,
    contract: &str,
    to: &str,
    amount: U256,
    opts: SubmitOptions,
) -> Result<Submitted> {
    let params = prepare_trc20(rpc, owner, contract, to, amount, opts).await?;
    let signed = sign_prepared(secret, &params)?;
    let receipt = broadcast_signed(rpc, &signed).await?;
    Ok(Submitted { signed, receipt })
}

/// TRC-20 approve: prepare → sign → broadcast.
pub async fn submit_trc20_approve(
    rpc: &TronGridClient,
    secret: &Zeroizing<[u8; SECRET_KEY_LEN]>,
    owner: &str,
    contract: &str,
    spender: &str,
    amount: U256,
    opts: SubmitOptions,
) -> Result<Submitted> {
    let params = prepare_trc20_approve(rpc, owner, contract, spender, amount, opts).await?;
    let signed = sign_prepared(secret, &params)?;
    let receipt = broadcast_signed(rpc, &signed).await?;
    Ok(Submitted { signed, receipt })
}

/// A TRC-20 call recovered from raw calldata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trc20Call {
    /// `transfer(address,uint256)`.
    Transfer { to: String, amount: U256 },
    /// `approve(address,uint256)`.
    Approve { spender: String, amount: U256 },
}

/// Decode `transfer` / `approve` calldata back into typed arguments.
///
/// Needed by [`submit_send_speedup`]: TRON has no fee-bump primitive, so a
/// "speed-up" is a *new* transaction carrying the same intent, and the intent
/// has to be recovered from the original call. Only the two selectors this
/// crate can rebuild are accepted — anything else is refused rather than
/// guessed, because signing a guessed call moves someone's funds.
pub fn decode_trc20_call(data_hex: &str) -> Result<Trc20Call> {
    let raw = hex::decode(data_hex.trim_start_matches("0x"))
        .map_err(|e| Error::TransactionBuild(format!("calldata is not hex: {e}")))?;
    if raw.len() < 4 + 32 + 32 {
        return Err(Error::TransactionBuild(format!(
            "calldata is {} bytes; transfer/approve need 68",
            raw.len()
        )));
    }
    let selector: [u8; 4] = raw[..4].try_into().expect("checked length");
    let address = evm_word_to_t_address(&raw[4..36])?;
    let amount = U256::from_big_endian(&raw[36..68]);

    if selector == trc20::TRANSFER_SELECTOR {
        Ok(Trc20Call::Transfer {
            to: address,
            amount,
        })
    } else if selector == trc20::APPROVE_SELECTOR {
        Ok(Trc20Call::Approve {
            spender: address,
            amount,
        })
    } else {
        Err(Error::TransactionBuild(format!(
            "unsupported selector 0x{}: only transfer and approve can be rebuilt",
            hex::encode(selector)
        )))
    }
}

/// Turn a 32-byte ABI address word into a T-address.
///
/// Bytes 11..32 are the 21-byte TRON address (`0x41` + 20), which is how
/// [`trc20::balance_of_args`] laid it out on the way in. Parsing the 42-char
/// hex form re-runs the crate's own address validation rather than trusting
/// the word.
fn evm_word_to_t_address(word: &[u8]) -> Result<String> {
    if word.len() != 32 {
        return Err(Error::TransactionBuild(format!(
            "address word must be 32 bytes, got {}",
            word.len()
        )));
    }
    if word[..11].iter().any(|b| *b != 0) {
        return Err(Error::TransactionBuild(
            "address word has non-zero padding; refusing to reinterpret it".into(),
        ));
    }
    let hex_form = hex::encode(&word[11..32]);
    let parsed: crate::address::Address = hex_form.parse()?;
    Ok(parsed.to_base58())
}

/// Re-send an existing transaction's intent with a higher fee limit.
///
/// TRON has no replace-by-fee: the original keeps its own fate, and this
/// broadcasts a **new** transaction with a **new txid**. If the original later
/// confirms, both may execute — so this is for calls that failed on
/// `OUT_OF_ENERGY`, not for ones merely waiting.
pub async fn submit_send_speedup(
    rpc: &TronGridClient,
    secret: &Zeroizing<[u8; SECRET_KEY_LEN]>,
    txid_hex: &str,
    fee_limit_sun: i64,
) -> Result<Submitted> {
    let opts = SubmitOptions {
        fee_limit_sun: Some(fee_limit_sun),
        ..SubmitOptions::default()
    };
    match rpc.get_transaction_by_id(txid_hex).await? {
        OriginalCall::Transfer {
            owner,
            to,
            amount_sun,
        } => submit_trx(rpc, secret, &owner, &to, amount_sun, opts).await,
        OriginalCall::TriggerSmartContract {
            owner,
            contract,
            data_hex,
        } => match decode_trc20_call(&data_hex)? {
            Trc20Call::Transfer { to, amount } => {
                submit_trc20(rpc, secret, &owner, &contract, &to, amount, opts).await
            }
            Trc20Call::Approve { spender, amount } => {
                submit_trc20_approve(rpc, secret, &owner, &contract, &spender, amount, opts).await
            }
        },
    }
}

/// Poll `gettransactioninfobyid` until the transaction lands in a block.
///
/// A timeout is an error, never an `Ok` with an empty receipt: a caller that
/// reads "not yet confirmed" as "confirmed" ships goods unpaid.
pub async fn wait_for_confirm(
    rpc: &TronGridClient,
    txid_hex: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<TransactionInfo> {
    if poll_interval.is_zero() {
        return Err(Error::Config("poll interval must be non-zero".into()));
    }
    let deadline = SystemTime::now() + timeout;
    loop {
        let info = rpc.get_tx_info(txid_hex).await?;
        if info.block_number.is_some() {
            return Ok(info);
        }
        if SystemTime::now() + poll_interval > deadline {
            return Err(Error::Node(format!(
                "{txid_hex} not confirmed within {}s",
                timeout.as_secs()
            )));
        }
        tokio::time::sleep(poll_interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mainnet USDT — used only as a well-formed address; no network access.
    const USDT: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

    fn transfer_calldata(to: &str, amount: u64) -> String {
        let addr: crate::address::Address = to.parse().expect("valid address");
        let mut out = Vec::new();
        out.extend_from_slice(&trc20::TRANSFER_SELECTOR);
        out.extend_from_slice(&trc20::balance_of_args(addr.as_bytes()));
        out.extend_from_slice(&trc20::encode_uint256_arg(U256::from(amount)));
        hex::encode(out)
    }

    #[test]
    fn decodes_its_own_transfer_calldata() {
        let data = transfer_calldata(USDT, 1_000_000);
        assert_eq!(
            decode_trc20_call(&data).expect("decode"),
            Trc20Call::Transfer {
                to: USDT.to_string(),
                amount: U256::from(1_000_000u64),
            }
        );
    }

    #[test]
    fn decodes_approve_calldata() {
        let addr: crate::address::Address = USDT.parse().expect("valid");
        let mut out = Vec::new();
        out.extend_from_slice(&trc20::APPROVE_SELECTOR);
        out.extend_from_slice(&trc20::balance_of_args(addr.as_bytes()));
        out.extend_from_slice(&trc20::encode_uint256_arg(U256::from(7u64)));
        assert_eq!(
            decode_trc20_call(&hex::encode(out)).expect("decode"),
            Trc20Call::Approve {
                spender: USDT.to_string(),
                amount: U256::from(7u64),
            }
        );
    }

    #[test]
    fn refuses_an_unknown_selector() {
        let mut data = transfer_calldata(USDT, 1);
        data.replace_range(0..8, "deadbeef");
        // Rebuilding an unknown call would sign something we cannot describe.
        assert!(matches!(
            decode_trc20_call(&data),
            Err(Error::TransactionBuild(_))
        ));
    }

    #[test]
    fn refuses_truncated_calldata() {
        assert!(decode_trc20_call("a9059cbb").is_err());
    }

    #[test]
    fn refuses_non_hex_calldata() {
        assert!(decode_trc20_call("zzzz").is_err());
    }

    #[test]
    fn refuses_a_dirty_address_word() {
        // Non-zero padding means the word is not an address slot this crate
        // produced; reinterpreting the low 21 bytes would invent a recipient.
        let mut raw = hex::decode(transfer_calldata(USDT, 1)).expect("hex");
        raw[4] = 0xff;
        assert!(matches!(
            decode_trc20_call(&hex::encode(raw)),
            Err(Error::TransactionBuild(_))
        ));
    }

    #[test]
    fn address_word_round_trips() {
        let addr: crate::address::Address = USDT.parse().expect("valid");
        let word = trc20::balance_of_args(addr.as_bytes());
        assert_eq!(evm_word_to_t_address(&word).expect("decode"), USDT);
    }

    #[tokio::test]
    async fn wait_for_confirm_rejects_a_zero_interval_before_any_call() {
        let rpc = TronGridClient::new("http://127.0.0.1:1", None).expect("client");
        let err = wait_for_confirm(&rpc, "deadbeef", Duration::from_secs(1), Duration::ZERO)
            .await
            .expect_err("zero interval is a config error");
        assert!(matches!(err, Error::Config(_)));
    }

    /// A `const` assertion rather than a runtime one: the point is to make a
    /// future edit that widens the window fail to compile, and anychain's own
    /// default (5 minutes) leaves a signed transfer replayable far longer than
    /// a CLI invocation needs.
    const _: () = assert!(DEFAULT_EXPIRATION_MS < 5 * 60 * 1000);
}
