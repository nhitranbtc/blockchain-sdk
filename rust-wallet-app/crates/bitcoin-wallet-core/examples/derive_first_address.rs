// One-shot address derivation for manual testnet verification.
// Uses the existing offline path (`first_external_address_offline`)
// which does NOT require Esplora sync.
//
// Usage:
//   cargo run -p bitcoin-wallet-core --example derive_first_address -- "word1 word3 ... word12"
use bdk_wallet::bitcoin::Network;
use bitcoin_wallet_core::keys::Mnemonic;
use bitcoin_wallet_core::wallet::Wallet;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: derive_first_address <12|24-word mnemonic>");
        std::process::exit(2);
    }
    let phrase = args.join(" ");
    let word_count = phrase.split_whitespace().count();
    if word_count != 12 && word_count != 24 {
        eprintln!("expected 12 or 24 words, got {word_count}");
        std::process::exit(2);
    }
    // Mnemonic::from_phrase derives the word count from the input.
    let mnemonic = match Mnemonic::from_phrase(&phrase) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("invalid mnemonic: {e}");
            std::process::exit(2);
        }
    };
    let wallet =
        Wallet::from_mnemonic(&mnemonic, Network::Testnet, None).expect("wallet construction");
    let addr = wallet
        .first_external_address_offline()
        .expect("address derivation");
    println!("{addr}");
}
