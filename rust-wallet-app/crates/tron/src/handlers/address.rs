//! `tron address` handlers — plan §Phase 6 Task 5.3.

use std::path::{Path, PathBuf};

use tron_wallet_core::address::Address;
use tron_wallet_core::keys::{derive_keypair, xpub, DEFAULT_DERIVATION_PATH};

use super::{derivation_path, emit, resolve_mnemonic, unlock, Result};

/// `tron address new` — derive an address at `--index` or `--path`.
///
/// Public data only: the address and the path that produced it. The private
/// scalar stays inside `KeyPair` and is zeroized on drop.
pub fn new(
    mnemonic: Option<String>,
    mnemonic_file: Option<PathBuf>,
    index: u32,
    path: Option<String>,
    bip39_passphrase: String,
    json: bool,
) -> Result<()> {
    let mnemonic = resolve_mnemonic(mnemonic, mnemonic_file)?;
    let path = derivation_path(path.as_deref(), index)?;
    let keypair = derive_keypair(&mnemonic, &bip39_passphrase, &path)?;
    let address = Address::from_public_key(keypair.public_key())?.to_base58();
    emit(
        json,
        serde_json::json!({ "address": address, "path": path.to_string() }),
        &address,
    );
    Ok(())
}

/// `tron address xpub` — export the extended public key for a stored wallet.
///
/// An xpub is a watch-only credential: it exposes every future address of the
/// account. It is requested data, so STDOUT, but treat it as privacy-sensitive.
pub fn xpub_cmd(
    data_dir: &Path,
    wallet_id: String,
    password: Option<String>,
    path: Option<String>,
    json: bool,
) -> Result<()> {
    let mnemonic = unlock(data_dir, &wallet_id, password)?;
    let path_str = path.unwrap_or_else(|| DEFAULT_DERIVATION_PATH.to_string());
    let parsed = derivation_path(Some(&path_str), 0)?;
    let key = xpub(&mnemonic, "", &parsed)?;
    emit(
        json,
        serde_json::json!({ "xpub": key, "path": parsed.to_string() }),
        &key,
    );
    Ok(())
}
