//! End-to-end Solana wallet creation example.
//!
//! Generates a random 12-word BIP-39 mnemonic, derives a Phantom-compatible wallet
//! at `m/44'/501'/0'/0'`, encrypts it via `WalletManager`, prints the base58
//! pubkey, and (gated by env var) airdrops SOL on devnet.
//!
//! Audit controls baked in per
//! [docs/audit/2026-09-12-sol-wallet-core-phase9-security-audit.md](../../../audit/2026-09-12-sol-wallet-core-phase9-security-audit.md)
//! (issue [#563](https://github.com/nhitranbtc/blockchain-sdk/issues/563)).
//! Controls: P9-1 (env-var password), P9-2 (mnemonic to 0o600 file), P9-3 (data_dir
//! validation), P9-4 (non-devnet RPC warning), P9-6 (explicit tokio shutdown),
//! P9-7 (0.5 SOL airdrop cap), P9-8 (CI early-return).
//!
//! ## Run
//!
//! ```text
//! cargo run -p sol-wallet-core --example create_wallet_devnet -- /tmp/sol-wallet-example
//!
//! # With devnet airdrop (gated):
//! RUN_SOL_DEVNET_AIRDROP=1 \
//!     cargo run -p sol-wallet-core --example create_wallet_devnet -- /tmp/sol-wallet-example
//!
//! # Custom RPC URL:
//! SOL_RPC_URL=https://api.devnet.solana.com cargo run -p sol-wallet-core --example create_wallet_devnet -- /tmp/sol-wallet-example
//!
//! # Custom password (overrides random default):
//! EXAMPLE_WALLET_PASSWORD='my-secret' cargo run -p sol-wallet-core --example create_wallet_devnet -- /tmp/sol-wallet-example
//! ```
//!
//! All output to stdout. Mnemonic + warnings to stderr. Encrypted blob + mnemonic
//! file written to `data_dir` with mode 0o600 (Unix).

use sol_wallet_core::{
    chain::{account::request_airdrop, client::RpcClient},
    ffi_mnemonic::{generate_12_word_english, now_unix},
    platform::storage::FileWalletStorage,
    wallet_manager::WalletManager,
    Result,
};
use std::path::{Path, PathBuf};

// -- Audit controls -----------------------------------------------------------

/// P9-8: early-return in CI to avoid orphan encrypted blobs on CI scratch.
fn skip_in_ci() -> bool {
    std::env::var("CI").is_ok()
}

/// P9-3: validate `data_dir` — absolute path, not under a system dir, not a symlink parent.
fn validate_data_dir(raw: &str) -> Option<PathBuf> {
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        eprintln!("ERROR: data_dir must be absolute (got {raw})");
        return None;
    }
    // System dirs we refuse to write wallet data under
    const SYSTEM_DIRS: &[&str] = &[
        "/etc", "/var", "/usr", "/bin", "/sbin", "/sys", "/proc", "/boot", "/lib", "/lib64",
        "/root",
    ];
    for sys in SYSTEM_DIRS {
        if path.starts_with(sys) {
            eprintln!("ERROR: data_dir must not be a system directory (got {raw})");
            return None;
        }
    }
    // Reject if any parent component is a symlink
    let mut cur = path.clone();
    while let Some(parent) = cur.parent() {
        if parent.as_os_str().is_empty() {
            break;
        }
        if let Ok(meta) = std::fs::symlink_metadata(&cur) {
            if meta.file_type().is_symlink() {
                eprintln!(
                    "ERROR: data_dir path contains a symlink at {}",
                    cur.display()
                );
                return None;
            }
        }
        cur = parent.to_path_buf();
    }
    Some(path)
}

/// P9-1: env-var password — no plaintext literal in source.
fn resolve_password() -> Option<zeroize::Zeroizing<String>> {
    match std::env::var("EXAMPLE_WALLET_PASSWORD") {
        Ok(p) if !p.is_empty() => {
            eprintln!(
                "Using EXAMPLE_WALLET_PASSWORD from env (length={}).",
                p.len()
            );
            Some(zeroize::Zeroizing::new(p))
        }
        _ => {
            eprintln!("WARN: EXAMPLE_WALLET_PASSWORD not set; using time-based random default.");
            eprintln!(
                "      Set EXAMPLE_WALLET_PASSWORD before running for repeatable dev cycles."
            );
            let suffix = now_unix();
            Some(zeroize::Zeroizing::new(format!("change-me-{suffix}")))
        }
    }
}

/// P9-2: write mnemonic to `0o600` file in `data_dir`. NO stdout.
fn emit_mnemonic_securely(data_dir: &Path, phrase: &str) -> Result<()> {
    let mnemonic_path = data_dir.join("mnemonic.txt");
    std::fs::write(&mnemonic_path, format!("{phrase}\n")).map_err(|source| {
        sol_wallet_core::Error::FileIo {
            path: mnemonic_path.clone(),
            source,
        }
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&mnemonic_path, std::fs::Permissions::from_mode(0o600)).map_err(
            |source| sol_wallet_core::Error::FileIo {
                path: mnemonic_path.clone(),
                source,
            },
        )?;
    }
    eprintln!("Mnemonic written to: {}", mnemonic_path.display());
    eprintln!("Permissions: 0o600 (owner read/write only).");
    eprintln!("DO NOT share, screenshot, or commit this file. Back it up to encrypted storage.");
    Ok(())
}

/// P9-4: warn if airdrop is requested but URL is non-devnet.
fn warn_if_non_devnet_airdrop_target(url: &str) {
    if !url.contains("api.devnet.solana.com")
        && !url.contains("api.testnet.solana.com")
        && !url.contains("localhost")
        && !url.contains("127.0.0.1")
        && std::env::var("RUN_SOL_DEVNET_AIRDROP").is_ok()
    {
        eprintln!("WARN: SOL_RPC_URL={url} but RUN_SOL_DEVNET_AIRDROP=1 set.");
        eprintln!("      request_airdrop enforces the devnet host allowlist:");
        eprintln!("        api.devnet.solana.com, api.testnet.solana.com, localhost, 127.0.0.1");
        eprintln!("      Airdrop will fail with Error::Transport. Adjust SOL_RPC_URL or unset");
        eprintln!("      RUN_SOL_DEVNET_AIRDROP if you do not want airdrop.");
    }
}

// -- Main --------------------------------------------------------------------

fn main() -> Result<()> {
    // P9-8: CI early-return
    if skip_in_ci() {
        eprintln!("SKIP: CI env detected; example exits without side effects.");
        return Ok(());
    }

    // Args
    let args: Vec<String> = std::env::args().collect();
    let data_dir_raw = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/sol-wallet-example".to_string());
    let Some(data_dir) = validate_data_dir(&data_dir_raw) else {
        std::process::exit(1);
    };
    let rpc_url = std::env::var("SOL_RPC_URL")
        .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());

    warn_if_non_devnet_airdrop_target(&rpc_url);

    eprintln!("data_dir: {}", data_dir.display());
    eprintln!("rpc_url : {}", rpc_url);

    // 1. Generate random 12-word mnemonic (no hardcoded phrase)
    let phrase = generate_12_word_english()?;

    // 2. Init storage + manager (FileWalletStorage sets 0o600 on the root on Unix)
    let storage = FileWalletStorage::open(&data_dir)?;
    let mgr = WalletManager::new(storage)?;

    // 3. P9-1: env-var password (no plaintext literal)
    let password = match resolve_password() {
        Some(p) => p,
        None => std::process::exit(1),
    };

    // 4. Create + encrypt
    let id = mgr.create_with_mnemonic(&phrase, &password, "example", 0, 0, now_unix())?;

    // 5. P9-2: mnemonic to 0o600 file (NOT stdout)
    emit_mnemonic_securely(&data_dir, &phrase)?;

    // 6. Unlock → derive pubkey
    let lock = mgr.unlock(id, &password)?;
    let pubkey = lock.wallet().public_key();
    eprintln!("Wallet address: {}", pubkey);

    // 7. Optional devnet airdrop — gated
    if std::env::var("RUN_SOL_DEVNET_AIRDROP").is_ok() {
        // P9-6: explicit shutdown via Drop guard on the runtime
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| sol_wallet_core::Error::Transport(format!("tokio build: {e}")))?;
        let result: Result<()> = runtime.block_on(async {
            let rpc = RpcClient::new(&rpc_url)?;
            // P9-7: 0.5 SOL airdrop (below typical devnet per-request cap)
            let sig = request_airdrop(&rpc, &pubkey, 500_000_000).await?;
            eprintln!("Airdrop tx sig: {}", sig);
            eprintln!(
                "Verify: https://explorer.solana.com/tx/{}?cluster=devnet",
                sig
            );
            Ok(())
        });
        drop(runtime); // P9-6: explicit shutdown
        result?;
    } else {
        eprintln!("Skipping airdrop (set RUN_SOL_DEVNET_AIRDROP=1 to enable).");
    }

    eprintln!("Done.");
    Ok(())
}
