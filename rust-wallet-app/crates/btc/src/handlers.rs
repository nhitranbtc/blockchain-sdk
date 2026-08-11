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
use bitcoin_wallet_core::crypto::bip137::{sign_message, verify_message, SignedMessage};
use bitcoin_wallet_core::keys::{AddressType, Mnemonic, Secret, Signer, XPrvHolder};
use bitcoin_wallet_core::wallet::{create_wallet, show_wallet, WalletId, SUPPORTED_WORD_COUNTS};

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
