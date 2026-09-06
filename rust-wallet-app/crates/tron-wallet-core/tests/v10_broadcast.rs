//! Plan Phase 3 Verification, line 857: live USDT-TRC20 broadcast against Nile.
//!
//! Gate:
//! - `RUN_TRON_NILE=1` — required (operator opt-in, matches rest of suite).
//!
//! Sender + recipient + their mnemonics are read from the bundled Nile
//! fixture at `tokens/nile.json` (`test.sender-tr20.mnemonic` and
//! `test.recipient-tr20`). Both wallets were generated 2026-09-06 via
//! `cargo run --example gen_nile_wallet` (random 12-word BIP-39).
//!
//! **Funding requirement:** the operator MUST fund both addresses at the
//! Nile faucet before running:
//!   - sender: TRX (gas) + USDT-TRC20 (transfer amount).
//!   - recipient: receives 1 USDT-TRC20.
//!
//! Run:
//! ```bash
//! RUN_TRON_NILE=1 \
//!   cargo test -p tron-wallet-core --test v10_broadcast \
//!     -- --include-ignored --nocapture
//! ```
//!
//! Pass criterion: `receipt.is_success() == true` AND `receipt.txid` non-empty.
//! A non-SUCCESS code with a `message` (e.g. `TRANSACTION_EXPIRED`,
//! `BALANCE_INSUFFICIENT`) fails loudly — operator must investigate.
//!
//! RED-honest: `#[ignore]` (excluded from default `cargo test`); panics with
//! actionable message if `RUN_TRON_NILE` absent. Follows plan
//! [Conventions → Gated live tests](../superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md#gated-live-tests-loud-red-never-silent-skip).

use std::time::{SystemTime, UNIX_EPOCH};

use tron_wallet_core::config::Network;
use tron_wallet_core::keys::{derive_keypair, DerivationPath, Language, Mnemonic};
use tron_wallet_core::tx::builder;
use tron_wallet_core::tx::sign::sign_tx;
use tron_wallet_core::TronConfig;

/// SLIP-10 path used by every existing keypair derivation in the suite.
/// Must stay in sync with `tests/address.rs::TRON_PATH` and the default in
/// `examples/gen_nile_wallet.rs`.
const SENDER_PATH: &str = "m/44'/195'/0'/0/0";

/// 1 USDT-TRC20 = 1_000_000 in the 6-decimals USDT unit. Plan §V5 row 2.
const ONE_USDT: u64 = 1_000_000;

/// On-disk shape for the `test.sender-tr20` / `test.recipient-tr20` blocks
/// in `tokens/nile.json`. Mirrors the public T-address + mnemonic of the
/// testnet wallets committed 2026-09-06.
#[derive(serde::Deserialize)]
struct TestWallet {
    mnemonic: String,
    address: String,
}

/// On-disk shape of `tokens/nile.json` — only the fields this test reads.
#[derive(serde::Deserialize)]
struct NileFixture {
    test: NileFixtureTest,
}

#[derive(serde::Deserialize)]
struct NileFixtureTest {
    #[serde(rename = "sender-tr20")]
    sender_tr20: TestWallet,
    #[serde(rename = "recipient-tr20")]
    recipient_tr20: TestWallet,
}

#[tokio::test]
#[ignore = "gated live test — runs only with RUN_TRON_NILE=1; panics if unset; broadcast fails loudly if wallet unfunded (see plan Conventions)"]
async fn live_broadcast_usdt_trc20_to_recipient_succeeds_on_nile() {
    // --- Gating: RUN_TRON_NILE must be set ---
    if std::env::var_os("RUN_TRON_NILE").is_none() {
        panic!(
            "v10_broadcast RED-gated: set RUN_TRON_NILE=1 to run this live broadcast. \
             Sender + recipient + mnemonics are bundled in tokens/nile.json \
             (test.sender-tr20, test.recipient-tr20). Fund both addresses at \
             https://nileex.io/join/getJoinPage before running."
        );
    }

    // --- Load sender + recipient from the bundled Nile fixture ---
    let nile_json = include_str!("../tokens/nile.json");
    let fixture: NileFixture =
        serde_json::from_str(nile_json).expect("tokens/nile.json must parse as NileFixture");
    let sender_wallet = fixture.test.sender_tr20;
    let recipient_wallet = fixture.test.recipient_tr20;
    let owner_address = sender_wallet.address.as_str();
    let recipient = recipient_wallet.address.as_str();
    let mnemonic_phrase = sender_wallet.mnemonic.as_str();
    eprintln!("sender (owner) T-address: {owner_address}");
    eprintln!("recipient T-address:     {recipient}");

    // --- Derive sender keypair from the bundled mnemonic ---
    let mnemonic = Mnemonic::from_phrase(mnemonic_phrase, Language::English)
        .expect("bundled sender mnemonic must be a valid BIP-39 phrase");
    let sender_path: DerivationPath = SENDER_PATH
        .parse()
        .expect("SENDER_PATH must parse as a DerivationPath");
    let keypair = derive_keypair(&mnemonic, "", &sender_path).expect("derive_keypair must succeed");

    // --- Look up the canonical Nile USDT-TRC20 contract from the bundle ---
    let usdt = tron_wallet_core::tokens::by_symbol(Network::Nile, "USDT")
        .expect("Nile USDT must be in the bundled token registry");
    let usdt_address = usdt.address.as_str();
    eprintln!("Nile USDT contract:      {usdt_address}");

    // --- Fetch a fresh ref block from the live network ---
    let cfg = TronConfig::for_network(Network::Nile);
    let rpc = tron_wallet_core::TronGridClient::new(&cfg.rpc_url, cfg.spki_pin)
        .expect("TronGridClient must build against Nile config");
    let head = rpc
        .get_now_block()
        .await
        .expect("get_now_block must succeed");
    eprintln!(
        "Nile head block: #{} id={}",
        head.block_number, head.block_id
    );

    // --- Build + populate the TRC-20 transfer parameters ---
    let amount = ethereum_types::U256::from(ONE_USDT);
    let mut params = builder::trc20_transfer(owner_address, usdt_address, recipient, amount)
        .expect("trc20_transfer builder must succeed");
    builder::set_ref_block(&mut params, head.block_number as i64, &head.block_id)
        .expect("set_ref_block must accept head");
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after epoch")
        .as_millis() as i64;
    builder::set_timestamp(&mut params, ts_ms);

    // --- Sign the transfer with the sender's secret ---
    let signed = sign_tx(keypair.secret_bytes(), &params).expect("sign_tx must succeed");
    eprintln!("local txid: {}", hex::encode(signed.txid));
    eprintln!(
        "raw_data_hex ({} bytes): {}",
        signed.raw_data_hex.len() / 2,
        signed.raw_data_hex
    );
    eprintln!(
        "tail bytes (last 8): {}",
        &signed.raw_data_hex[signed.raw_data_hex.len().saturating_sub(16)..]
    );
    eprintln!(
        "signature_hex ({} bytes): {}",
        signed.signature_hex.len() / 2,
        signed.signature_hex
    );

    // --- Broadcast against the live Nile node ---
    let receipt = rpc
        .broadcast(&signed.signed_envelope_hex)
        .await
        .expect("broadcast must reach the Nile node without transport failure");

    // --- Assertions: SUCCESS code + non-empty txid ---
    assert!(
        receipt.is_success(),
        "Nile broadcast rejected: code={:?} message={:?} error={:?}",
        receipt.code,
        receipt.message,
        receipt.error
    );
    let txid_hex = receipt
        .txid
        .as_deref()
        .expect("successful broadcast must include txid");
    assert!(
        !txid_hex.is_empty(),
        "successful broadcast returned empty txid"
    );
    eprintln!("Nile txid: {txid_hex}");
}
