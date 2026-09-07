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

use std::num::{NonZeroU32, NonZeroU64};
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
///
/// Both fields are non-zero by construction. A zero fee limit expressed as
/// `Some(0)` and "no override" (`None`) are different instructions that the
/// builder cannot distinguish once they reach it, and a zero expiration window
/// signs a transaction that is already expired; the types make both
/// unrepresentable rather than validated at every call site.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubmitOptions {
    /// Energy ceiling in SUN. `None` keeps the builder's default: 0 for native
    /// TRX (bandwidth only), [`builder::DEFAULT_TRC20_FEE_LIMIT_SUN`] for
    /// contract calls.
    pub fee_limit_sun: Option<NonZeroU64>,
    /// Validity window in ms after the timestamp. `None` ⇒
    /// [`DEFAULT_EXPIRATION_MS`].
    pub expiration_ms: Option<NonZeroU32>,
}

impl SubmitOptions {
    /// Both knobs at once, for callers that set them together.
    pub fn new(fee_limit_sun: Option<NonZeroU64>, expiration_ms: Option<NonZeroU32>) -> Self {
        Self {
            fee_limit_sun,
            expiration_ms,
        }
    }
}

/// A signed transaction plus whatever the node said about it.
#[derive(Debug, Clone)]
pub struct Submitted {
    /// The signed envelope. `signed.txid_hex()` is the id to poll.
    pub signed: SignedTransaction,
    /// The node's answer. Prefer [`Submitted::is_success`].
    pub receipt: BroadcastReceipt,
}

impl Submitted {
    /// Pairs a signed envelope with the node's answer about *that* envelope.
    ///
    /// The debug assertion pins the one invariant the rest of this type rests
    /// on: [`Submitted::txid`] reads the locally-computed txid, so a receipt
    /// belonging to a different transaction would make it report an id the
    /// node never acknowledged. TronGrid omits `txid` on some rejections,
    /// which is why only a present-and-different id trips it.
    pub(crate) fn new(signed: SignedTransaction, receipt: BroadcastReceipt) -> Self {
        debug_assert!(
            receipt
                .txid
                .as_deref()
                .is_none_or(|id| id == signed.txid_hex()),
            "receipt txid {:?} does not match the signed transaction {}",
            receipt.txid,
            signed.txid_hex()
        );
        Self { signed, receipt }
    }

    /// Whether the node accepted the broadcast. A `false` here is a
    /// chain-state answer, not a transport failure.
    pub fn is_success(&self) -> bool {
        self.receipt.is_success()
    }

    /// The locally-computed transaction id — the one to poll.
    pub fn txid(&self) -> String {
        self.signed.txid_hex()
    }

    /// The node's verbatim answer.
    pub fn receipt(&self) -> &BroadcastReceipt {
        &self.receipt
    }

    /// Consume the pair, keeping only the node's answer.
    pub fn into_receipt(self) -> BroadcastReceipt {
        self.receipt
    }
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
    let expiration = opts
        .expiration_ms
        .unwrap_or_else(|| NonZeroU32::new(60_000).expect("const nonzero"));
    builder::set_expiration(params, i64::from(expiration.get()));
    if let Some(limit) = opts.fee_limit_sun {
        // `NonZeroU64` is already bounded below; the cast is the only remaining
        // hazard, and a fee limit past `i64::MAX` SUN is not a real ceiling.
        let limit: i64 = i64::try_from(limit.get()).map_err(|_| {
            Error::TransactionBuild(format!("fee limit {} exceeds i64 SUN", limit.get()))
        })?;
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
    Ok(Submitted::new(signed, receipt))
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
    Ok(Submitted::new(signed, receipt))
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
    Ok(Submitted::new(signed, receipt))
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
///
/// The supplied key must be the original owner's. A speed-up rebuilds the
/// original *intent* — recipient and amount come from the chain, not from the
/// caller — so signing it with a different key would move that key's own funds
/// to a recipient the operator never named in this command.
pub async fn submit_send_speedup(
    rpc: &TronGridClient,
    secret: &Zeroizing<[u8; SECRET_KEY_LEN]>,
    txid_hex: &str,
    fee_limit_sun: NonZeroU64,
) -> Result<Submitted> {
    let opts = SubmitOptions {
        fee_limit_sun: Some(fee_limit_sun),
        ..SubmitOptions::default()
    };
    let original = rpc.get_transaction_by_id(txid_hex).await?;
    let owner = match &original {
        OriginalCall::Transfer { owner, .. } => owner,
        OriginalCall::TriggerSmartContract { owner, .. } => owner,
    };
    verify_owner(secret, owner)?;
    match original {
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

/// Refuse a rebuild whose signing key is not the original owner.
///
/// [`Error::Disambiguation`] rather than `Signing`: nothing is wrong with the
/// key or the envelope — the operator pointed the wrong wallet at a txid.
fn verify_owner(secret: &Zeroizing<[u8; SECRET_KEY_LEN]>, owner: &str) -> Result<()> {
    let keypair = crate::keys::keypair_from_secret_bytes(secret.as_slice())?;
    let derived = crate::address::Address::from_public_key(keypair.public_key())?.to_base58();
    if derived != owner {
        return Err(Error::Disambiguation(format!(
            "the supplied key controls {derived}, but that transaction was sent by {owner}"
        )));
    }
    Ok(())
}

/// Reject a poll interval that would spin the loop below without ever sleeping.
///
/// Single source of truth for the check so the CLI can refuse the flag before
/// opening a client and [`wait_for_confirm`] still cannot be entered with a
/// zero interval by an FFI caller. Always [`Error::Config`] — the CLI maps that
/// to exit 2 (operator input) via `handlers::exit_code`.
pub fn validate_poll_interval(poll_interval: Duration) -> Result<()> {
    if poll_interval.is_zero() {
        return Err(Error::Config("poll interval must be non-zero".into()));
    }
    Ok(())
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
    validate_poll_interval(poll_interval)?;
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

    #[test]
    fn wait_for_confirm_zero_interval_returns_config_error() {
        // The lifted helper is the single source of truth for the rule, so the
        // CLI can refuse the flag without constructing a client.
        assert!(matches!(
            validate_poll_interval(Duration::ZERO),
            Err(Error::Config(_))
        ));
        assert!(validate_poll_interval(Duration::from_secs(1)).is_ok());
    }

    /// A signed envelope with a known txid, for the [`Submitted`] accessors.
    /// Signing is exercised in `tx::sign`; here only the txid matters.
    fn signed_with(txid_byte: u8) -> SignedTransaction {
        SignedTransaction {
            txid: [txid_byte; 32],
            raw_data_hex: "00".into(),
            signature_hex: "00".into(),
            signed_envelope_hex: "00".into(),
        }
    }

    fn receipt(code: &str, txid: Option<String>) -> BroadcastReceipt {
        BroadcastReceipt {
            code: Some(code.into()),
            txid,
            message: None,
            error: None,
        }
    }

    #[test]
    fn submitted_is_success_reflects_receipt() {
        let signed = signed_with(0xab);
        let id = signed.txid_hex();
        assert!(Submitted::new(signed.clone(), receipt("SUCCESS", Some(id))).is_success());
        // A rejection is still a well-formed `Submitted` — the caller decides
        // how loudly to fail.
        let rejected = Submitted::new(signed, receipt("CONTRACT_VALIDATE_ERROR", None));
        assert!(!rejected.is_success());
        assert!(!rejected.into_receipt().is_success());
    }

    #[test]
    fn submitted_txid_matches_signed() {
        let signed = signed_with(0x11);
        let id = signed.txid_hex();
        let submitted = Submitted::new(signed, receipt("SUCCESS", Some(id.clone())));
        // `txid()` reads the locally-computed id, which is what the debug
        // assertion in `new` pins against the receipt.
        assert_eq!(submitted.txid(), id);
        assert_eq!(submitted.receipt().txid.as_deref(), Some(id.as_str()));
    }

    #[test]
    fn submit_options_new_validates_non_zero() {
        // Zero is unrepresentable: `NonZeroU64::new(0)` is the rejection, so a
        // handler mapping a CLI `0` through it cannot reach the builder.
        assert!(NonZeroU64::new(0).is_none());
        assert!(NonZeroU32::new(0).is_none());

        let opts = SubmitOptions::new(NonZeroU64::new(1_000_000), NonZeroU32::new(30_000));
        assert_eq!(opts.fee_limit_sun.map(|n| n.get()), Some(1_000_000));
        assert_eq!(opts.expiration_ms.map(|n| n.get()), Some(30_000));

        let empty = SubmitOptions::new(NonZeroU64::new(0), NonZeroU32::new(0));
        assert!(empty.fee_limit_sun.is_none() && empty.expiration_ms.is_none());
        assert!(SubmitOptions::default().fee_limit_sun.is_none());
    }

    #[tokio::test]
    async fn submit_send_speedup_owner_mismatch_is_disambiguation() {
        // `verify_owner` is the gate: rebuilding an intent recovered from the
        // chain with someone else's key would move *that* key's funds to a
        // recipient this command never named.
        let secret = Zeroizing::new([7u8; SECRET_KEY_LEN]);
        let err = verify_owner(&secret, USDT).expect_err("wrong owner must be refused");
        assert!(matches!(err, Error::Disambiguation(_)));

        // The matching owner passes, so the guard is not simply always-on.
        let keypair = crate::keys::keypair_from_secret_bytes(secret.as_slice()).expect("keypair");
        let derived = crate::address::Address::from_public_key(keypair.public_key())
            .expect("address")
            .to_base58();
        verify_owner(&secret, &derived).expect("the real owner is accepted");
    }

    /// A `const` assertion rather than a runtime one: the point is to make a
    /// future edit that widens the window fail to compile, and anychain's own
    /// default (5 minutes) leaves a signed transfer replayable far longer than
    /// a CLI invocation needs.
    const _: () = assert!(DEFAULT_EXPIRATION_MS < 5 * 60 * 1000);
}
