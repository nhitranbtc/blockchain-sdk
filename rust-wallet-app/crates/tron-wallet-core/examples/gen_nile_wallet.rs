//! Generate a fresh TRON wallet for Nile testnet use.
//!
//! Run with:
//!   cargo run --example gen_nile_wallet
//!
//! Prints: BIP-39 phrase, secp256k1 secret scalar (hex), TRON address.
//! The phrase + scalar ARE the wallet — anyone holding them can spend it.
//! Treat the output like a password. Fund via the Nile faucet:
//!   https://nileex.io/join/getJoinPage
//! or https://www.trongrid.io/faucet (Nile endpoint).

use tron_wallet_core::address::Address;
use tron_wallet_core::keys::{
    derive_keypair, Language, Mnemonic, MnemonicType, DEFAULT_DERIVATION_PATH,
};

fn main() {
    let mnemonic = Mnemonic::generate(MnemonicType::Words12, Language::English);
    let path: tron_wallet_core::keys::DerivationPath = DEFAULT_DERIVATION_PATH
        .parse()
        .expect("default path parses");
    let keypair = derive_keypair(&mnemonic, "", &path).expect("derivation must succeed");

    println!("=== TRON Nile Wallet (do NOT share) ===");
    println!();
    println!("Mnemonic (12 words):");
    println!("  {}", mnemonic.phrase());
    println!();
    println!("Derivation path:     {}", DEFAULT_DERIVATION_PATH);
    println!(
        "Secret (hex, 32 B):  {}",
        hex::encode(**keypair.secret_bytes())
    );
    let address = Address::from_public_key(keypair.public_key()).expect("address derivation");

    println!("Public key:          {}", keypair.public_key());
    println!();
    println!("TRON address (T):    {}", address.to_base58());
    println!("Address hex (41…):   {}", address.to_hex());
    println!();
    println!("Fund the T-address from the Nile faucet, then keep the mnemonic offline.");
}
