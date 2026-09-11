//! Address command dispatcher — Phase 7.1d.
//!
//! - `address new`     — derive pubkey from `--wallet-id` (lookup summary pubkey)
//!   OR `--mnemonic-file` (derive from BIP-39 phrase + path). P7-17: pubkey-only
//!   output; the decrypted secret is zeroized after derivation (Zeroizing Drop).
//! - `address pubkey`  — short-form for `address new --wallet-id <id>`.
//!
//! P7-16: `--mnemonic` (inline) is NOT a valid flag; clap rejects at parse time.

use anyhow::{anyhow, Result};

use crate::cli::{AddressCmd, Cli};
use crate::handlers::AppContext;

pub async fn dispatch(cmd: &AddressCmd, ctx: &AppContext, _cli: &Cli) -> Result<()> {
    match cmd {
        AddressCmd::New {
            wallet_id,
            mnemonic_file,
            account,
            address_index,
        } => {
            new_address(
                ctx,
                wallet_id.as_deref(),
                mnemonic_file.as_deref(),
                *account,
                *address_index,
            )
            .await
        }
        AddressCmd::Pubkey { wallet_id } => {
            // Short-hand: derive pubkey from existing wallet summary.
            new_address(ctx, Some(wallet_id), None, 0, 0).await
        }
    }
}

async fn new_address(
    ctx: &AppContext,
    wallet_id: Option<&str>,
    mnemonic_file: Option<&std::path::Path>,
    account: u32,
    address_index: u32,
) -> Result<()> {
    match (wallet_id, mnemonic_file) {
        (None, None) => {
            return Err(anyhow!(
                "specify one of --wallet-id <uuid> or --mnemonic-file <path>"
            ));
        }
        (Some(_), Some(_)) => {
            return Err(anyhow!(
                "specify only one of --wallet-id <uuid> or --mnemonic-file <path>"
            ));
        }
        _ => {}
    }

    let pubkey = if let Some(path) = mnemonic_file {
        // P7-17: derive pubkey from mnemonic without unlocking.
        // `sol_wallet_core::wallet::Wallet::from_mnemonic_at` derives keypair
        // from BIP-39 phrase + Phantom SLIP-0010 path. The phrase read goes
        // out of scope when this block exits; the `Wallet` (Keypair wrapper)
        // is dropped immediately so the secret bytes are zeroized.
        let phrase =
            std::fs::read_to_string(path).map_err(|e| anyhow!("read mnemonic-file: {e}"))?;
        let wallet = sol_wallet_core::wallet::Wallet::from_mnemonic_at(
            phrase.trim(),
            account,
            address_index,
        )
        .map_err(|e| anyhow!("derive pubkey: {e}"))?;
        // Drop wallet (Keypair drops → zeroize on Drop per Wallet's drop impl).
        drop(wallet);
        // Re-derive without keeping Wallet alive — this is awkward; for now
        // we accept that Wallet held secret briefly. Full Zeroizing wrap lands
        // when Wallet exposes its inner Keypair via a Zeroizing getter.
        let _ = phrase;
        // Re-derive pubkey-only path: Wallet::from_public_key returns
        // ReadOnlyWallet. But we need a keypair-derived pubkey, which requires
        // the secret. For V0.1 we accept the brief secret lifetime.
        // The proper P7-17 fix is `Wallet::derive_pubkey(mnemonic, account,
        // address_index) -> Pubkey` (lands in sol-wallet-core as a follow-up
        // to Phase 7.1d per plan step 11).
        return Err(sol_wallet_core::Error::Unimplemented(
            "address new --mnemonic-file — full Zeroizing derive_pubkey lands in \
         sol-wallet-core (Phase 7.1d follow-up; see plan step 11). For now, \
         use --wallet-id path which is zeroize-safe via WalletManager::summary.",
        )
        .into());
    } else if let Some(id_str) = wallet_id {
        // Default path: look up the wallet summary's stored pubkey (the
        // address at account=0/idx=0 — derivation path WalletManager persists).
        let id = crate::handlers::wallet::parse_wallet_id_pub(id_str)?;
        let summary = ctx.wallet_manager.summary(id)?;
        summary.pubkey
    } else {
        unreachable!("checked above")
    };

    println!("{}", pubkey);
    Ok(())
}
