//! Handlers for `btc wallet create` + `btc wallet show` subcommands.
//!
//! **F49 / L28**: wallet_id → STDOUT (scriptable), mnemonic → STDERR
//! (operator-only). Regression test enforces in `tests/cli.rs`.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};

use bitcoin::CompressedPublicKey;
use bitcoin_wallet_core::chain::esplora::{EsploraClient, TlsPolicy};
use bitcoin_wallet_core::chain::esplora_url::EsploraUrl;
use bitcoin_wallet_core::chain::network::coin_type_for;
use bitcoin_wallet_core::chain::spki::SpkiPin;
use bitcoin_wallet_core::config::WalletConfig;
use bitcoin_wallet_core::crypto::aad::Aad;
use bitcoin_wallet_core::crypto::bip137::{sign_message, verify_message, SignedMessage};
use bitcoin_wallet_core::crypto::mnemonic_cipher::{
    decrypt_mnemonic, encrypt_mnemonic, MnemonicCipherBlob,
};
use bitcoin_wallet_core::keys::{AddressType, Mnemonic, Secret, Signer, XPrvHolder};
use bitcoin_wallet_core::util::atomic_write::atomic_write;
use bitcoin_wallet_core::wallet::{
    create_wallet, show_wallet, KeychainKind, Wallet, WalletId, SUPPORTED_WORD_COUNTS,
};

use crate::cli::{NetArg, WordCount};

/// Default Esplora URLs per Bitcoin network (Issue #74).
///
/// **Coverage rationale**:
/// - `bitcoin` / `testnet` / `signet` — public blockstream.info endpoints
///   (HTTPS by default; F20 SPKI pin optional for testnet, **required**
///   for mainnet/signet per `EsploraClient::from_config`)
/// - `testnet4` — public mempool.space endpoint (newer network;
///   blockstream doesn't run testnet4)
/// - `regtest` — **no default**. Local-only network; typical setup uses
///   `http://localhost:50002` which is rejected by `EsploraUrl::new`
///   (HTTPS-only per F20). Operators must pass `--esplora-url` with
///   an HTTPS-terminating proxy or expose regtest behind stunnel.
const DEFAULT_BITCOIN_ESPLORA: &str = "https://blockstream.info/api";
const DEFAULT_TESTNET_ESPLORA: &str = "https://blockstream.info/testnet/api";
const DEFAULT_TESTNET4_ESPLORA: &str = "https://mempool.space/testnet4/api";
const DEFAULT_SIGNET_ESPLORA: &str = "https://blockstream.info/signet/api";

/// Default Esplora URL for a given network, or `None` if no sensible
/// default exists (currently: regtest).
pub(crate) fn default_url_for(network: bitcoin::Network) -> Option<&'static str> {
    match network {
        bitcoin::Network::Bitcoin => Some(DEFAULT_BITCOIN_ESPLORA),
        bitcoin::Network::Testnet => Some(DEFAULT_TESTNET_ESPLORA),
        bitcoin::Network::Testnet4 => Some(DEFAULT_TESTNET4_ESPLORA),
        bitcoin::Network::Signet => Some(DEFAULT_SIGNET_ESPLORA),
        bitcoin::Network::Regtest => None,
    }
}

/// Resolve the wallet data directory:
///   explicit `--data-dir` flag > `$BTC_DATA_DIR` env > `$XDG_DATA_HOME`.
///
/// **Important**: we return the **raw** `$XDG_DATA_HOME` (no `btc`
/// suffix). The library's `wallet_path_at(base, ...)` appends
/// `btc/wallets/<network>/<id>.enc` to whatever base it's given,
/// so passing `$XDG_DATA_HOME/btc` here would yield a doubled
/// `btc/btc/` segment. Tests rely on this contract.
pub fn resolve_data_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg));
        }
    }
    let home = std::env::var("HOME").context("HOME not set; required for default data_dir")?;
    Ok(PathBuf::from(home).join(".local").join("share"))
}

/// Prompt for password if not provided via `--password` flag.
/// `rpassword::prompt_password` reads from `/dev/tty` (not stdin)
/// to avoid leaking via piped input.
fn password_or_prompt(flag: Option<String>) -> Result<String> {
    match flag {
        Some(p) if !p.is_empty() => Ok(p),
        _ => rpassword::prompt_password("Wallet password: ").context("password prompt failed"),
    }
}

/// Wrap plaintext password in zeroizing `Secret<Vec<u8>>` for the
/// library API. Wrapped once at the boundary; the plaintext `String`
/// is moved into the `Secret` (no copies survive past this point).
fn secret_password(plaintext: String) -> Secret<Vec<u8>> {
    Secret::new(plaintext.into_bytes())
}

/// Parse a 64-char hex SPKI pin string into `SpkiPin`.
///
/// **Format**: SHA-256 of the leaf cert's SubjectPublicKeyInfo, as 64
/// lowercase or uppercase hex chars. The CLI accepts this format
/// (operator-friendly) and converts to 32 raw bytes internally; the
/// library's `SpkiPin::from_base64` would require base64 encoding
/// instead — we deliberately use hex for CLI ergonomics (operators
/// copy SPKI hashes from TLS inspection tools that typically render
/// hex).
///
/// # Errors
///
/// `anyhow::Error` with `context` on:
/// - wrong length (not 64 chars)
/// - non-hex characters
pub fn parse_spki_pin_hex(s: &str) -> Result<SpkiPin> {
    if s.len() != 64 {
        anyhow::bail!(
            "SPKI pin must be 64 hex chars (SHA-256 of SubjectPublicKeyInfo); got {} chars",
            s.len()
        );
    }
    let mut bytes = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(chunk).context("SPKI pin contains non-ASCII byte")?;
        bytes[i] = u8::from_str_radix(pair, 16)
            .with_context(|| format!("SPKI pin contains non-hex pair {pair:?} at position {i}"))?;
    }
    Ok(SpkiPin::from_bytes(bytes))
}

pub async fn handle_create(
    words: WordCount,
    network: NetArg,
    password: Option<String>,
    data_dir: &Path,
) -> Result<()> {
    let n = words.as_usize();
    if !SUPPORTED_WORD_COUNTS.contains(&n) {
        anyhow::bail!("unsupported BIP-39 word count: {n} (supported: {SUPPORTED_WORD_COUNTS:?})");
    }
    let pwd_plain = password_or_prompt(password)?;
    let pwd = secret_password(pwd_plain);
    let network_obj = network.as_network();

    let (wallet_id, mnemonic) =
        create_wallet(data_dir, network_obj, n, &pwd).context("create_wallet failed")?;
    let wallet_id_str = wallet_id.to_string();
    let phrase = mnemonic.expose().to_string();

    // F49 / L28: wallet_id on STDOUT (scriptable), mnemonic on
    // STDERR (operator-only). The mnemonic never lands on STDOUT
    // so it can't leak via shell history, CI capture, or pipes.
    println!("{wallet_id_str}");
    eprintln!("Mnemonic (write this down; never stored in plaintext):");
    eprintln!("{phrase}");
    Ok(())
}

pub async fn handle_show(
    id: String,
    network: NetArg,
    password: Option<String>,
    esplora_url: Option<String>,
    esplora_spki_pin: Option<String>,
    data_dir: &Path,
) -> Result<()> {
    let pwd_plain = password_or_prompt(password)?;
    let pwd = secret_password(pwd_plain);
    let wallet_id = WalletId::from_str(&id).with_context(|| format!("parsing wallet_id {id:?}"))?;
    let network_obj = network.as_network();
    let url = esplora_url
        .as_deref()
        .or_else(|| default_url_for(network_obj))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "--network regtest has no default Esplora URL; \
                 pass --esplora-url <https://...> (regtest localhost behind \
                 HTTPS-terminating proxy or stunnel is recommended per F20)"
            )
        })?;
    let esplora_url_typed =
        EsploraUrl::new(url).with_context(|| format!("invalid esplora url {url:?}"))?;

    // F20 enforcement: when the operator passes `--esplora-spki-pin`
    // (or `BTC_ESPLORA_SPKI_PIN` env), route through
    // `EsploraClient::from_config` which applies `TlsPolicy::Pinned`.
    // When unset, fall back to `EsploraClient::new(url, SystemRoots)`
    // (PR-2 default — testnet-suitable; mainnet/signet/regtest without
    // a pin will fail at network level per ADR 0001 / F20).
    let client = match esplora_spki_pin {
        Some(pin_hex) => {
            let pin = parse_spki_pin_hex(&pin_hex)
                .with_context(|| format!("parsing --esplora-spki-pin ({pin_hex:?})"))?;
            // `db_path` is unused for show (only the lib's bdk_file_store
            // integration needs it, deferred per F14). Use a placeholder
            // path so the WalletConfig builds cleanly. Show does not
            // touch the sidecar DB.
            let mut cfg = WalletConfig::testnet(url, data_dir.join("placeholder.sqlite"))
                .with_esplora_spki_pin(pin);
            // Override the testnet default with the actual requested
            // network. WalletConfig has dedicated constructors for
            // Bitcoin/Signet/Regtest but not Testnet4, so we mutate
            // the pub `network` field (allowed despite
            // `#[non_exhaustive]`).
            cfg.network = network_obj;
            EsploraClient::from_config(&cfg).context("build pinned Esplora client")?
        }
        None => EsploraClient::new(esplora_url_typed, TlsPolicy::SystemRoots)
            .context("build Esplora client (SystemRoots)")?,
    };

    let info = show_wallet(data_dir, network_obj, wallet_id, &pwd, &client)
        .await
        .context("show_wallet failed")?;

    // Pretty JSON to STDOUT for piping / scripting.
    let json = serde_json::to_string_pretty(&info).context("serializing wallet info")?;
    println!("{json}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url_for_bitcoin() {
        assert_eq!(
            default_url_for(bitcoin::Network::Bitcoin),
            Some(DEFAULT_BITCOIN_ESPLORA)
        );
    }

    #[test]
    fn default_url_for_testnet() {
        assert_eq!(
            default_url_for(bitcoin::Network::Testnet),
            Some(DEFAULT_TESTNET_ESPLORA)
        );
    }

    #[test]
    fn default_url_for_testnet4() {
        assert_eq!(
            default_url_for(bitcoin::Network::Testnet4),
            Some(DEFAULT_TESTNET4_ESPLORA)
        );
    }

    #[test]
    fn default_url_for_signet() {
        assert_eq!(
            default_url_for(bitcoin::Network::Signet),
            Some(DEFAULT_SIGNET_ESPLORA)
        );
    }

    #[test]
    fn default_url_for_regtest_is_none() {
        assert!(
            default_url_for(bitcoin::Network::Regtest).is_none(),
            "regtest has no public Esplora endpoint; operator must pass --esplora-url"
        );
    }

    #[test]
    fn parse_spki_pin_hex_accepts_valid_64_lower_hex() {
        let pin_hex = "0".repeat(64);
        let pin = parse_spki_pin_hex(&pin_hex).expect("valid hex should parse");
        let serialized = pin.to_string();
        assert!(!serialized.is_empty(), "serialized pin should be non-empty");
    }

    #[test]
    fn parse_spki_pin_hex_rejects_too_short() {
        let pin_hex = "ab".repeat(20); // 40 chars — too short
        let err = parse_spki_pin_hex(&pin_hex).expect_err("too short should reject");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("64 hex chars"),
            "error should mention 64-char requirement: {msg}"
        );
    }
}

// ============================================================================
// Issue #61 / Task 54a: `btc message sign|verify` handlers (BIP-137 stateless)
// ============================================================================

/// BIP-44 path for the **first external receive address** of an account.
/// Signing from non-first-external addresses is deferred to v0.1.1
/// (out of Issue #61 scope).
fn first_external_derivation_path(network: bitcoin::Network) -> Result<bip32::DerivationPath> {
    let coin = coin_type_for(network);
    let purpose = AddressType::NativeSegwit.purpose();
    let path_str = format!("m/{purpose}h/{coin}h/0h/0/0");
    bip32::DerivationPath::from_str(&path_str)
        .with_context(|| format!("building BIP-44 derivation path for {network:?}"))
}

/// Derive a [`Signer`] for the first external address from a BIP-39
/// mnemonic + network. Pure function (no FS, no IO, no async).
///
/// **v0.1 limitation**: only the first external address is supported.
fn derive_first_external_signer(
    mnemonic_phrase: &str,
    network: bitcoin::Network,
) -> Result<(Signer, bitcoin::Address)> {
    let mnemonic =
        Mnemonic::from_phrase(mnemonic_phrase).context("invalid BIP-39 mnemonic phrase")?;
    let seed = mnemonic.to_seed("");
    let seed_arr: [u8; 64] = seed.expose().as_slice().try_into().map_err(|_| {
        anyhow::anyhow!("BIP-39 seed must be 64 bytes (got {})", seed.expose().len())
    })?;
    let master =
        XPrvHolder::master_from_seed(&seed_arr).context("deriving master xprv from seed")?;
    let path = first_external_derivation_path(network)?;
    let derived = master
        .derive(&path)
        .context("deriving first external xprv")?;
    let signer = Signer::from_xprv(&derived).context("constructing Signer from xprv")?;
    let compressed_pk = CompressedPublicKey(signer.public_key());
    // BIP-137 requires P2PKH (legacy 1.../m.../n...) addresses per
    // lib contract. P2WPKH (tb1.../bc1...) is rejected by `sign_message`
    // with `Bip137("address is not P2PKH")`. Future v0.1.1 may add
    // BIP-322 (Taproot/SegWit message signing).
    let address = bitcoin::Address::p2pkh(compressed_pk, network);
    Ok((signer, address))
}

/// Handle `btc message sign --mnemonic <words> --network <NET> --address <ADDR> <MSG>`.
///
/// Prints the base64 BIP-137 signature to STDOUT.
pub fn handle_message_sign(
    mnemonic_phrase: String,
    network: NetArg,
    address_arg: String,
    message: String,
) -> Result<()> {
    let network_obj = network.as_network();
    let (signer, derived_address) = derive_first_external_signer(&mnemonic_phrase, network_obj)
        .context("deriving first-external signer")?;

    let claimed_address = bitcoin::Address::from_str(&address_arg)
        .with_context(|| format!("parsing --address {address_arg:?}"))?
        .require_network(network_obj)
        .with_context(|| {
            format!("--address {address_arg:?} is not valid for network {network_obj:?}")
        })?;

    // L12 CRITICAL: refuse if --address doesn't match the derived address.
    if claimed_address != derived_address {
        anyhow::bail!(
            "--address {claimed_address} does not match the first-external \
             address derived from the mnemonic ({derived_address}); v0.1 \
             only signs with the first external key (m/44'/coin'/0'/0/0). \
             v0.1.1 will add a key-search option for arbitrary addresses."
        );
    }

    let signed =
        sign_message(&message, &signer, &claimed_address).context("BIP-137 sign_message failed")?;
    println!("{}", signed.as_ref());
    Ok(())
}

/// Handle `btc message verify --address <ADDR> <MSG> <SIG_B64>`.
///
/// Prints `true`/`false` to STDOUT. Returns `Ok(())` for both valid and
/// invalid signatures (verification result is encoded in the boolean
/// output); errors (parse failures, header rejection) propagate via `Err`.
pub fn handle_message_verify(
    address_arg: String,
    message: String,
    signature_b64: String,
) -> Result<()> {
    let signed = SignedMessage::from_str(&signature_b64)
        .with_context(|| format!("parsing signature {signature_b64:?}"))?;
    // Infer network from bech32 HRP. BIP-137 P2PKH addresses use
    // bech32 (tb1/bc1/bcrt1), so the HRP uniquely determines the
    // network. The legacy P2PKH (1/m/n) form is rejected by the
    // network check downstream if mismatched — not supported by this
    // CLI in v0.1.
    let network = network_from_address_prefix(&address_arg).with_context(|| {
        format!(
            "inferring network from --address {address_arg:?} \
             (tb1/m/n testnet, bc1/1 mainnet, bcrt1 regtest)"
        )
    })?;
    let claimed_address = bitcoin::Address::from_str(&address_arg)
        .with_context(|| format!("parsing --address {address_arg:?}"))?
        .require_network(network)
        .with_context(|| format!("require_network failed for {address_arg:?}"))?;
    let valid = verify_message(&claimed_address, &signed, &message)
        .context("BIP-137 verify_message failed")?;
    println!("{valid}");
    Ok(())
}

/// Infer the Bitcoin network from an address prefix (bech32 HRP OR
/// legacy P2PKH base58 prefix). BIP-137 signs legacy P2PKH per lib
/// caller contract, so the CLI accepts both formats:
/// - bech32 (P2WPKH): `tb1` (testnet), `bc1` (mainnet), `bcrt1` (regtest)
/// - base58 P2PKH: `m`/`n` (testnet), `1` (mainnet)
///
/// Signet + Testnet4 use the same `tb1` HRP as Testnet; callers
/// disambiguate via the `bitcoin::Network` enum if needed. For v0.1
/// verify, the network is inferred from the prefix and
/// `Address::require_network` enforces it.
fn network_from_address_prefix(s: &str) -> Result<bitcoin::Network> {
    if s.starts_with("tb1") || s.starts_with('m') || s.starts_with('n') {
        Ok(bitcoin::Network::Testnet)
    } else if s.starts_with("bc1") || s.starts_with('1') {
        Ok(bitcoin::Network::Bitcoin)
    } else if s.starts_with("bcrt1") {
        Ok(bitcoin::Network::Regtest)
    } else {
        anyhow::bail!(
            "address {s:?} does not match a known prefix \
             (tb1/m/n testnet, bc1/1 mainnet, bcrt1 regtest)"
        )
    }
}

// ============================================================================
// Issue #62 / Task 54b: `btc encrypt|decrypt` handlers (stateless file ops)
// ============================================================================
//
// **Threat-model coverage** (per issue #62 + `crypto::mnemonic_cipher`):
// - **F5** (Argon2id KDF m=256 MiB / t=10 / p=4) — offline-cracker resistance.
// - **F6** (AES-256-GCM AEAD, 96-bit random nonce per blob) — confidentiality + integrity.
// - **F47** (zeroize on drop) — `Secret<Vec<u8>>` (password) + `Secret<String>` (plaintext).
// - **N2 oracle mitigation**: wrong-password / tampered / truncated / non-UTF8
//   all surface as a uniform "decrypt failed" message (handler collapses).
// - **F19 atomic write**: `atomic_write` (write-to-temp + fsync + parent fsync
//   + rename) for both `--out` paths — no partial ciphertext / plaintext
//   on disk after a crash, 0o600 permissions, symlink destination rejected.
//
// **AAD choice (v0.1):** `Aad::NONE`. The encrypt/decrypt subcommands are
// generic file ops with no caller-side context to bind. Future v0.1.1
// may add `--aad <hex>` for caller-bound context (matches the wallet
// store's `Aad::network(net)` per ADR 0001).

// ============================================================================
// Issue #63 / Task 54c: `btc wallet sync|balance` handlers (stateless chain ops)
// ============================================================================
//
// **Threat-model coverage** (per issue #63 body):
// - **F12** (chain sync via Esplora `/address/{addr}/utxo`)
// - **F13** (confirmed-only UTXO aggregation, MAX_MONEY cap)
// - **F20** (Esplora SPKI pinning — required for non-regtest)
// - **F36** (https-only URL — `EsploraUrl::new` rejects http:// per #36)
//
// **Stateless design**: these handlers do not touch the wallet store.
// The mnemonic is parsed → wrapped in `Mnemonic` → passed to
// `Wallet::from_mnemonic` → `wallet.sync` / `wallet.balance`. No
// `WalletId`, no `MnemonicCipherBlob`, no `data_dir`. F14 (UTXO
// persistence) explicitly out of scope; each invocation re-syncs.

/// Scan window for `Wallet::sync` UTXO discovery (matches the
/// library's `wallet::SCAN_GAP_LIMIT`). Duplicated here because the
/// library constant is module-private. Both values must move
/// together if the scan window ever changes.
const SYNC_SCAN_GAP_LIMIT: u32 = 5;

/// Build an `EsploraClient` for stateless sync/balance handlers.
///
/// **F20 enforcement**: non-regtest networks REQUIRE a SPKI pin (or
/// the call fails with an anyhow error before any network IO).
/// Regtest may omit the pin (operator's localhost may use SystemRoots
/// during development). Mirrors `handle_show`'s construction (Issue #74).
fn build_esplora_client_for(
    network_obj: bitcoin::Network,
    esplora_url: &str,
    pin_spki: Option<&str>,
    data_dir: &Path,
) -> Result<EsploraClient> {
    let esplora_url_typed = EsploraUrl::new(esplora_url)
        .with_context(|| format!("invalid esplora url {esplora_url:?}"))?;
    match pin_spki {
        Some(pin_hex) => {
            let pin = parse_spki_pin_hex(pin_hex)
                .with_context(|| format!("parsing --pin-spki ({pin_hex:?})"))?;
            let cfg = WalletConfig::testnet(esplora_url, data_dir.join("placeholder.sqlite"))
                .with_esplora_spki_pin(pin);
            // `cfg` is constructed via the testnet convenience ctor
            // because it carries the SPKI-pin setter; mutate the
            // `#[non_exhaustive]` `network` field to the actual
            // requested network (same pattern as `handle_show`).
            let mut cfg = cfg;
            cfg.network = network_obj;
            EsploraClient::from_config(&cfg).context("build pinned Esplora client")
        }
        None => {
            // F20: non-regtest without a pin is unsafe (TLS chain is
            // only as trusted as the system CA store, which the
            // operator cannot pin for public Esplora servers). Refuse.
            if network_obj != bitcoin::Network::Regtest {
                anyhow::bail!(
                    "--pin-spki is required for non-regtest networks (F20 enforcement); \
                     pass --pin-spki <64-char hex SHA-256 of leaf SubjectPublicKeyInfo> \
                     or use --network regtest for localhost development"
                );
            }
            EsploraClient::new(esplora_url_typed, TlsPolicy::SystemRoots)
                .context("build Esplora client (SystemRoots)")
        }
    }
}

/// Handle `btc wallet sync --mnemonic <words> --network <NET> --esplora-url <URL> [--pin-spki <hex64>]`.
///
/// Prints `n_utxos=<N> total_sat=<S>` to STDOUT (scriptable). The
/// mnemonic lives only in process memory; nothing is persisted.
pub async fn handle_wallet_sync(
    mnemonic_phrase: String,
    network: NetArg,
    esplora_url: String,
    pin_spki: Option<String>,
) -> Result<()> {
    let network_obj = network.as_network();
    // Stateless: use a throwaway placeholder base so WalletConfig
    // builds cleanly (EsploraClient::from_config requires a
    // `db_path` even when no DB is written).
    let tmp_base = std::env::temp_dir();
    let client =
        build_esplora_client_for(network_obj, &esplora_url, pin_spki.as_deref(), &tmp_base)?;
    let mnemonic =
        Mnemonic::from_phrase(&mnemonic_phrase).context("invalid BIP-39 mnemonic phrase")?;
    let wallet =
        Wallet::from_mnemonic(&mnemonic, network_obj).context("Wallet::from_mnemonic failed")?;
    wallet.sync(&client).await.context("Wallet::sync failed")?;
    let balance = wallet
        .balance(&client)
        .await
        .context("Wallet::balance failed")?;
    // UTXO count: count all confirmed UTXOs across the first
    // SCAN_GAP_LIMIT external addresses by querying Esplora directly
    // (avoids reaching into bdk's internal UTXO set, which is
    // private).
    let addresses = wallet
        .peek_addresses(KeychainKind::External, SYNC_SCAN_GAP_LIMIT)
        .context("peek_addresses failed after sync")?;
    let mut n_utxos = 0u64;
    for addr in &addresses {
        let utxos = client
            .address_utxos(addr)
            .await
            .with_context(|| format!("address_utxos {addr}"))?;
        n_utxos += utxos.len() as u64;
    }
    println!("n_utxos={n_utxos} total_sat={balance}");
    Ok(())
}

/// Handle `btc wallet balance --mnemonic <words> --network <NET> --esplora-url <URL> [--pin-spki <hex64>]`.
///
/// Prints the confirmed balance in sats to STDOUT (single integer,
/// scriptable). The mnemonic lives only in process memory; nothing
/// is persisted.
pub async fn handle_wallet_balance(
    mnemonic_phrase: String,
    network: NetArg,
    esplora_url: String,
    pin_spki: Option<String>,
) -> Result<()> {
    let network_obj = network.as_network();
    let tmp_base = std::env::temp_dir();
    let client =
        build_esplora_client_for(network_obj, &esplora_url, pin_spki.as_deref(), &tmp_base)?;
    let mnemonic =
        Mnemonic::from_phrase(&mnemonic_phrase).context("invalid BIP-39 mnemonic phrase")?;
    let wallet =
        Wallet::from_mnemonic(&mnemonic, network_obj).context("Wallet::from_mnemonic failed")?;
    let balance = wallet
        .balance(&client)
        .await
        .context("Wallet::balance failed")?;
    println!("{balance}");
    Ok(())
}

/// Cap for `--in` plaintext size on encrypt (1 MiB). Library has
/// `MAX_PLAINTEXT_LEN=256` for BIP-39 phrases; CLI accepts any UTF-8
/// so we cap higher (1 MiB) for general file ops but still reject
/// multi-GB inputs before allocating (N2 DoS mitigation).
const ENCRYPT_PLAINTEXT_MAX_BYTES: u64 = 1024 * 1024;

/// Guard: reject `--in == --out` (otherwise encrypt truncates its own input).
fn reject_same_in_out(in_path: &Path, out_path: &Path) -> Result<()> {
    if in_path == out_path {
        anyhow::bail!("--in and --out must differ (refusing to overwrite source)");
    }
    Ok(())
}

/// Pre-check `--in` size before allocating. Library enforces
/// `MAX_PLAINTEXT_LEN=256` for BIP-39 specifically; the CLI accepts any
/// UTF-8 so we cap at `ENCRYPT_PLAINTEXT_MAX_BYTES` for encrypt and at
/// `MAX_LEN` (300) for decrypt (the library cap minus headroom).
/// Also rejects `--in` symlinks (no following — would encrypt whatever
/// the symlink points to, possibly surprising the operator).
fn check_input_size(in_path: &Path, max_bytes: u64) -> Result<()> {
    let meta =
        std::fs::symlink_metadata(in_path).with_context(|| format!("stat --in {in_path:?}"))?;
    if meta.file_type().is_symlink() {
        anyhow::bail!("--in must not be a symlink (refusing to follow)");
    }
    let len = meta.len();
    if len > max_bytes {
        anyhow::bail!(
            "--in size {len} bytes exceeds cap {max_bytes} bytes; \
             refusing to allocate (rejected before reading)"
        );
    }
    Ok(())
}

/// Handle `btc encrypt --password <pwd> --in <file> --out <file>`.
///
/// Reads plaintext from `--in` (UTF-8), encrypts with `crypto::mnemonic_cipher`
/// (F5 Argon2id KDF + F6 AES-256-GCM), atomically writes the
/// `MnemonicCipherBlob` bytes to `--out`. No persistence outside the
/// operator's chosen paths.
pub fn handle_encrypt(password: Option<String>, in_path: PathBuf, out_path: PathBuf) -> Result<()> {
    reject_same_in_out(&in_path, &out_path)?;
    check_input_size(&in_path, ENCRYPT_PLAINTEXT_MAX_BYTES)?;
    let plaintext_bytes =
        std::fs::read(&in_path).with_context(|| format!("reading --in {in_path:?}"))?;
    // UTF-8 boundary: MnemonicCipher requires `Secret<String>`. Non-UTF8
    // is rejected here with a clear error (not silently corrupted).
    let plaintext = String::from_utf8(plaintext_bytes)
        .context("--in file is not valid UTF-8; convert to UTF-8 before encrypting")?;
    if plaintext.is_empty() {
        anyhow::bail!("--in file is empty; refusing to encrypt an empty plaintext");
    }
    // F47: wrap plaintext in zeroizing Secret<String>.
    let phrase = Secret::new(plaintext);
    let pwd = secret_password(password_or_prompt(password)?);
    let blob = encrypt_mnemonic(&phrase, pwd.expose().as_slice(), Aad::NONE)
        .context("encrypt_mnemonic failed")?;
    // Zeroize plaintext Secret BEFORE atomic_write (which copies the
    // ciphertext bytes into the temp file). After atomic_write returns
    // the only on-disk artifact is the ciphertext.
    drop(phrase);
    atomic_write(&out_path, blob.as_bytes())
        .with_context(|| format!("writing --out {out_path:?}"))?;
    Ok(())
}

/// Handle `btc decrypt --password <pwd> --in <file> --out <file>`.
///
/// Reads `MnemonicCipherBlob` bytes from `--in`, decrypts, atomically
/// writes the recovered UTF-8 plaintext to `--out`. Errors on wrong
/// password / tampered blob / truncated blob / non-UTF8 plaintext — all
/// surface as a **uniform** "decrypt failed" message (N2 oracle
/// mitigation: caller can't probe which failure mode occurred).
pub fn handle_decrypt(password: Option<String>, in_path: PathBuf, out_path: PathBuf) -> Result<()> {
    reject_same_in_out(&in_path, &out_path)?;
    // Library cap: MAX_LEN = 300 (44 overhead + 256 plaintext).
    check_input_size(&in_path, MnemonicCipherBlob::MAX_LEN as u64)?;
    let blob_bytes =
        std::fs::read(&in_path).with_context(|| format!("reading --in {in_path:?}"))?;
    // N2 oracle mitigation: collapse both truncated-blob AND decrypt-fail
    // errors to a single uniform message. Caller cannot probe which
    // failure mode occurred.
    let blob = MnemonicCipherBlob::try_from(blob_bytes.as_slice())
        .map_err(|_| anyhow::anyhow!("decrypt failed"))?;
    let pwd = secret_password(password_or_prompt(password)?);
    let phrase_secret = decrypt_mnemonic(&blob, pwd.expose().as_slice(), Aad::NONE)
        .map_err(|_| anyhow::anyhow!("decrypt failed"))?;
    // F47 fix: borrow directly from the zeroizing Secret (no .clone()
    // into a non-zeroizing String). The Secret stays alive until end of
    // scope and zeroizes on drop.
    let plaintext_bytes = phrase_secret.expose().as_bytes();
    atomic_write(&out_path, plaintext_bytes).map_err(|e| anyhow::anyhow!("decrypt failed: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod message_tests {
    use super::*;

    /// Deterministic testnet mnemonic (well-known test vector; do NOT
    /// use for real funds — exposed in plaintext here for unit tests).
    /// Source: BIP-39 test vector 12-word phrase.
    const TEST_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon \
                                abandon abandon abandon abandon abandon about";

    #[test]
    fn derive_first_external_signer_testnet_deterministic() {
        let (signer, address) =
            derive_first_external_signer(TEST_MNEMONIC, bitcoin::Network::Testnet)
                .expect("deterministic mnemonic + network must derive");
        // Address must be a valid testnet P2WPKH (starts with `tb1q`).
        assert!(
            address.to_string().starts_with("mz"),
            "expected mz... testnet P2PKH, got {address}"
        );
        // Signer should be reusable — public key derives to the same address.
        let pk = signer.public_key();
        let addr_from_pk =
            bitcoin::Address::p2pkh(CompressedPublicKey(pk), bitcoin::Network::Testnet);
        assert_eq!(addr_from_pk, address);
    }

    /// Parametrized sign + verify roundtrip across all 5 networks.
    /// `bitcoin` (coin type 0) derives a unique key; the 4 testnet-family
    /// networks (testnet / signet / testnet4 / regtest) all share coin
    /// type 1 → identical key + signature. The test asserts each
    /// network's full path: derive signer → sign → verify roundtrip.
    #[test]
    fn sign_then_verify_roundtrip_all_networks() {
        for net in [
            bitcoin::Network::Bitcoin,
            bitcoin::Network::Testnet,
            bitcoin::Network::Testnet4,
            bitcoin::Network::Signet,
            bitcoin::Network::Regtest,
        ] {
            let (signer, address) = derive_first_external_signer(TEST_MNEMONIC, net)
                .unwrap_or_else(|e| panic!("derive failed for {net:?}: {e:?}"));
            let sig = sign_message("hello", &signer, &address)
                .unwrap_or_else(|e| panic!("sign failed for {net:?}: {e:?}"));
            let valid = verify_message(&address, &sig, "hello")
                .unwrap_or_else(|e| panic!("verify failed for {net:?}: {e:?}"));
            assert!(valid, "sign+verify roundtrip must return true for {net:?}");
        }
    }

    /// Parametrized tamper-detection across all 5 networks. The same
    /// signature is verified against a different message → must return
    /// `false` for every network (BIP-137 message-binding property).
    #[test]
    fn verify_tampered_message_all_networks() {
        for net in [
            bitcoin::Network::Bitcoin,
            bitcoin::Network::Testnet,
            bitcoin::Network::Testnet4,
            bitcoin::Network::Signet,
            bitcoin::Network::Regtest,
        ] {
            let (signer, address) = derive_first_external_signer(TEST_MNEMONIC, net)
                .unwrap_or_else(|e| panic!("derive failed for {net:?}: {e:?}"));
            let sig = sign_message("hello", &signer, &address)
                .unwrap_or_else(|e| panic!("sign failed for {net:?}: {e:?}"));
            let valid = verify_message(&address, &sig, "goodbye")
                .unwrap_or_else(|e| panic!("verify failed for {net:?}: {e:?}"));
            assert!(
                !valid,
                "verify with tampered message must return false for {net:?}"
            );
        }
    }

    #[test]
    fn sign_then_verify_roundtrip() {
        let (signer, address) =
            derive_first_external_signer(TEST_MNEMONIC, bitcoin::Network::Testnet).unwrap();
        let signed = sign_message("hello world", &signer, &address).unwrap();
        let valid = verify_message(&address, &signed, "hello world").unwrap();
        assert!(valid, "sign+verify roundtrip must succeed");
    }

    #[test]
    fn verify_rejects_tampered_message() {
        let (signer, address) =
            derive_first_external_signer(TEST_MNEMONIC, bitcoin::Network::Testnet).unwrap();
        let signed = sign_message("hello world", &signer, &address).unwrap();
        // Verify against a different message — must fail.
        let valid = verify_message(&address, &signed, "goodbye world").unwrap();
        assert!(
            !valid,
            "verifying with a different message must return false"
        );
    }

    #[test]
    fn handle_message_sign_refuses_wrong_address() {
        let err = handle_message_sign(
            TEST_MNEMONIC.to_string(),
            crate::cli::NetArg::Testnet,
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx".to_string(),
            "hello".to_string(),
        )
        .expect_err("wrong address must be rejected");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("does not match") || msg.contains("first-external"),
            "error should mention address mismatch: {msg}"
        );
    }

    #[test]
    fn handle_message_sign_refuses_mismatched_network() {
        let (_, derived) =
            derive_first_external_signer(TEST_MNEMONIC, bitcoin::Network::Testnet).unwrap();
        // Build a mainnet-format address from the derived pubkey (wrong network).
        let (signer2, _) =
            derive_first_external_signer(TEST_MNEMONIC, bitcoin::Network::Bitcoin).unwrap();
        let _ = signer2;
        let mainnet_addr = derived.to_string(); // pretend it's mainnet — just need a valid addressto pass require_network
        let _ = mainnet_addr;
        // Simplest test: pass the testnet address with --network bitcoin → require_network rejects.
        let err = handle_message_sign(
            TEST_MNEMONIC.to_string(),
            crate::cli::NetArg::Bitcoin,
            derived.to_string(),
            "hello".to_string(),
        )
        .expect_err("testnet address under --network bitcoin must be rejected");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("not valid for network"),
            "error should mention network mismatch: {msg}"
        );
    }
}

// ============================================================================
// Issue #62 / Task 54b: `btc encrypt|decrypt` handlers (stateless file ops)
// ============================================================================

/// Roundtrip: encrypt then decrypt recovers the exact plaintext bytes.
/// Pure end-to-end test of the handler pair against the library's
/// MnemonicCipher (F5 Argon2id KDF + F6 AES-256-GCM AEAD).
#[test]
fn encrypt_decrypt_roundtrip_recovers_phrase() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plaintext_path = tmp.path().join("plaintext.txt");
    let blob_path = tmp.path().join("cipher.enc");
    let recovered_path = tmp.path().join("recovered.txt");
    std::fs::write(
        &plaintext_path,
        "abandon abandon abandon abandon abandon abandon about",
    )
    .expect("write plaintext");

    handle_encrypt(
        Some("test-password".to_string()),
        plaintext_path.clone(),
        blob_path.clone(),
    )
    .expect("encrypt");
    handle_decrypt(
        Some("test-password".to_string()),
        blob_path,
        recovered_path.clone(),
    )
    .expect("decrypt");

    let recovered = std::fs::read_to_string(&recovered_path).expect("read recovered");
    assert_eq!(
        recovered,
        "abandon abandon abandon abandon abandon abandon about"
    );
}

/// Wrong password → decrypt must fail with MnemonicCipher error (N2
/// oracle mitigation: error message doesn't reveal "wrong password"
/// vs "tampered" — caller can't probe which).
#[test]
fn decrypt_with_wrong_password_fails() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plaintext_path = tmp.path().join("plaintext.txt");
    let blob_path = tmp.path().join("cipher.enc");
    let recovered_path = tmp.path().join("recovered.txt");
    std::fs::write(&plaintext_path, "secret mnemonic phrase here").expect("write");

    handle_encrypt(
        Some("correct-password".to_string()),
        plaintext_path,
        blob_path.clone(),
    )
    .expect("encrypt");
    let err = handle_decrypt(
        Some("wrong-password".to_string()),
        blob_path,
        recovered_path,
    )
    .expect_err("decrypt must reject wrong password");
    let msg = format!("{err:?}");
    // N2 oracle mitigation: caller cannot probe WHICH failure mode
    // (wrong-password vs tampered vs truncated) — all collapsed to a
    // single "decrypt failed" message.
    assert!(
        msg.contains("decrypt failed"),
        "error should be uniform 'decrypt failed', got: {msg}"
    );
}

/// Tampered blob → decrypt fails (AES-GCM tag mismatch surfaces as
/// MnemonicCipher). Verifies integrity protection actually works.
#[test]
fn decrypt_tampered_blob_fails() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plaintext_path = tmp.path().join("plaintext.txt");
    let blob_path = tmp.path().join("cipher.enc");
    let recovered_path = tmp.path().join("recovered.txt");
    std::fs::write(&plaintext_path, "hello world").expect("write");

    handle_encrypt(
        Some("test-password".to_string()),
        plaintext_path,
        blob_path.clone(),
    )
    .expect("encrypt");
    // Flip last byte (the GCM tag) — must cause AES-GCM verification to fail.
    let mut bytes = std::fs::read(&blob_path).expect("read blob");
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    std::fs::write(&blob_path, bytes).expect("rewrite tampered blob");

    let err = handle_decrypt(Some("test-password".to_string()), blob_path, recovered_path)
        .expect_err("decrypt must reject tampered blob");
    let msg = format!("{err:?}");
    // N2 oracle mitigation: same uniform message as wrong-password.
    assert!(
        msg.contains("decrypt failed"),
        "error should be uniform 'decrypt failed', got: {msg}"
    );
}

/// Tampered blob (first byte / salt region) → decrypt fails with same
/// uniform message. Defense-in-depth — confirms tamper-detection works
/// at the KDF boundary, not just the AEAD tag.
#[test]
fn decrypt_tampered_first_byte_fails() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plaintext_path = tmp.path().join("plaintext.txt");
    let blob_path = tmp.path().join("cipher.enc");
    let recovered_path = tmp.path().join("recovered.txt");
    std::fs::write(&plaintext_path, "hello world").expect("write");

    handle_encrypt(
        Some("test-password".to_string()),
        plaintext_path,
        blob_path.clone(),
    )
    .expect("encrypt");
    let mut bytes = std::fs::read(&blob_path).expect("read blob");
    bytes[0] ^= 0x01; // flip first byte (in salt region)
    std::fs::write(&blob_path, bytes).expect("rewrite tampered blob");

    let err = handle_decrypt(Some("test-password".to_string()), blob_path, recovered_path)
        .expect_err("decrypt must reject salt-tampered blob");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("decrypt failed"),
        "error should be uniform 'decrypt failed', got: {msg}"
    );
}

/// Truncated blob → uniform "decrypt failed" (N2 oracle collapse).
#[test]
fn decrypt_truncated_blob_fails() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let blob_path = tmp.path().join("truncated.enc");
    let recovered_path = tmp.path().join("recovered.txt");
    // MIN_LEN = 44 (SALT 16 + NONCE 12 + TAG 16). Write 10 bytes — well below.
    std::fs::write(&blob_path, vec![0u8; 10]).expect("write truncated");

    let err = handle_decrypt(Some("any-password".to_string()), blob_path, recovered_path)
        .expect_err("decrypt must reject truncated blob");
    let msg = format!("{err:?}");
    // N2 oracle: same uniform message as wrong-password + tampered.
    assert!(
        msg.contains("decrypt failed"),
        "error should be uniform 'decrypt failed', got: {msg}"
    );
}

/// Non-UTF8 plaintext in encrypt → anyhow UTF-8 error before encryption.
/// (MnemonicCipher rejects empty phrase; non-UTF8 doesn't have a direct
/// path because we read as String. This test pins the UTF-8 boundary.)
#[test]
fn encrypt_rejects_non_utf8_plaintext() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plaintext_path = tmp.path().join("binary.bin");
    let blob_path = tmp.path().join("cipher.enc");
    // Invalid UTF-8 byte sequence (0xFF is never valid UTF-8 start byte).
    std::fs::write(&plaintext_path, [0xFF, 0xFE, 0xFD, 0xFC]).expect("write");

    let err = handle_encrypt(
        Some("test-password".to_string()),
        plaintext_path,
        blob_path.clone(),
    )
    .expect_err("encrypt must reject non-UTF8 plaintext");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("UTF-8") || msg.contains("utf-8") || msg.contains("utf8"),
        "error should mention UTF-8 rejection, got: {msg}"
    );
    // Side-effect isolation: encrypt must NOT have written a partial blob.
    // (atomic_write only fires on the success path; UTF-8 rejection happens
    // before any encryption or write — the output path must not exist.)
    // Note: blob_path's parent dir exists; we just verify blob_path is absent.
    assert!(
        !blob_path.exists(),
        "encrypt must not create --out on UTF-8 rejection"
    );
}

/// Empty plaintext → encrypt refuses (defense-in-depth — library also
/// rejects empty phrases at `encrypt_mnemonic`).
#[test]
fn encrypt_rejects_empty_input() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plaintext_path = tmp.path().join("empty.txt");
    let blob_path = tmp.path().join("cipher.enc");
    std::fs::write(&plaintext_path, "").expect("write empty");

    let err = handle_encrypt(Some("test-password".to_string()), plaintext_path, blob_path)
        .expect_err("encrypt must reject empty plaintext");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("empty"),
        "error should mention empty rejection, got: {msg}"
    );
}

/// `--in == --out` → refused (would overwrite source mid-encrypt, leaving
/// truncated plaintext tail in the original file's inode blocks).
#[test]
fn encrypt_refuses_same_in_out() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("overlap.txt");
    std::fs::write(&path, "secret phrase").expect("write");
    let err = handle_encrypt(Some("test-password".to_string()), path.clone(), path)
        .expect_err("encrypt must refuse --in == --out");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("must differ") || msg.contains("refusing to overwrite"),
        "error should explain same-in/out refusal, got: {msg}"
    );
}

/// `--in == --out` for decrypt → same refusal (avoids truncating
/// the input blob mid-decrypt).
#[test]
fn decrypt_refuses_same_in_out() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plaintext_path = tmp.path().join("plaintext.txt");
    let blob_path = tmp.path().join("cipher.enc");
    std::fs::write(&plaintext_path, "secret phrase").expect("write");
    handle_encrypt(
        Some("test-password".to_string()),
        plaintext_path,
        blob_path.clone(),
    )
    .expect("encrypt");

    let err = handle_decrypt(
        Some("test-password".to_string()),
        blob_path.clone(),
        blob_path,
    )
    .expect_err("decrypt must refuse --in == --out");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("must differ"),
        "error should explain same-in/out refusal, got: {msg}"
    );
}

/// Missing input file → io::Error surfaces as anyhow error.
/// (Don't panic on missing file; clean error.)
#[test]
fn encrypt_missing_input_file_fails() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plaintext_path = tmp.path().join("does-not-exist.txt");
    let blob_path = tmp.path().join("cipher.enc");

    let err = handle_encrypt(Some("test-password".to_string()), plaintext_path, blob_path)
        .expect_err("missing input must error");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("No such file") || msg.contains("not found") || msg.contains("os error"),
        "error should be io-flavored, got: {msg}"
    );
}

// ============================================================================
// Issue #63 / Task 54c: `build_esplora_client_for` unit tests
// ============================================================================
//
// These tests cover the F20 (SPKI pin required for non-regtest) +
// F36 (https-only URL) enforcement at the client-construction
// layer. They are pure (no network IO) — we only assert that the
// helper refuses bad inputs BEFORE constructing the client or
// hitting the network. Live testnet coverage lives in
// `tests/cli.rs` (operator-driven, `#[ignore]` per L29).

#[cfg(test)]
mod sync_balance_tests {
    use super::*;
    use std::path::PathBuf;

    fn empty_data_dir() -> PathBuf {
        std::env::temp_dir()
    }

    /// F36: http:// (not https://) must be rejected at URL parse.
    /// `EsploraUrl::new` (consolidated by #36) is the canonical gate.
    #[test]
    fn build_esplora_client_for_rejects_http_url() {
        let pin_hex = "0".repeat(64);
        let err = build_esplora_client_for(
            bitcoin::Network::Bitcoin,
            "http://blockstream.info/api",
            Some(&pin_hex),
            &empty_data_dir(),
        )
        .expect_err("http:// must be rejected at URL parse");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("https") || msg.contains("invalid"),
            "error should mention https/invalid: {msg}"
        );
    }

    /// F20: non-regtest network WITHOUT a SPKI pin must be rejected
    /// BEFORE any network IO (refuse with a clear error). The
    /// `EsploraUrl` parse succeeds (https://), but the missing pin
    /// trips our explicit F20 check.
    #[test]
    fn build_esplora_client_for_requires_spki_for_mainnet() {
        let err = build_esplora_client_for(
            bitcoin::Network::Bitcoin,
            "https://blockstream.info/api",
            None,
            &empty_data_dir(),
        )
        .expect_err("mainnet without --pin-spki must be refused (F20)");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("--pin-spki") && msg.contains("required"),
            "error should explain --pin-spki is required: {msg}"
        );
    }

    #[test]
    fn build_esplora_client_for_requires_spki_for_testnet() {
        let err = build_esplora_client_for(
            bitcoin::Network::Testnet,
            "https://blockstream.info/testnet/api",
            None,
            &empty_data_dir(),
        )
        .expect_err("testnet without --pin-spki must be refused (F20)");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("required"),
            "error should mention required: {msg}"
        );
    }

    #[test]
    fn build_esplora_client_for_requires_spki_for_signet() {
        let err = build_esplora_client_for(
            bitcoin::Network::Signet,
            "https://blockstream.info/signet/api",
            None,
            &empty_data_dir(),
        )
        .expect_err("signet without --pin-spki must be refused (F20)");
        let msg = format!("{err:?}");
        assert!(msg.contains("required"), "error mentions required: {msg}");
    }

    /// F20 regtest exemption: localhost regtest often uses a
    /// self-signed cert + SystemRoots, which is acceptable behind
    /// stunnel. Operator may omit `--pin-spki`.
    #[test]
    fn build_esplora_client_for_accepts_regtest_without_spki() {
        // This test will fail in environments where localhost Esplora
        // is not configured. We only assert that the construction
        // path is reachable (no F20 refusal); the actual TLS handshake
        // is not exercised here.
        let result = build_esplora_client_for(
            bitcoin::Network::Regtest,
            "https://localhost:50002",
            None,
            &empty_data_dir(),
        );
        assert!(
            result.is_ok(),
            "regtest without --pin-spki must be allowed (F20 exemption): {result:?}"
        );
    }

    /// Regtest MAY also pass `--pin-spki` (operator choice). We
    /// only assert the F20 gating is NOT triggered for regtest —
    /// the actual `reqwest` client build depends on runtime TLS
    /// state (CA bundle, platform native-certs) and is exercised
    /// by live testnet smoke per L29, not unit tests.
    #[test]
    fn build_esplora_client_for_regtest_with_spki_skips_f20_refusal() {
        let pin_hex = "0".repeat(64);
        let err = build_esplora_client_for(
            bitcoin::Network::Regtest,
            "https://localhost:50002",
            Some(&pin_hex),
            &empty_data_dir(),
        )
        .err();
        if let Some(e) = err {
            // Must NOT be the F20 "required" refusal — anything else
            // (TLS builder, CA bundle missing) is acceptable for unit.
            let msg = format!("{e:?}");
            assert!(
                !msg.contains("--pin-spki") || !msg.contains("required"),
                "regtest with pin must not trip F20 refusal: {msg}"
            );
        }
    }

    /// Mainnet WITH pin → F20 gate is satisfied; downstream
    /// construction errors (reqwest TLS builder) are runtime-state
    /// issues, not F20 violations. Assert the F20 gate is passed.
    #[test]
    fn build_esplora_client_for_mainnet_with_spki_skips_f20_refusal() {
        let pin_hex = "0".repeat(64);
        let err = build_esplora_client_for(
            bitcoin::Network::Bitcoin,
            "https://blockstream.info/api",
            Some(&pin_hex),
            &empty_data_dir(),
        )
        .err();
        if let Some(e) = err {
            let msg = format!("{e:?}");
            assert!(
                !msg.contains("--pin-spki") || !msg.contains("required"),
                "mainnet with pin must not trip F20 refusal: {msg}"
            );
        }
    }

    /// Invalid SPKI pin (wrong length) → must fail at parse, not
    /// silently fall through to construction.
    #[test]
    fn build_esplora_client_for_rejects_malformed_pin() {
        let err = build_esplora_client_for(
            bitcoin::Network::Bitcoin,
            "https://blockstream.info/api",
            Some("too-short"),
            &empty_data_dir(),
        )
        .expect_err("malformed pin must be rejected");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("64 hex chars"),
            "error should explain 64-char requirement: {msg}"
        );
    }

    /// `handle_wallet_balance` and `handle_wallet_sync` both funnel
    /// through `build_esplora_client_for`. End-to-end F20 check via
    /// the public handler: mainnet + no pin + valid mnemonic must
    /// fail before any wallet construction.
    #[tokio::test]
    async fn handle_wallet_balance_refuses_mainnet_without_pin() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string();
        let err = handle_wallet_balance(
            mnemonic,
            crate::cli::NetArg::Bitcoin,
            "https://blockstream.info/api".to_string(),
            None,
        )
        .await
        .expect_err("mainnet + no pin must refuse");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("--pin-spki") || msg.contains("required"),
            "handler-level F20 refusal expected: {msg}"
        );
    }
}
