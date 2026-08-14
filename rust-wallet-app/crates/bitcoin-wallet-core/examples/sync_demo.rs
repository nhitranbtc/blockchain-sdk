//! End-to-end demo of `Wallet::sync` + `Wallet::balance` against
//! the public testnet Esplora server.
//!
//! Per L29 (test all paths before declaring ready): this binary
//! exercises the live network path before claiming the impl works.
//!
//! Run: `cargo run --example sync_demo -p bitcoin-wallet-core`
//!
//! Expects: ~0 UTXOs for a freshly-generated mnemonic (no funding
//! history); balance returns `0 sat`; sync completes without error.
//!
//! `tokio` runtime is required because `sync`/`balance` are async.

use bdk_wallet::bitcoin::Network;
use bitcoin_wallet_core::chain::esplora::{EsploraClient, TlsPolicy};
use bitcoin_wallet_core::chain::esplora_url::EsploraUrl;
use bitcoin_wallet_core::keys::Mnemonic;
use bitcoin_wallet_core::wallet::Wallet;

#[tokio::main]
async fn main() {
    // 1. Fresh mnemonic (12 words) — per CONTEXT.md never reuse a
    //    published BIP-39 test vector.
    let mnemonic = Mnemonic::generate(12usize).expect("fresh mnemonic generation");
    let phrase = mnemonic.to_phrase();
    println!("Generated 12-word mnemonic (testnet):");
    println!("  phrase = \"{}\"", phrase.expose());
    println!();

    // 2. Construct wallet on testnet.
    let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).expect("valid input");
    println!(
        "Wallet constructed: network = {:?}, word_count = {}",
        wallet.network(),
        wallet.phrase().expose().split_whitespace().count()
    );
    println!();

    // 3. Build F20 SPKI-pinned Esplora client (SystemRoots default
    //    for v0.1; production code should pass TlsPolicy::Pinned
    //    with a real SPKI pin via EsploraClient::from_config).
    let esplora_url = "https://blockstream.info/testnet/api";
    let client = EsploraClient::new(
        EsploraUrl::new(esplora_url).expect("esplora url"),
        TlsPolicy::SystemRoots,
    )
    .expect("esplora client build");
    println!("Esplora client built: {esplora_url}");
    println!();

    // 4. F12 chain scan. For a fresh testnet mnemonic, expects
    //    ~0 UTXOs across the first SCAN_GAP_LIMIT=5 external +
    //    internal addresses. Sync completes OK either way.
    println!("Calling wallet.sync(&client).await ...");
    wallet
        .sync(&client)
        .await
        .expect("full sync against live testnet should succeed");
    println!("Sync complete.");
    println!();

    // 5. F13 confirmed-only balance. Should be 0 sat for a fresh
    //    wallet.
    let balance = wallet
        .balance(&client)
        .await
        .expect("balance fetch should succeed");
    println!("Wallet balance: {balance} sat (fresh testnet wallet = 0)");
}
