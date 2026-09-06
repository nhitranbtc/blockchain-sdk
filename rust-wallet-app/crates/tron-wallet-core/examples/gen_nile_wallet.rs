//! Generate a fresh TRON wallet for Nile testnet use.
//!
//! Run with:
//!   cargo run --example gen_nile_wallet
//!   cargo run --example gen_nile_wallet -- --out ./nile.wallet.txt
//!   cargo run --example gen_nile_wallet -- --quiet            # T-address only
//!
//! Default behavior: mnemonic + derivation path + T-address go to stderr
//! (NOT stdout) so a `cargo run --example gen_nile_wallet > wallet.bak`
//! redirection does not silently write the seed to a redirectable fd.
//! Stdout is reserved for the operator-actionable summary.
//!
//! `--out <path>` writes the full wallet record (mnemonic + path + T-address)
//! to a `0o600`-permed file. Pass the secret material to a file under your
//! control, not into a shell pipe.
//!
//! `--quiet` prints only the T-address + faucet URL (use this when the
//! operator does not need the mnemonic re-derived, e.g. when only funding).
//!
//! Security note: the phrase IS the wallet. Anyone holding the file can
//! spend the wallet. Treat like a password.
//!
//! Fund the T-address via the Nile faucet:
//!   https://nileex.io/join/getJoinPage
//!   https://www.trongrid.io/faucet (Nile endpoint)

use std::io::Write as _;

use tron_wallet_core::address::Address;
use tron_wallet_core::keys::{
    derive_keypair, Language, Mnemonic, MnemonicType, DEFAULT_DERIVATION_PATH,
};

fn main() {
    // --- Parse CLI flags (intentionally minimal: --out, --quiet) ---
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out_path: Option<String> = None;
    let mut quiet = false;
    for arg in &args {
        match arg.as_str() {
            "--quiet" => quiet = true,
            other if other.starts_with("--out=") => {
                out_path = Some(other.trim_start_matches("--out=").to_owned());
            }
            _ => {}
        }
    }
    if out_path.is_none() {
        if let Some(pos) = args.iter().position(|a| a == "--out") {
            if let Some(p) = args.get(pos + 1) {
                out_path = Some(p.clone());
            }
        }
    }

    // --- Generate a fresh 12-word BIP-39 mnemonic ---
    let mnemonic = Mnemonic::generate(MnemonicType::Words12, Language::English);
    let path: tron_wallet_core::keys::DerivationPath = DEFAULT_DERIVATION_PATH
        .parse()
        .expect("default path parses");
    let keypair = derive_keypair(&mnemonic, "", &path).expect("derivation must succeed");
    let address = Address::from_public_key(keypair.public_key()).expect("address derivation");
    let t_address = address.to_base58();

    // --- Write mode: --out <path> ---
    // 0o600 perms; fails if the file exists (avoids clobber).
    if let Some(path) = out_path.as_ref() {
        let body = format!(
            "mnemonic: {}\npath:     {}\naddress:  {}\n",
            mnemonic.phrase(),
            DEFAULT_DERIVATION_PATH,
            t_address,
        );
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("open --out file (with create_new; pre-existing file is an error)");
        // 0o600 perms on unix; Windows ACLs differ but the wallet is
        // operator-side on macOS/Linux (the active TRON dev targets).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .expect("set 0o600 perms");
        }
        f.write_all(body.as_bytes()).expect("write --out file");
        f.sync_all().ok();
        eprintln!(
            "wrote {} (mode 0600, create_new refused clobber) — store offline, do NOT commit",
            path
        );
    }

    if quiet {
        // Quiet mode: T-address + faucet hint only. Mnemonic NOT printed.
        // Operator should already know the mnemonic from --out, or this run
        // is a no-secret "give me an address to fund" stub.
        println!("T-address:  {}", t_address);
        println!("Fund via:    https://nileex.io/join/getJoinPage");
        return;
    }

    // Default mode: mnemonic + path + T-address go to stderr (NOT stdout)
    // so default `cargo run --example gen_nile_wallet > wallet.bak` does not
    // silently write the seed to a redirectable file descriptor.
    eprintln!("=== TRON Nile Wallet (do NOT share) ===");
    eprintln!();
    eprintln!("Mnemonic (12 words):");
    eprintln!("  {}", mnemonic.phrase());
    eprintln!();
    eprintln!("Derivation path:     {}", DEFAULT_DERIVATION_PATH);
    eprintln!("TRON address (T):    {}", t_address);
    eprintln!("Address hex (41…):   {}", address.to_hex());
    eprintln!();
    eprintln!("Fund the T-address from the Nile faucet, then keep the mnemonic offline.");
    eprintln!("Re-derive the secp256k1 secret from the mnemonic + path — no separate scalar dump.");
}
