//! Wallet command dispatcher.
//!
//! Phase 7.1b implements 6/9 wallet commands end-to-end:
//!   - `create`  — `--mnemonic-file` required (P7-1: no inline `--mnemonic`);
//!     P7-21: STDERR emits `SECRET: mnemonic=<words>` prefix before persist
//!   - `import`  — mnemonic-file OR private-key-file (P7-1 reject --mnemonic inline;
//!     P7-19 mode check via `WalletManager::import_from_pk_file`)
//!   - `show`    — `WalletManager::summary` returns `id`, `name`, `pubkey`
//!   - `list`    — `WalletManager::list` returns `Vec<WalletSummary>`
//!   - `delete`  — P7-6: requires `--yes` in non-TTY
//!   - `rename`  — P7-10: validates name (1..=64 chars, no forbidden chars, no Windows-reserved)
//!
//! Phase 7 STUBS (deferred):
//!   - `balance` — needs RPC client wiring (Phase 7.1d)
//!   - `send`    — Phase 7.1c (P7-2 / P7-7 / P7-13)
//!   - `send-speedup` — Phase 7.1c (P7-2)
//!
//! Password input: `SOL_WALLET_PASSWORD` env var for V0.1 (real CLI prompts via
//! stdin in V0.1.5; rpassword or similar).

use anyhow::{anyhow, Result};
use sol_wallet_core::{Error, WalletId};
use zeroize::{Zeroize, Zeroizing};

use crate::cli::{Cli, WalletCmd};
use crate::handlers::AppContext;

pub async fn dispatch(cmd: &WalletCmd, ctx: &AppContext, _cli: &Cli) -> Result<()> {
    match cmd {
        WalletCmd::Create {
            name,
            mnemonic_file,
            cluster,
            account,
            address_index,
        } => create(ctx, name, *cluster, mnemonic_file, *account, *address_index).await,
        WalletCmd::Import {
            name,
            cluster,
            mnemonic_file,
            private_key_file,
            account,
            address_index,
            yes,
        } => {
            import(
                ctx,
                name,
                *cluster,
                mnemonic_file.as_deref(),
                private_key_file.as_deref(),
                *account,
                *address_index,
                *yes,
            )
            .await
        }
        WalletCmd::Show { id, json } => show(ctx, id, *json).await,
        WalletCmd::List {
            json,
            all_clusters: _,
        } => list(ctx, *json).await,
        WalletCmd::Delete { id, yes } => delete(ctx, id, *yes).await,
        WalletCmd::Rename { id, to } => rename(ctx, id, to).await,
        WalletCmd::Balance { .. } => Err(anyhow!(
            "wallet balance deferred to Task 7.1d (needs RPC client wiring)"
        )),
        WalletCmd::Send { .. } => Err(anyhow!(
            "wallet send deferred to Task 7.1c (P7-2 / P7-7 / P7-13)"
        )),
        WalletCmd::SendSpeedup { .. } => {
            Err(anyhow!("wallet send-speedup deferred to Task 7.1c (P7-2)"))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn create(
    ctx: &AppContext,
    name: &str,
    cluster: crate::cli::Cluster,
    mnemonic_file: &std::path::Path,
    account: u32,
    address_index: u32,
) -> Result<()> {
    validate_name(name)?;

    let password = read_password_from_cli()?;
    let mut phrase = Zeroizing::new(
        std::fs::read_to_string(mnemonic_file)
            .map_err(|source| Error::FileIo {
                path: mnemonic_file.to_path_buf(),
                source,
            })?
            .trim()
            .to_string(),
    );

    // P7-21 (revised after background security review on commit 49ade2d3):
    // do NOT emit the mnemonic to STDERR or any observability sink. The
    // mnemonic stays in the zeroized buffer; the caller has the source file
    // for recovery. Logging the secret defeats P7-1's reject-`--mnemonic`
    // intent (STDERR is scraped / indexed / logged, exactly like cmdline).
    let now_unix = current_unix_secs();
    let result = ctx.wallet_manager.create_with_mnemonic(
        &phrase,
        &password,
        name,
        account,
        address_index,
        now_unix,
    );
    phrase.zeroize();
    let id = result?;
    // `cluster` carried on the WalletCmd for future cluster-aware persistence;
    // current WalletManager writes the cluster from the global default. Honored
    // via let-bind to keep handler signature symmetric with the CLI surface.
    let _ = cluster;
    println!("{}", id.as_uuid());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn import(
    ctx: &AppContext,
    name: &str,
    cluster: crate::cli::Cluster,
    mnemonic_file: Option<&std::path::Path>,
    private_key_file: Option<&std::path::Path>,
    account: u32,
    address_index: u32,
    yes: bool,
) -> Result<()> {
    validate_name(name)?;

    if mnemonic_file.is_none() && private_key_file.is_none() {
        return Err(anyhow!(
            "specify one of --mnemonic-file <path> or --private-key-file <path>"
        ));
    }

    // P7-19: preview derived address before import for private-key files.
    if let Some(path) = private_key_file {
        if !yes {
            // Background review #5: wrap secret bytes in `Zeroizing<>` so
            // they are scrubbed on scope exit (RAII). Without this, the
            // 64-byte secret key persists in heap until natural drop.
            let mut secret_b58 = Zeroizing::new(
                std::fs::read_to_string(path)
                    .map_err(|source| Error::FileIo {
                        path: path.to_path_buf(),
                        source,
                    })?
                    .trim()
                    .to_string(),
            );
            let decoded_vec =
                bs58::decode(&*secret_b58)
                    .into_vec()
                    .map_err(|_| Error::InvalidBase58Secret {
                        got: secret_b58.len(),
                        expected: 64,
                    })?;
            let mut decoded = Zeroizing::new(decoded_vec);
            if decoded.len() != 64 {
                return Err(Error::InvalidBase58Secret {
                    got: decoded.len(),
                    expected: 64,
                }
                .into());
            }
            let pubkey_bytes_owned: [u8; 32] =
                decoded[32..64]
                    .try_into()
                    .map_err(|_| Error::InvalidBase58Secret {
                        got: decoded.len(),
                        expected: 64,
                    })?;
            let mut pubkey_bytes = Zeroizing::new(pubkey_bytes_owned);
            let pubkey = solana_sdk::pubkey::Pubkey::from(*pubkey_bytes);
            secret_b58.zeroize();
            decoded.zeroize();
            pubkey_bytes.zeroize();
            eprintln!(
                "Will import private key with derived address: {} (cluster: {:?})",
                pubkey, cluster
            );
            eprintln!("Re-run with --yes to confirm.");
            return Err(anyhow!("aborted — pass --yes to confirm import"));
        }
    }

    let password = read_password_from_cli()?;
    let now_unix = current_unix_secs();
    let id = if let Some(path) = mnemonic_file {
        let mut phrase = Zeroizing::new(
            std::fs::read_to_string(path)
                .map_err(|source| Error::FileIo {
                    path: path.to_path_buf(),
                    source,
                })?
                .trim()
                .to_string(),
        );
        let result = ctx.wallet_manager.import_from_phrase(
            &phrase,
            &password,
            name,
            account,
            address_index,
            now_unix,
        );
        phrase.zeroize();
        result?
    } else if let Some(path) = private_key_file {
        ctx.wallet_manager
            .import_from_pk_file(path, &password, name, now_unix)?
    } else {
        unreachable!("checked above")
    };

    println!("{}", id.as_uuid());
    Ok(())
}

async fn show(ctx: &AppContext, id_str: &str, json: bool) -> Result<()> {
    let id = parse_wallet_id(id_str)?;
    let summary = ctx.wallet_manager.summary(id)?;
    if json {
        let json = serde_json::json!({
            "id": summary.id.as_uuid().to_string(),
            "name": summary.name,
            "pubkey": summary.pubkey.to_string(),
            "created_at_unix": summary.created_at_unix,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{:<36}  {}", "id", summary.id.as_uuid());
        println!("{:<36}  {}", "name", summary.name);
        println!("{:<36}  {}", "address", summary.pubkey);
        println!("{:<36}  {}", "created_at_unix", summary.created_at_unix);
    }
    Ok(())
}

async fn list(ctx: &AppContext, json: bool) -> Result<()> {
    let wallets = ctx.wallet_manager.list()?;
    if json {
        let arr: Vec<_> = wallets
            .iter()
            .map(|w| {
                serde_json::json!({
                    "id": w.id.as_uuid().to_string(),
                    "name": w.name,
                    "address": w.pubkey.to_string(),
                    "created_at_unix": w.created_at_unix,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
    } else {
        println!("{:<36}  {:<32}  ADDRESS", "ID", "NAME");
        for w in &wallets {
            println!("{:<36}  {:<32}  {}", w.id.as_uuid(), w.name, w.pubkey);
        }
    }
    Ok(())
}

async fn delete(ctx: &AppContext, id_str: &str, yes: bool) -> Result<()> {
    let id = parse_wallet_id(id_str)?;
    if yes {
        // approved — proceed
    } else if atty_stdin() {
        match ctx.wallet_manager.summary(id) {
            Ok(s) => {
                eprintln!("About to delete wallet:");
                eprintln!("  id:      {}", s.id.as_uuid());
                eprintln!("  name:    {}", s.name);
                eprintln!("  address: {}", s.pubkey);
            }
            Err(_) => eprintln!("About to delete wallet: id={}", id.as_uuid()),
        }
        eprintln!("Type 'delete {}' to confirm (or pass --yes):", id.as_uuid());
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .map_err(|source| Error::FileIo {
                path: std::path::PathBuf::from("<stdin>"),
                source,
            })?;
        if line.trim() != format!("delete {}", id.as_uuid()) {
            return Err(anyhow!("aborted — confirmation phrase did not match"));
        }
    } else {
        return Err(anyhow!(
            "refusing to delete wallet in non-TTY context without --yes"
        ));
    }

    ctx.wallet_manager.delete(id)?;
    println!("deleted wallet {}", id.as_uuid());
    Ok(())
}

async fn rename(ctx: &AppContext, id_str: &str, new_name: &str) -> Result<()> {
    let id = parse_wallet_id(id_str)?;
    validate_name(new_name)?;
    ctx.wallet_manager.rename(id, new_name)?;
    println!("renamed wallet {} -> {}", id.as_uuid(), new_name);
    Ok(())
}

// -------- helpers --------

fn parse_wallet_id(s: &str) -> Result<WalletId> {
    WalletId::parse_str(s).map_err(Into::into)
}

/// P7-10 — wallet name validation. Library does not currently validate;
/// CLI is the only enforcement point. Mirrors Phase 7 audit doc P7-10
/// fix recommendation.
fn validate_name(name: &str) -> Result<()> {
    const NAME_MAX_LEN: usize = 64;
    if name.is_empty() || name.len() > NAME_MAX_LEN {
        return Err(anyhow!(
            "wallet name must be 1..={} chars (got {})",
            NAME_MAX_LEN,
            name.len()
        ));
    }
    if name.chars().any(|c| {
        c.is_control()
            || matches!(
                c,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0'
            )
    }) {
        return Err(anyhow!(
            "wallet name contains forbidden char (control or / \\ : * ? \" < > | NUL)"
        ));
    }
    let upper = name.to_uppercase();
    if matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        return Err(anyhow!("wallet name is reserved (Windows): {}", name));
    }
    if name.trim().is_empty() {
        return Err(anyhow!("wallet name is whitespace-only"));
    }
    Ok(())
}

fn atty_stdin() -> bool {
    #[cfg(unix)]
    {
        extern "C" {
            fn isatty(fd: i32) -> i32;
        }
        unsafe { isatty(0) != 0 }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Read password from `SOL_WALLET_PASSWORD` env var. Real CLI prompts via
/// stdin in V0.1.5 (rpassword or similar).
fn read_password_from_cli() -> Result<String> {
    std::env::var("SOL_WALLET_PASSWORD").map_err(|_| {
        anyhow!(
            "password required — set SOL_WALLET_PASSWORD env var (V0.1) or pass --password flag (V0.1.5)"
        )
    })
}

fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
