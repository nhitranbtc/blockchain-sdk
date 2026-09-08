//! Full-flow: create a wallet, encrypt with password, unlock, re-derive
//! the address, prove the round-trip. Then prove a wrong password is
//! rejected.
//!
//! Run with:
//!   cargo run --example fullflow_create_wallet -p tron-wallet-core
//!
//! **Runtime:** ~40 s dominated by Argon2id (m=256 MiB, t=10, p=4).
//! That cost is the point — it is what makes brute-forcing the wallet
//! file expensive. Do not lower it.
//!
//! What this exercises:
//!   1. Generate a fresh 12-word BIP-39 mnemonic.
//!   2. Derive the T-address pre-encryption (proves derivation works).
//!   3. `WalletManager::create(mnemonic, password)` — Argon2id KDF +
//!      AES-256-GCM under `InMemoryStorage`.
//!   4. `WalletManager::unlock(id, password)` — decrypt, verify the
//!      mnemonic round-trips byte-for-byte.
//!   5. Re-derive the T-address from the unlocked mnemonic — must equal
//!      step 2. End-to-end proof that the encrypted blob round-trips.
//!   6. Wrong password → `Error::Encryption` (GCM tag mismatch). Same
//!      variant as a corrupt blob — no password-guessing oracle.

use tron_wallet_core::address::Address;
use tron_wallet_core::keys::{
    derive_keypair, Language, Mnemonic, MnemonicType, DEFAULT_DERIVATION_PATH,
};
use tron_wallet_core::platform::test::InMemoryStorage;
use tron_wallet_core::wallet::WalletManager;

fn main() {
    // --- Flags: --quiet suppresses the mnemonic on stderr -----------
    let quiet = std::env::args().any(|a| a == "--quiet");

    // --- 1. Generate a fresh 12-word mnemonic ------------------------
    let mnemonic = Mnemonic::generate(MnemonicType::Words12, Language::English);
    let path: tron_wallet_core::keys::DerivationPath = DEFAULT_DERIVATION_PATH
        .parse()
        .expect("default path parses");

    // --- 2. Derive T-address pre-encryption --------------------------
    let keypair_a = derive_keypair(&mnemonic, "", &path).expect("derivation A");
    let address_a = Address::from_public_key(keypair_a.public_key()).expect("address A");
    let t_address = address_a.to_base58();

    // --- 3. Create encrypted blob with password ----------------------
    // The "password" here is the file-encryption passphrase, NOT the
    // BIP-39 25th-word passphrase. Empty BIP-39 passphrase (`""`) is
    // the common case; the file-encryption password is whatever the
    // operator types — here we hardcode a demo value.
    const WALLET_PASSWORD: &str = "correct horse battery staple";
    const WRONG_PASSWORD: &str = "Tr0ub4dor&3";

    let storage = InMemoryStorage::new();
    let mgr = WalletManager::new(&storage);
    let id = mgr
        .create(&mnemonic, WALLET_PASSWORD)
        .expect("create encrypted wallet");
    assert_eq!(storage.len(), 1, "one encrypted blob persisted");

    // --- 4. Unlock with correct password -----------------------------
    let unlocked = mgr
        .unlock(id, WALLET_PASSWORD)
        .expect("unlock with right password");
    let round_trip_phrase = unlocked
        .mnemonic()
        .expect("a mnemonic-backed record yields a phrase")
        .phrase();
    assert_eq!(
        round_trip_phrase,
        mnemonic.phrase(),
        "decrypted phrase must equal input"
    );

    // --- 5. Re-derive T-address from unlocked mnemonic ----------------
    let keypair_b = derive_keypair(unlocked.mnemonic().unwrap(), "", &path)
        .expect("derivation B from unlocked mnemonic");
    let address_b = Address::from_public_key(keypair_b.public_key()).expect("address B");
    assert_eq!(
        address_b.to_base58(),
        t_address,
        "address derived from the decrypted mnemonic must match the pre-encryption address"
    );

    // --- 6. Wrong password rejected ----------------------------------
    let err = mgr
        .unlock(id, WRONG_PASSWORD)
        .expect_err("wrong password must fail");
    assert!(
        matches!(err, tron_wallet_core::Error::Encryption(_)),
        "wrong password must surface as Error::Encryption (same variant as a corrupt blob)"
    );

    // --- Summary ------------------------------------------------------
    if quiet {
        println!("T-address:  {}", t_address);
        println!("Wallet id:  {}", id);
        return;
    }

    eprintln!("=== Fullflow: password-gated wallet create + unlock + re-derive ===");
    eprintln!();
    eprintln!("Mnemonic (12 words):");
    eprintln!("  {}", mnemonic.phrase());
    eprintln!();
    eprintln!("Derivation path:    {}", DEFAULT_DERIVATION_PATH);
    eprintln!("T-address (fresh):  {}", t_address);
    eprintln!("Wallet id:          {}", id);
    eprintln!();
    eprintln!("Encrypted blob:     Argon2id(m=256MiB, t=10, p=4) + AES-256-GCM");
    eprintln!("Unlock (correct):   OK — phrase round-trips byte-for-byte");
    eprintln!("Re-derived address: == pre-encryption address (round-trip proof)");
    eprintln!("Unlock (wrong):     rejected as Error::Encryption (no password oracle)");
}
