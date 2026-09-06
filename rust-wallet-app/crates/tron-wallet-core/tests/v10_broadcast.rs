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
//! Per full `--include-ignored` run the suite spends 2 USDT-TRC20 (USDT happy-path +
//! rebroadcast idempotency), about 130k Energy (≈0.06 TRX), and 1 TRX of bandwidth
//! (native-TRX happy-path). The Nile faucet has a 24h cooldown — plan test
//! sequencing accordingly.
//!
//! Run:
//! ```bash
//! RUN_TRON_NILE=1 \
//!   cargo test -p tron-wallet-core --test v10_broadcast \
//!     -- --include-ignored --test-threads=1
//! ```
//!
//! Pass criterion: `receipt.is_success() == true` AND `receipt.txid` non-empty.
//! A non-SUCCESS code with a `message` (e.g. `TRANSACTION_EXPIRED`,
//! `BALANCE_INSUFFICIENT`) fails loudly — operator must investigate.
//!
//! RED-honest: `#[ignore]` excludes this from default `cargo test`. Run with
//! `cargo test -p tron-wallet-core --test v10_broadcast -- --include-ignored`
//! after funding the wallet. RPC/network failures now surface directly via
//! `.expect(...)` and `assert!` — no separate loud-RED panic gate (removed
//! 2026-09-06 per operator direction). Plan Conventions §Gated live tests
//! remains in force for `tests/v5_resource.rs`, `tests/v7_spki_pin.rs`,
//! `tests/v9_token_registry.rs`; this file is the documented exception.

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
#[ignore = "gated live test — runs only with RUN_TRON_NILE=1; loud-RED panic removed 2026-09-06, RPC failure now surfaces directly (see plan Conventions)"]
async fn live_broadcast_usdt_trc20_to_recipient_succeeds_on_nile() {
    // RUN_TRON_NILE env-var gate removed 2026-09-06 (was the panic above).
    // Test still requires RUN_TRON_NILE=1 to succeed against live Nile RPC;
    // removal only drops the loud-RED panic, not the network requirement.

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
    let rpc = tron_wallet_core::TronGridClient::new(&cfg.rpc_url, None)
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
    let local_txid_hex = hex::encode(signed.txid);
    eprintln!("local txid: {local_txid_hex}");

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
    // Pin the wire-form txid contract: locally-computed
    // (sha256(raw_bytes) per tx::sign::txid — live Nile verification
    // 2026-09-06) must match the network-reported txid. Regression target
    // for the original single-vs-double SHA-256 confusion (Q2 plan
    // hypothesis was wrong; the actual algorithm is single SHA-256,
    // matching upstream).
    assert_eq!(
        local_txid_hex.to_ascii_lowercase(),
        txid_hex.to_ascii_lowercase(),
        "locally-computed txid {local_txid_hex} != network-reported txid {txid_hex} \
         — local txid computation regressed? See tx::sign::txid = sha256(raw_bytes) per live Nile verification 2026-09-06."
    );
    eprintln!("Nile txid: {txid_hex}");
}

/// 1 TRX = 1_000_000 SUN. Used by the native-TRX broadcast leg below.
const ONE_TRX_SUN: u64 = 1_000_000;

#[tokio::test]
#[ignore = "gated live test — runs only with RUN_TRON_NILE=1; loud-RED panic removed 2026-09-06, RPC failure now surfaces directly (see plan Conventions)"]
async fn live_broadcast_trx_native_transfer_succeeds_on_nile() {
    // RUN_TRON_NILE env-var gate removed 2026-09-06 (was the panic above).
    // Test still requires RUN_TRON_NILE=1 to succeed against live Nile RPC;
    // removal only drops the loud-RED panic, not the network requirement.

    // --- Load sender + recipient from the bundled Nile fixture ---
    let nile_json = include_str!("../tokens/nile.json");
    let fixture: NileFixture =
        serde_json::from_str(nile_json).expect("tokens/nile.json must parse as NileFixture");
    let sender_wallet = fixture.test.sender_tr20;
    let recipient_wallet = fixture.test.recipient_tr20;
    let owner_address = sender_wallet.address.as_str();
    let recipient = recipient_wallet.address.as_str();
    let mnemonic_phrase = sender_wallet.mnemonic.as_str();
    eprintln!("[trx] sender (owner) T-address: {owner_address}");
    eprintln!("[trx] recipient T-address:     {recipient}");

    // --- Derive sender keypair from the bundled mnemonic ---
    let mnemonic = Mnemonic::from_phrase(mnemonic_phrase, Language::English)
        .expect("bundled sender mnemonic must be a valid BIP-39 phrase");
    let sender_path: DerivationPath = SENDER_PATH
        .parse()
        .expect("SENDER_PATH must parse as a DerivationPath");
    let keypair = derive_keypair(&mnemonic, "", &sender_path).expect("derive_keypair must succeed");

    // --- Fetch a fresh ref block from the live network ---
    let cfg = TronConfig::for_network(Network::Nile);
    let rpc = tron_wallet_core::TronGridClient::new(&cfg.rpc_url, None)
        .expect("TronGridClient must build against Nile config");
    let head = rpc
        .get_now_block()
        .await
        .expect("get_now_block must succeed");

    // --- Build native TRX transfer (bandwidth-only, fee_limit = 0) ---
    let mut params = builder::trx_transfer(owner_address, recipient, ONE_TRX_SUN)
        .expect("trx_transfer builder must succeed");
    builder::set_ref_block(&mut params, head.block_number as i64, &head.block_id)
        .expect("set_ref_block must accept head");
    builder::set_fee_limit(&mut params, 0);
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after epoch")
        .as_millis() as i64;
    builder::set_timestamp(&mut params, ts_ms);

    // --- Sign the transfer with the sender's secret ---
    let signed = sign_tx(keypair.secret_bytes(), &params).expect("sign_tx must succeed");
    let local_txid_hex = hex::encode(signed.txid);
    eprintln!("[trx] local txid: {local_txid_hex}");

    // --- Broadcast against the live Nile node ---
    let receipt = rpc
        .broadcast(&signed.signed_envelope_hex)
        .await
        .expect("broadcast must reach the Nile node without transport failure");

    // --- Assertions: SUCCESS code + non-empty txid ---
    assert!(
        receipt.is_success(),
        "Nile TRX broadcast rejected: code={:?} message={:?} error={:?}",
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
        "successful TRX broadcast returned empty txid"
    );
    // Pin the wire-form txid contract: locally-computed
    // (sha256(raw_bytes) per tx::sign::txid — live Nile verification
    // 2026-09-06) must match the network-reported txid. Regression target
    // for the original single-vs-double SHA-256 confusion (Q2 plan
    // hypothesis was wrong; the actual algorithm is single SHA-256,
    // matching upstream).
    assert_eq!(
        local_txid_hex.to_ascii_lowercase(),
        txid_hex.to_ascii_lowercase(),
        "locally-computed txid {local_txid_hex} != network-reported txid {txid_hex} \
         — local txid computation regressed? See tx::sign::txid = sha256(raw_bytes) per live Nile verification 2026-09-06."
    );
    eprintln!("[trx] Nile txid: {txid_hex}");
}

/// Plan Phase 7 Task 6.8 (V7a): re-POST the same signed envelope and assert
/// the network treats it as idempotent — either same txid (memoized) OR
/// explicit non-SUCCESS with a `code`/`message` explaining the prior
/// broadcast. What MUST NOT happen: SUCCESS with a different txid (would
/// imply double-charge against the sender).
#[tokio::test]
#[ignore = "gated live test — runs only with RUN_TRON_NILE=1; loud-RED panic removed 2026-09-06, RPC failure now surfaces directly (see plan Conventions)"]
async fn live_broadcast_rebroadcast_idempotency_on_nile() {
    // RUN_TRON_NILE env-var gate removed 2026-09-06 (was the panic above).
    // Test still requires RUN_TRON_NILE=1 to succeed against live Nile RPC;
    // removal only drops the loud-RED panic, not the network requirement.

    // --- Load sender + recipient + Nile USDT contract from the bundled fixture ---
    let nile_json = include_str!("../tokens/nile.json");
    let fixture: NileFixture =
        serde_json::from_str(nile_json).expect("tokens/nile.json must parse as NileFixture");
    let sender_wallet = fixture.test.sender_tr20;
    let recipient_wallet = fixture.test.recipient_tr20;
    let owner_address = sender_wallet.address.as_str();
    let recipient = recipient_wallet.address.as_str();
    let mnemonic_phrase = sender_wallet.mnemonic.as_str();
    eprintln!("[re] sender (owner) T-address: {owner_address}");
    eprintln!("[re] recipient T-address:     {recipient}");

    let mnemonic = Mnemonic::from_phrase(mnemonic_phrase, Language::English)
        .expect("bundled sender mnemonic must be a valid BIP-39 phrase");
    let sender_path: DerivationPath = SENDER_PATH
        .parse()
        .expect("SENDER_PATH must parse as a DerivationPath");
    let keypair = derive_keypair(&mnemonic, "", &sender_path).expect("derive_keypair must succeed");

    let usdt = tron_wallet_core::tokens::by_symbol(Network::Nile, "USDT")
        .expect("Nile USDT must be in the bundled token registry");
    let usdt_address = usdt.address.as_str();

    let cfg = TronConfig::for_network(Network::Nile);
    let rpc = tron_wallet_core::TronGridClient::new(&cfg.rpc_url, None)
        .expect("TronGridClient must build against Nile config");
    let head = rpc
        .get_now_block()
        .await
        .expect("get_now_block must succeed");

    // --- Build + sign one USDT transfer envelope ---
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
    let signed = sign_tx(keypair.secret_bytes(), &params).expect("sign_tx must succeed");
    let local_txid_hex = hex::encode(signed.txid);
    eprintln!("[re] local txid: {local_txid_hex}");

    // --- First broadcast (must succeed) ---
    let receipt1 = rpc
        .broadcast(&signed.signed_envelope_hex)
        .await
        .expect("first broadcast must reach the Nile node without transport failure");
    assert!(
        receipt1.is_success(),
        "first broadcast rejected: code={:?} message={:?} error={:?}",
        receipt1.code,
        receipt1.message,
        receipt1.error
    );
    let txid1 = receipt1
        .txid
        .as_deref()
        .expect("first successful broadcast must include txid")
        .to_owned();
    eprintln!("[re] first  Nile txid: {txid1}");

    // Pin the wire-form txid contract: locally-computed
    // (sha256(raw_bytes) per tx::sign::txid — live Nile verification
    // 2026-09-06) must match the first broadcast's network-reported txid.
    // Regression target for the original single-vs-double SHA-256
    // confusion (Q2 plan hypothesis was wrong; the actual algorithm is
    // single SHA-256, matching upstream).
    assert_eq!(
        local_txid_hex.to_ascii_lowercase(),
        txid1.to_ascii_lowercase(),
        "locally-computed txid {local_txid_hex} != first-broadcast network txid {txid1} \
         — local txid computation regressed? See tx::sign::txid = sha256(raw_bytes) per live Nile verification 2026-09-06."
    );

    // --- Re-POST the SAME signed envelope ---
    let receipt2 = rpc
        .broadcast(&signed.signed_envelope_hex)
        .await
        .expect("rebroadcast must reach the Nile node without transport failure");
    eprintln!(
        "[re] second code={:?} message={:?} txid={:?}",
        receipt2.code, receipt2.message, receipt2.txid
    );

    // Idempotency contract (plan Phase 7 Task 6.8 V7a empirical finding,
    // live verified 2026-09-06 on Nile): pure envelope rebroadcast returns
    // `code = "DUP_TRANSACTION_ERROR"`, `message = "Dup transaction."` with
    // the SAME txid echoed back. Sender is NOT double-charged.
    //
    // Pin both branches:
    //   - SUCCESS path: txid MUST equal the first broadcast's txid
    //     (no double-charge). Any other txid = bug.
    //   - non-SUCCESS path: code OR message MUST mention DUP_TRANSACTION.
    //     A generic "FAILED" or empty rejection is a regression (the operator
    //     should investigate).
    let same_txid = receipt2.txid.as_deref() == Some(txid1.as_str());
    if receipt2.is_success() {
        assert!(
            same_txid,
            "rebroadcast returned SUCCESS with a different txid (potential double-charge): \
             first={txid1} second={:?}",
            receipt2.txid
        );
    } else {
        let code = receipt2.code.as_deref().unwrap_or_default();
        let msg = receipt2.message.as_deref().unwrap_or_default();
        assert!(
            code.contains("DUP_TRANSACTION_ERROR")
                || msg.to_ascii_uppercase().contains("DUP TRANSACTION"),
            "rebroadcast failure does not look like DUP_TRANSACTION_ERROR \
             (operator should re-investigate canonical node contract): code={code:?} message={msg:?}",
        );
    }
}
