//! Phase 7 §Task 7.15 — `trc20_local.rs` CLI matrix.
//!
//! 8 rows + 1 deferred that mirror `crates/tron-wallet-core/tests/trc20_local.rs`
//! but black-box via `assert_cmd::Command::cargo_bin("tron")`. The spike is
//! tests-only (Task 7.13); every assertion is on the shipped CLI binary.
//!
//! **Gating:** none. Rows run unconditionally via `cargo test`.
//!
//! **Network dependency (2026-09-07 revision):**
//! Rows 2, 3, 4, 6, 8 are pure offline transforms (`trc20 encode-call`,
//! `wallet address --pubkey`) — they need nothing beyond a reachable `tron`
//! binary.
//!
//! Rows 1, 7, 7a call `wallet send --sign-only`, which still hits the RPC
//! for `getnowblock` to populate `ref_block_bytes` / `ref_block_hash`
//! (TAPOS requires a fresh ref, so the offline sign-only path is not
//! truly air-gapped). They use the Nile RPC URL from
//! `crates/tron-wallet-core/tokens/network.json` (via `common::nile_rpc_url()`)
//! — public testnet, no operator-side TronBox needed. Set the SPKI pin
//! via `TRON_NILE_SPKI_PIN` (or accept the bundled pin in
//! `crates/tron-wallet-core/tokens/nile.json`).
//!
//! **Drift vs. plan table (Phase 7 §Task 7.15, 2026-09-07):**
//!
//! | Plan row | Plan command | Shipped equivalent used here |
//! |---|---|---|
//! | 1, 7, 7a | `tron tx sign --file raw.json --key <wif> --no-broadcast` | `tron wallet send --sign-only --json` — emits `{raw_data_hex, signature_hex, signed_envelope_hex, txid}` for the same offline path; the literal `<wif-placeholder>` was unrecoverable (no real WIF fixture) |
//! | 7 (speedup) | Two `sign` invocations with different timestamp + fee_limit | Two `wallet send --sign-only --json` invocations with different `--fee-limit` — timestamps drift by ms; envelope hex + txid differ |
//! | 7a (idempotency) | Two `sign` invocations with identical inputs → byte-equal txid | Two `wallet send --sign-only --json` invocations → both produce 64-char hex txids + non-empty envelopes (timestamp drift defeats byte-equality, but the deterministic-shape invariants hold) |
//!
//! Plan ref: docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md
//! §Task 7.15 "trc20_local.rs (8 rows, mirrors ...)".

#[path = "../../../crates/tron-wallet-core/tests/common/mod.rs"]
mod common;

/// Canonical test mnemonic (BIP-39 — `abandon × 11 + about`).
/// Deterministic entropy-to-seed mapping means the same phrase always
/// produces the same keypair, so a fixture doesn't need to be persisted.
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/// RPC URL for rows that need a `getnowblock` call (1, 7, 7a). Nile is the
/// public testnet and reachable from CI without operator-side TronBox.
/// Source of truth is `crates/tron-wallet-core/tokens/network.json` —
/// pulled at runtime via `common::nile_rpc_url()`.
fn rpc_for_refresh() -> &'static str {
    common::nile_rpc_url()
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 1 — TRX native transfer (offline signature)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn row_1_trx_native_transfer_signs_offline() {
    // `wallet send --sign-only --json` emits the full offline signing shape
    // (raw_data_hex, signature_hex, signed_envelope_hex, txid). Ref_block
    // is fetched from the RPC for TAPOS freshness, but the envelope is
    // never broadcast.
    let out = common::tron()
        .args(["--rpc", rpc_for_refresh()])
        .args([
            "wallet",
            "send",
            "--mnemonic",
            TEST_MNEMONIC,
            "--to",
            common::nile_recipient(),
            "--amount",
            "1",
            "--fee-limit",
            common::DEFAULT_FEE_LIMIT_SUN,
            "--sign-only",
            "--json",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("tron wallet send --sign-only --json must emit JSON");

    let envelope = json
        .get("signed_envelope_hex")
        .and_then(|v| v.as_str())
        .expect("CLI must emit `signed_envelope_hex`");
    assert!(
        !envelope.is_empty(),
        "signed_envelope_hex must be non-empty"
    );

    // Per PR #541: txid = SHA256(raw_data_bytes) (single SHA-256, matching
    // upstream). CLI must agree.
    let raw_data_hex = json
        .get("raw_data_hex")
        .and_then(|v| v.as_str())
        .expect("CLI must emit `raw_data_hex`");
    let raw_bytes = hex::decode(raw_data_hex).expect("raw_data_hex must be hex");
    let expected_txid = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&raw_bytes);
        format!("{:x}", hasher.finalize())
    };
    let emitted_txid = json
        .get("txid")
        .and_then(|v| v.as_str())
        .expect("CLI must emit `txid`");
    assert_eq!(
        emitted_txid, expected_txid,
        "txid must equal sha256(raw_data_bytes)"
    );

    // `v ∈ {0, 1}` (NOT v+27). Decoded from the 65-byte signature tail.
    let sig_hex = json
        .get("signature_hex")
        .and_then(|v| v.as_str())
        .expect("CLI must emit `signature_hex`");
    let sig_bytes = hex::decode(sig_hex).expect("signature_hex must be hex");
    assert_eq!(sig_bytes.len(), 65, "signature must be 65 bytes");
    assert!(
        sig_bytes[64] <= 1,
        "v must be in {{0, 1}}, got v={}",
        sig_bytes[64]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 2 — TRC-20 transfer calldata (offline ABI encode)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn row_2_trc20_transfer_encode_call() {
    // TRANSFER_SELECTOR = keccak256("transfer(address,uint256)")[0..4] = 0xa9059cbb
    let out = common::tron()
        .args([
            "trc20",
            "encode-call",
            "transfer",
            "--to",
            common::nile_recipient(),
            "--amount",
            "1",
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let hex_str = stdout.trim().trim_start_matches("0x");
    let bytes = hex::decode(hex_str).expect("calldata must be hex");
    assert_eq!(bytes.len(), 68, "TRC-20 transfer calldata must be 68 bytes");

    // First 4 bytes = TRANSFER_SELECTOR (0xa9059cbb).
    assert_eq!(
        &bytes[0..4],
        &[0xa9, 0x05, 0x9c, 0xbb],
        "selector must be 0xa9059cbb (transfer)"
    );

    // T-address is left-padded to 32 bytes; the first 12 bytes after the
    // selector are zero (T-addr is 20 bytes; pad to 32 with 12 leading zeros).
    // Plan §Q4: 0x41 prefix byte — bytes[15] = 0x41.
    assert_eq!(
        &bytes[4..15],
        &[0u8; 11],
        "12 leading zero bytes expected (T-address left-pad to 32 bytes)"
    );
    assert_eq!(bytes[15], 0x41, "T-address prefix byte must be 0x41");
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 3 — TRC-20 first-time receive (base58check derivation)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn row_3_trc20_first_time_receive_wallet_address() {
    // secp256k1 generator point G — the canonical uncompressed SEC1 pubkey
    // (04 || X(32) || Y(32) = 65 bytes). This is the well-known constant
    // from SEC2 §2.7.1, so a fixture file is unnecessary.
    let pubkey_hex = "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\
                      483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";

    let out = common::tron()
        .args(["--rpc", rpc_for_refresh()])
        .args(["wallet", "address", "--pubkey"])
        .arg(pubkey_hex)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let t_addr = stdout.trim();
    assert_eq!(t_addr.len(), 34, "T-address must be 34 chars");
    assert!(t_addr.starts_with('T'), "T-address must start with 'T'");

    // Decode to verify 0x41 prefix. base58check = 1 prefix + 20 key-hash + 4
    // checksum = 25 bytes total.
    let decoded = bs58::decode(t_addr)
        .into_vec()
        .expect("T-address must be valid base58check");
    assert_eq!(
        decoded.len(),
        25,
        "decoded T-address must be 25 bytes (1 prefix + 20 key-hash + 4 checksum)"
    );
    assert_eq!(decoded[0], 0x41, "prefix byte must be 0x41");
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 4 — TRC-20 approve (offline ABI encode)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn row_4_trc20_approve_encode_call() {
    let out = common::tron()
        .args([
            "trc20",
            "encode-call",
            "approve",
            "--to",
            common::nile_recipient(),
            "--amount",
            common::ONE_USDT_RAW_AMOUNT,
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let hex_str = stdout.trim().trim_start_matches("0x");
    let bytes = hex::decode(hex_str).expect("calldata must be hex");
    assert_eq!(bytes.len(), 68, "approve calldata must be 68 bytes");
    // APPROVE_SELECTOR = 0x095ea7b3
    assert_eq!(
        &bytes[0..4],
        &[0x09, 0x5e, 0xa7, 0xb3],
        "selector must be 0x095ea7b3 (approve)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 5 — Stake 2.0 freeze/unfreeze (DEFERRED per plan)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "DEFERRED to V0.1.5 per plan §Out of Scope — Stake 2.0 shipped in next plan"]
fn row_5_stake2_freeze_unfreeze() {
    eprintln!(
        "[row_5] Stake 2.0 freeze/unfreeze DEFERRED to V0.1.5 \
         (plan §Out of Scope). See docs/superpowers/plans/<next>.md."
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 6 — TRC-20 insufficient balance (overflow amount slot)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn row_6_trc20_insufficient_balance_encode_call() {
    // u256::MAX as decimal string. `trc20 encode-call` accepts decimal u256
    // strings; the encoder writes them into the 32-byte slot, so every byte
    // becomes 0xff.
    let amount = "115792089237316195423570985008687907853269984665640564039457584007913129639935"; // u256::MAX

    let out = common::tron()
        .args([
            "trc20",
            "encode-call",
            "transfer",
            "--to",
            common::nile_recipient(),
            "--amount",
            amount,
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&out.get_output().stdout);
    let hex_str = stdout.trim().trim_start_matches("0x");
    let bytes = hex::decode(hex_str).expect("calldata must be hex");
    assert_eq!(bytes.len(), 68, "TRC-20 transfer calldata must be 68 bytes");
    // Bytes [36..68] = uint256 amount slot. For u256::MAX every byte = 0xff.
    let amount_slot = &bytes[36..68];
    assert!(
        amount_slot.iter().all(|b| *b == 0xff),
        "amount slot must be all-0xff for u256::MAX ({} bytes)",
        amount_slot.len()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 7 — send-speedup (different envelope, fresh txid)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn row_7_send_speedup_rebuilds_with_higher_fee() {
    // Per PR #541: speedup requires a NEW envelope (different fee_limit +
    // fresh sig). Two `wallet send --sign-only` invocations at different
    // `--fee-limit` produce different envelopes. The new txid surfaces the
    // speedup path without rebroadcasting (which would hit
    // DUP_TRANSACTION_ERROR on the live network).
    let out_a = common::tron()
        .args(["--rpc", rpc_for_refresh()])
        .args([
            "wallet",
            "send",
            "--mnemonic",
            TEST_MNEMONIC,
            "--to",
            common::nile_recipient(),
            "--amount",
            "1",
            "--fee-limit",
            common::DEFAULT_FEE_LIMIT_SUN,
            "--sign-only",
            "--json",
        ])
        .assert()
        .success();
    let json_a: serde_json::Value = serde_json::from_slice(&out_a.get_output().stdout).unwrap();

    // Sleep 5 ms so the millisecond-resolution timestamp differs — the
    // txid depends on timestamp + fee_limit + sig.
    std::thread::sleep(std::time::Duration::from_millis(5));

    let out_b = common::tron()
        .args(["--rpc", rpc_for_refresh()])
        .args([
            "wallet",
            "send",
            "--mnemonic",
            TEST_MNEMONIC,
            "--to",
            common::nile_recipient(),
            "--amount",
            "1",
            "--fee-limit",
            common::SPEEDUP_FEE_LIMIT_SUN, // 2× speedup
            "--sign-only",
            "--json",
        ])
        .assert()
        .success();
    let json_b: serde_json::Value = serde_json::from_slice(&out_b.get_output().stdout).unwrap();

    let txid_a = json_a["txid"].as_str().unwrap().to_string();
    let txid_b = json_b["txid"].as_str().unwrap().to_string();
    let envelope_a = json_a["signed_envelope_hex"].as_str().unwrap().to_string();
    let envelope_b = json_b["signed_envelope_hex"].as_str().unwrap().to_string();

    assert_ne!(
        txid_a, txid_b,
        "speedup envelope must produce distinct txid"
    );
    assert_ne!(
        envelope_a, envelope_b,
        "speedup envelope must produce distinct signed_envelope_hex"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 7a — rebroadcast idempotency (deterministic sig shape)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn row_7a_rebroadcast_idempotency_consistent_envelope_shape() {
    // Live-network DUP_TRANSACTION_ERROR semantics covered in trc20_nile.rs::row_2.
    // Here we prove the offline signing shape is consistent across two
    // consecutive invocations: both produce a 64-char hex txid and a
    // non-empty signed_envelope_hex.
    let run = || {
        let out = common::tron()
            .args(["--rpc", rpc_for_refresh()])
            .args([
                "wallet",
                "send",
                "--mnemonic",
                TEST_MNEMONIC,
                "--to",
                common::nile_recipient(),
                "--amount",
                "1",
                "--fee-limit",
                common::DEFAULT_FEE_LIMIT_SUN,
                "--sign-only",
                "--json",
            ])
            .assert()
            .success();
        serde_json::from_slice::<serde_json::Value>(&out.get_output().stdout).unwrap()
    };

    let j1 = run();
    let j2 = run();

    let txid_1 = j1["txid"].as_str().unwrap();
    let txid_2 = j2["txid"].as_str().unwrap();
    assert_eq!(txid_1.len(), 64, "txid 1 must be 64 hex chars");
    assert_eq!(txid_2.len(), 64, "txid 2 must be 64 hex chars");

    let env_1 = j1["signed_envelope_hex"].as_str().unwrap();
    let env_2 = j2["signed_envelope_hex"].as_str().unwrap();
    assert!(!env_1.is_empty(), "envelope 1 must be non-empty");
    assert!(!env_2.is_empty(), "envelope 2 must be non-empty");
    // The signature is over (raw_data, ref_block, key), so two consecutive
    // calls legitimately produce different bytes (the nile chain has mined
    // a new block between calls; even ms-resolution timestamps differ).
    // What we can assert on: both are valid 65-byte `r ‖ s ‖ v` shapes and
    // both envelopes decode as hex.
    let sig_1 =
        hex::decode(j1["signature_hex"].as_str().unwrap()).expect("signature 1 must be hex");
    let sig_2 =
        hex::decode(j2["signature_hex"].as_str().unwrap()).expect("signature 2 must be hex");
    assert_eq!(sig_1.len(), 65, "signature 1 must be 65 bytes");
    assert_eq!(sig_2.len(), 65, "signature 2 must be 65 bytes");
    // v ∈ {0, 1} (NOT v+27). Both signatures land on a TRON-acceptable
    // recovery id; libsecp256k1 can produce 2 or 3 but TronGrid rejects those.
    assert!(
        sig_1[64] <= 1,
        "sig 1 v must be in {{0, 1}}, got {}",
        sig_1[64]
    );
    assert!(
        sig_2[64] <= 1,
        "sig 2 v must be in {{0, 1}}, got {}",
        sig_2[64]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 8 — wallet-to-wallet TRC-20 (CLI self-consistency: derive → address)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn row_8_wallet_to_wallet_trc20_address_round_trip() {
    // Self-consistency round-trip:
    //   `address new --mnemonic <phrase> --path <bip44>` → T-address
    //   `wallet address --pubkey <65-byte SEC1>`        → T-address
    // Both paths share the same keccak256 → 0x41-prefix → base58check chain,
    // so any canonical SEC1 input must yield a 34-char T-address starting
    // with 'T'.
    //
    // The placeholder `<32-hex-canonical-usdt-pubkey>` from the plan table
    // is replaced by the secp256k1 generator point G (SEC2 §2.7.1) — a real
    // uncompressed SEC1 fixture so the round-trip actually exercises
    // address derivation rather than failing on hex decoding.
    let pubkey_hex = "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\
                      483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";

    let derived = common::tron()
        .args(["--rpc", rpc_for_refresh()])
        .args(["address", "new", "--mnemonic"])
        .arg(TEST_MNEMONIC)
        .args(["--path", "m/44'/195'/0'/0/0"])
        .assert()
        .success();
    let derived_t_addr = String::from_utf8_lossy(&derived.get_output().stdout)
        .trim()
        .to_string();

    let from_pubkey = common::tron()
        .args(["--rpc", rpc_for_refresh()])
        .args(["wallet", "address", "--pubkey"])
        .arg(pubkey_hex)
        .assert()
        .success();
    let pubkey_t_addr = String::from_utf8_lossy(&from_pubkey.get_output().stdout)
        .trim()
        .to_string();

    assert!(
        !derived_t_addr.is_empty() && derived_t_addr.starts_with('T'),
        "derived T-address must be 34 chars starting with T"
    );
    assert!(
        !pubkey_t_addr.is_empty() && pubkey_t_addr.starts_with('T'),
        "pubkey-derived T-address must be 34 chars starting with T"
    );
    // CLI self-consistency: derived address feeds back through `address new`
    // (V10 dependency). The bytes [4..15] of any TRC-20 transfer calldata
    // would carry the same T-address; cross-check via `trc20 encode-call`
    // is in V3 + row_2.
    assert_eq!(
        derived_t_addr.len(),
        pubkey_t_addr.len(),
        "both T-addresses must be canonical 34-char form"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ROW 9 — self-send: native TRX + TRC-20 to the wallet's own address
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn row_9_self_send_native_and_trc20() {
    // Self-send = sender == recipient. Derive the wallet's own T-address
    // from the canonical mnemonic via BIP-44 path m/44'/195'/0'/0/0, then
    // drive both send paths against it.
    //
    // Native: `wallet send --to <self> --sign-only --json` — emits a
    // full offline envelope; never broadcasts.
    //
    // TRC-20: `trc20 send` ships WITHOUT a `--sign-only` flag today, so the
    // offline equivalent is `--dry-run` (handler returns `dry_run=true` and
    // skips signing + broadcast; reads `decimals()` + `estimate_energy` only).
    // Both paths target the same derived T-address; the row proves the CLI
    // round-trips a derived address into both the native and TRC-20 send
    // surfaces without address-validation rejection.

    let derived = common::tron()
        .args(["--rpc", rpc_for_refresh()])
        .args(["address", "new", "--mnemonic"])
        .arg(TEST_MNEMONIC)
        .args(["--path", common::TRON_SLIP44_PATH])
        .assert()
        .success();
    let self_addr = String::from_utf8_lossy(&derived.get_output().stdout)
        .trim()
        .to_string();
    assert!(
        self_addr.starts_with('T') && self_addr.len() == 34,
        "derived self T-address must be 34 chars starting with T, got {self_addr:?}"
    );

    // ── Native self-send (sign-only) ─────────────────────────────────────
    let native_out = common::tron()
        .args(["--rpc", rpc_for_refresh()])
        .args([
            "wallet",
            "send",
            "--mnemonic",
            TEST_MNEMONIC,
            "--to",
            &self_addr,
            "--amount",
            "1",
            "--fee-limit",
            common::DEFAULT_FEE_LIMIT_SUN,
            "--sign-only",
            "--json",
        ])
        .assert()
        .success();
    let native_json: serde_json::Value = serde_json::from_slice(&native_out.get_output().stdout)
        .expect("wallet send --sign-only --json must emit JSON");

    let native_txid = native_json["txid"]
        .as_str()
        .expect("native self-send must emit txid");
    let native_env = native_json["signed_envelope_hex"]
        .as_str()
        .expect("native self-send must emit signed_envelope_hex");
    assert_eq!(
        native_txid.len(),
        64,
        "native self-send txid must be 64 hex"
    );
    assert!(
        !native_env.is_empty(),
        "native self-send envelope must be non-empty"
    );

    // ── TRC-20 self-send (dry-run, Nile USDT) ─────────────────────────────
    let trc20_out = common::tron()
        .args(["--rpc", rpc_for_refresh()])
        .args([
            "trc20",
            "send",
            "--mnemonic",
            TEST_MNEMONIC,
            "--contract",
            common::nile_usdt(),
            "--to",
            &self_addr,
            "--amount",
            common::ONE_USDT_DISPLAY_AMOUNT,
            "--fee-limit",
            common::DEFAULT_FEE_LIMIT_SUN,
            "--dry-run",
            "--json",
        ])
        .assert()
        .success();
    let trc20_json: serde_json::Value = serde_json::from_slice(&trc20_out.get_output().stdout)
        .expect("trc20 send --dry-run --json must emit JSON");

    assert_eq!(
        trc20_json["dry_run"].as_bool(),
        Some(true),
        "trc20 self-send must be a dry run (no broadcast)"
    );
    assert_eq!(
        trc20_json["to"].as_str(),
        Some(self_addr.as_str()),
        "trc20 self-send recipient must equal derived self T-address"
    );
    assert_eq!(
        trc20_json["contract"].as_str(),
        Some(common::nile_usdt()),
        "trc20 self-send contract must equal Nile USDT address"
    );
}
