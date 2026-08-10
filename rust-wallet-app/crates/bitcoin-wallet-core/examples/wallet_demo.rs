use bdk_wallet::bitcoin::Network;
use bitcoin_wallet_core::keys::Mnemonic;
use bitcoin_wallet_core::wallet::Wallet;

fn main() {
    // Generate fresh mnemonic (not hardcoded — per CONTEXT.md hard rule #5)
    let mnemonic = Mnemonic::generate(12usize).expect("fresh mnemonic generation");
    let phrase = mnemonic.to_phrase();
    println!("Generated 12-word mnemonic:");
    println!("  phrase = \"{}\"", phrase.expose());
    println!("  word count = {}", mnemonic.word_count());

    // Construct wallet
    let wallet = Wallet::from_mnemonic(&mnemonic, Network::Testnet).expect("wallet construction");
    println!("\nWallet constructed:");
    println!("  network = {:?}", wallet.network());
    println!("  phrase  = \"{}\"", wallet.phrase().expose());
    println!(
        "  word count = {}",
        wallet.phrase().expose().split_whitespace().count()
    );
}
