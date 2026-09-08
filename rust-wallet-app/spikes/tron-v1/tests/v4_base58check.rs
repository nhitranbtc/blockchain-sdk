//! V4 — base58check T-prefix address (CLI-driven, Phase 7 §Task 7.4).
//!
//! Plan §V4: a TRON T-address is `base58check(0x41 || keccak256(pubkey_uncompressed[1..])[..20])`
//! → 34-char base58 string starting with `T`. The shipped CLI derives this
//! via `tron address new --mnemonic <phrase>` (handler:
//! `crates/tron/src/handlers/address.rs::new`); we assert the emitted
//! T-address against offline base58check invariants without pulling in
//! `tron_wallet_core` (Phase 7 spike invariant: tests-only, no library).
//!
//! Offline checks used here:
//! - bs58 alphabet (no `0`/`O`/`I`/`l`).
//! - Length 34 chars after encoding.
//! - Decoded payload = 21 bytes (1 prefix + 20 account).
//! - Payload[0] == 0x41 (TRON mainnet prefix).
//! - Last 4 decoded bytes = sha256d(payload[..17])[..4] (base58check checksum).
//!
//! KAT parity: `tokens/nile.json` pairs `sender-tr20.mnemonic` with
//! `sender-tr20.address`. Re-derive from the mnemonic via the CLI and
//! assert byte-equality with the fixture address. Drift in either field
//! without updating the other fails LOUDLY here, not silently later.
//!
//! Drift detector (BLOCKING-conditional): `tron address --help` must list
//! the `new` + `xpub` subcommands. If a future refactor drops one, this
//! test fails and forces the spike to track the new surface.

#[path = "../../../crates/tron-wallet-core/tests/common/mod.rs"]
mod common;

use sha2::Digest as _;

/// Drive `tron address new --mnemonic <phrase> --json`, return the emitted
/// T-address from stdout. Uses `--json` so the output is a deterministic
/// `{"address":"T...","path":"m/44'/195'/0'/0/0"}` JSON object rather than
/// the bare address — future flags (e.g. adding an xpub field) cannot
/// change the field that holds the address without breaking the JSON
/// key, which keeps this helper stable.
fn derive_address_via_cli(mnemonic: &str) -> String {
    let assert = common::tron()
        .args(["address", "new", "--mnemonic", mnemonic, "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let v: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("tron address new --json must emit JSON");
    v.get("address")
        .and_then(|a| a.as_str())
        .expect("`tron address new --json` must include an `address` string field")
        .to_string()
}

/// Offline base58check decode. Returns the 21-byte payload
/// (`0x41 || keccak256(pubkey)[..20]`) or a string describing why the
/// candidate is invalid. Mirrors what `tron_wallet_core::address::Address::from_str`
/// does internally (bs58 decode + sha256d checksum verify), but without
/// importing the library — Phase 7 spike invariant.
fn decode_base58check(candidate: &str) -> Result<Vec<u8>, String> {
    let decoded = bs58::decode(candidate)
        .into_vec()
        .map_err(|e| format!("bs58 decode failed: {e}"))?;
    if decoded.len() < 5 {
        return Err(format!(
            "decoded length {} below base58check minimum (4 checksum + 1 payload)",
            decoded.len()
        ));
    }
    let (payload, checksum) = decoded.split_at(decoded.len() - 4);
    let first = sha2::Sha256::digest(payload);
    let second = sha2::Sha256::digest(first);
    if &second[..4] != checksum {
        return Err(format!(
            "checksum mismatch: got {}, expected {}",
            hex::encode(checksum),
            hex::encode(&second[..4])
        ));
    }
    Ok(payload.to_vec())
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Shape invariants — length, alphabet, prefix.
// ─────────────────────────────────────────────────────────────────────────────

/// The CLI emits a 34-character T-address starting with `T`. Length is
/// deterministic: base58check(21-byte payload) → 34 chars regardless of
/// which 20-byte account is encoded.
#[test]
fn v4_tron_address_new_emits_34_char_t_string() {
    let mnemonic = common::nile_sender_mnemonic();
    let addr = derive_address_via_cli(&mnemonic);
    assert!(
        addr.starts_with('T'),
        "TRON address must start with `T` (0x41 mainnet prefix → base58 leading `T`); got {addr:?}"
    );
    assert_eq!(
        addr.len(),
        34,
        "TRON T-address must be exactly 34 chars (base58check over 21-byte payload); got {} ({addr:?})",
        addr.len()
    );
    // Alphabet sanity: `bs58::decode` below rejects non-bs58 chars, so we
    // rely on the roundtrip decoder in the next test to surface alphabet
    // violations. Re-asserting here would duplicate work and require
    // reaching into private Alphabet fields — skip.
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Offline payload decode — 21 bytes, prefix 0x41.
// ─────────────────────────────────────────────────────────────────────────────

/// Decoding the CLI-emitted address offline yields a 21-byte payload
/// whose first byte is `0x41` (TRON mainnet prefix; universal across
/// mainnet / Shasta / Nile per Plan §Q4). A future CLI change that
/// emitted a non-standard prefix (e.g. ETH's `0x`) trips this.
#[test]
fn v4_tron_address_decodes_to_21_byte_prefix_0x41() {
    let mnemonic = common::nile_sender_mnemonic();
    let addr = derive_address_via_cli(&mnemonic);
    let payload = decode_base58check(&addr)
        .unwrap_or_else(|e| panic!("CLI-emitted address {addr:?} must base58check-decode: {e}"));
    assert_eq!(
        payload.len(),
        21,
        "base58check payload must be 21 bytes (1 prefix + 20 account); got {}",
        payload.len()
    );
    assert_eq!(
        payload[0], 0x41,
        "TRON mainnet prefix byte must be 0x41 (Plan §Q4); got 0x{:02x}",
        payload[0]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Bad-checksum rejection — tamper one character and the offline
//    decoder rejects. Proves the checksum is actually being checked,
//    not just stripped.
// ─────────────────────────────────────────────────────────────────────────────

/// Flipping the last base58 character breaks the sha256d checksum; the
/// offline decoder must reject (proves the checksum is verified, not
/// silently ignored). Catches a regression where a future CLI emits
/// plain base58 (no checksum) and the spike's offline helper starts
/// passing everything through.
#[test]
fn v4_tron_address_bad_checksum_rejected_offline() {
    let mnemonic = common::nile_sender_mnemonic();
    let addr = derive_address_via_cli(&mnemonic);
    let mut tampered = addr.clone();
    let last = tampered.pop().expect("address is non-empty");
    let replacement = if last == 'A' { 'B' } else { 'A' };
    tampered.push(replacement);
    assert_ne!(
        tampered, addr,
        "tamper must actually change the string; got identical {tampered:?}"
    );
    let result = decode_base58check(&tampered);
    assert!(
        result.is_err(),
        "tampered address {tampered:?} must fail base58check; got Ok({:?})",
        result.ok()
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("checksum"),
        "failure reason must mention the checksum (not bs58 alphabet); got {err:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. KAT parity — fixture address must equal CLI-derived address.
// ─────────────────────────────────────────────────────────────────────────────

/// `tokens/nile.json` pairs `test.sender-tr20.mnemonic` with
/// `test.sender-tr20.address`. Re-derive from the mnemonic via the CLI
/// and assert byte-equality with the fixture address. If the fixture
/// mnemonic has been regenerated without updating its paired address
/// (or vice versa), this fails LOUDLY here rather than masking the
/// drift in a later trc20_nile row that depends on both.
#[test]
fn v4_kat_parity_nile_sender_address_matches_cli() {
    let mnemonic = common::nile_sender_mnemonic();
    let cli_addr = derive_address_via_cli(&mnemonic);
    let fixture_addr = common::nile_owner();

    // Sanity: the fixture must already be a valid T-address shape, or
    // we cannot tell whether a divergence is a CLI bug or a fixture bug.
    assert!(
        fixture_addr.starts_with('T') && fixture_addr.len() == 34,
        "fixture address {fixture_addr:?} is not a valid T-address shape — \
         refresh crates/tron-wallet-core/tokens/nile.json test.sender-tr20.address"
    );

    assert_eq!(
        cli_addr, fixture_addr,
        "CLI-derived address for `test.sender-tr20.mnemonic` diverges from \
         the paired fixture address `test.sender-tr20.address`. The mnemonic \
         and the fixture address MUST move together; pick one source of \
         truth and re-run `cargo run --example gen_nile_wallet` (see \
         `tokens/nile.json._note`). cli={cli_addr:?} fixture={fixture_addr:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Determinism — same mnemonic, same address, every invocation.
// ─────────────────────────────────────────────────────────────────────────────

/// Deriving the address twice from the same mnemonic via the CLI must
/// yield byte-equal output. BIP-44 derivation is deterministic by
/// construction; a regression that injects randomness (e.g. picking a
/// different `bip39_passphrase` default, or skipping a hash step) trips
/// this. Cross-run determinism is also a prerequisite for the KAT test
/// above — if determinism were off, the KAT would be a one-shot flake.
#[test]
fn v4_tron_address_new_is_deterministic_for_same_mnemonic() {
    let mnemonic = common::nile_sender_mnemonic();
    let first = derive_address_via_cli(&mnemonic);
    let second = derive_address_via_cli(&mnemonic);
    assert_eq!(
        first, second,
        "same mnemonic must derive the same address on repeated CLI invocations; \
         got {first:?} vs {second:?} — BIP-44 derivation is deterministic"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Drift detector — `tron address --help` lists `new` + `xpub`.
// ─────────────────────────────────────────────────────────────────────────────

/// Anchors the shipped CLI surface: `tron address --help` exposes `new`
/// (the subcommand every other V4 test drives) and `xpub` (sister
/// subcommand per Plan §Phase 6 Task 5.3). If a future refactor renames
/// or removes either, this test fails and forces the spike author to
/// update the test bodies above — silent skip is the worst outcome for
/// a drift detector.
///
/// The check is anchored on the clap subcommand banner (first
/// whitespace-delimited token of each `Commands:` line), not raw
/// substring search — `new`/`xpub` could legitimately appear in
/// `--help` boilerplate (env-var descriptions, footer examples) and
/// trip a naive scan.
#[test]
fn v4_tron_address_help_lists_new_and_xpub() {
    let assert = common::tron()
        .args(["address", "--help"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_lowercase();

    let lines: Vec<&str> = stdout.lines().collect();
    let banner_start = lines
        .iter()
        .position(|l| l.trim_start().starts_with("commands:"))
        .unwrap_or(0);
    let banner = &lines[banner_start..];
    let first_tokens: Vec<&str> = banner
        .iter()
        .filter_map(|l| l.split_whitespace().next())
        .collect();

    assert!(
        first_tokens.contains(&"new"),
        "shipped `tron address --help` must expose a `new` subcommand (every V4 \
         test drives it); banner first-tokens: {first_tokens:?}\nhelp:\n{stdout}"
    );
    assert!(
        first_tokens.contains(&"xpub"),
        "shipped `tron address --help` must expose an `xpub` subcommand (Plan §Phase 6 \
         Task 5.3); banner first-tokens: {first_tokens:?}\nhelp:\n{stdout}"
    );
}
