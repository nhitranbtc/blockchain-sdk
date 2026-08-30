//! EIP-191 + EIP-712 signing handlers — Issue #426 / Batch D.
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §5.10 + §6.4. Batch D (TDD): `assert_polygon_chain_id` (Q7 critical-tier
//! gate against cross-chain replay on EIP-712 typed-data signing).
//!
//! L13 Round 1 review fix #2: thin CLI wrappers around `PolygonChain`
//! inherent methods (`from_chain_id`, `is_polygon_chain_id`) — single
//! source of truth in the enum kills the zkEVM forward-compat trap
//! when `PolygonChain::ZkEvm` lands in v0.2.

use polygon_wallet_core::{Error, PolygonChain};

/// Q7 + C1 enforcement: EIP-712 `chain_id` must be a Polygon PoS chain
/// (137 = mainnet, 80002 = amoy). Single chokepoint for cross-chain
/// replay protection on EIP-712 typed-data signing. Both `sign_typed_data`
/// (explicit arg) and any future EIP-712 path (Permit2, route handlers,
/// etc.) call this before signing.
///
/// Delegates to `PolygonChain::is_polygon_chain_id` — adding a new
/// variant (e.g. `PolygonChain::ZkEvm` for v0.2 per design doc §9)
/// automatically extends acceptance without a CLI-side change.
///
/// Returns `Error::InvalidInput` for non-Polygon-PoS chain_ids.
#[allow(dead_code)] // wired into cli.rs sign-typed command in T6 follow-up
pub fn assert_polygon_chain_id(chain_id: u64) -> polygon_wallet_core::Result<()> {
    if PolygonChain::is_polygon_chain_id(chain_id) {
        Ok(())
    } else {
        Err(Error::InvalidInput(format!(
            "EIP-712 chain_id {chain_id} is not a polygon PoS chain (expected 137|80002)"
        )))
    }
}

/// Resolve a `--chain-id` u64 to a `PolygonChain` enum variant. Thin
/// CLI wrapper around `PolygonChain::from_chain_id` — same single
/// source of truth. Returns `Error::InvalidInput` for unknown
/// chain_ids.
#[allow(dead_code)] // wired into cli.rs sign-typed command in T6 follow-up
pub fn polygon_chain_from_id(chain_id: u64) -> polygon_wallet_core::Result<PolygonChain> {
    PolygonChain::from_chain_id(chain_id)
        .ok_or_else(|| Error::InvalidInput(format!("unknown polygon chain_id {chain_id}")))
}

/// T6d-3 handler (Issue #426 / Batch D-2): EIP-191 personal_sign.
///
/// Per design doc §5.10. Thin CLI wrapper around
/// `polygon_wallet_core::sign_message` (live at
/// `evm-wallet-core/src/signer.rs:149`). The CLI layer adds:
///   - chain_id validation deferred (EIP-191 carries no chain_id)
///   - optional `--verify` round-trip (sig must recover to `verify_address`)
///   - Zeroizing wrap on the message buffer at the call site
///
/// Returns the 65-byte signature as `0x`-prefixed hex on stdout
/// (one-line text mode); `--json` returns `{"signature": "0x..."}`
/// (flag honored at the `main.rs` dispatch layer).
///
/// **Dispatch wiring deferred to T6 follow-up PR** — the CLI
/// dispatcher needs `WalletManager::unlock` to derive the
/// `PrivateKeySigner`. The handler is fully implemented + tested at
/// the unit level (`tests::*` below); wiring is a 5-line dispatcher
/// addition once the unlock helper lands. Until then the
/// `#[allow(dead_code)]` suppresses the clippy warning honestly.
#[allow(dead_code)] // wired in main.rs dispatch in T6 follow-up PR
pub fn sign_message(
    signer: &alloy_signer_local::PrivateKeySigner,
    message: &[u8],
    verify_address: Option<alloy_primitives::Address>,
) -> polygon_wallet_core::Result<String> {
    use evm_wallet_core::sign_message as core_sign_message;
    use evm_wallet_core::SignError;
    let sig = core_sign_message(signer, message).map_err(|e| match e {
        SignError::InvalidAddress(s) | SignError::InvalidRequest(s) => {
            Error::InvalidInput(format!("eip191: {s}"))
        }
        SignError::Sign(s) | SignError::Unsupported(s) => Error::Rpc(format!("eip191: {s}")),
    })?;
    // Optional --verify round-trip: recover signer from sig, must match.
    // EIP-191 prefix: `"\x19Ethereum Signed Message:\n" + len(message) + message`,
    // then keccak256. The signer signs the resulting prehash.
    if let Some(expected) = verify_address {
        let mut prefix = Vec::with_capacity(message.len() + 26);
        prefix.extend_from_slice(b"\x19Ethereum Signed Message:\n");
        prefix.extend_from_slice(message.len().to_string().as_bytes());
        prefix.extend_from_slice(message);
        let prehash = alloy_primitives::keccak256(&prefix);
        let recovered = sig
            .recover_address_from_prehash(&prehash)
            .map_err(|e| Error::Rpc(format!("eip191 verify: recover failed: {e}")))?;
        if recovered != expected {
            return Err(Error::InvalidInput(format!(
                "eip191 verify mismatch: recovered {recovered} != expected {expected}"
            )));
        }
    }
    Ok(format!(
        "0x{}",
        alloy_primitives::hex::encode(sig.as_bytes())
    ))
}

/// T6d-3 handler (Issue #426 / Batch D-2): EIP-712 typed-data sign with
/// chain_id validation (Q7 critical-tier gate).
///
/// Per design doc §5.10. The chain_id gate fires BEFORE the underlying
/// lib call — `assert_polygon_chain_id` rejects any value outside
/// `{137, 80002}` with `Error::InvalidInput` (exit 2). Valid chain_id
/// proceeds to the lib helper.
///
/// **Gate contract (Q7 critical-tier):** the `--chain-id` CLI arg is
/// the gate. When the alloy `eip712` feature lands and the lib can
/// parse `typed_data_json`, the handler MUST additionally:
///   1. parse `typed_data.domain.chainId` from the JSON,
///   2. call `assert_polygon_chain_id(typed.domain.chain_id)`,
///   3. require `typed.domain.chain_id == chain_id` (CLI arg must
///      match the authoritative domain value).
///
/// Per EIP-712 the domain's `chainId` is authoritative; the CLI arg
/// is convenience. Skipping this leaves a cross-chain replay hole
/// when the lib lands — an attacker passes `--chain-id 137` with a
/// payload declaring `domain.chainId=1` (Ethereum mainnet), the gate
/// accepts, the lib signs the Ethereum-domain payload. Tracked in
/// the T6d-3 follow-up PR alongside the eip712 feature gate.
///
/// **Deferral notice (T6d-3 scope):** the underlying
/// `polygon_wallet_core::sign_typed_data` (at
/// `evm-wallet-core/src/signer.rs:164`) is currently stubbed pending the
/// `alloy eip712` feature gate decision (tracked in the lib's doc
/// comment). T6d-3 ships the Q7 gate + call-site wiring; the full
/// EIP-712 crypto lands in a follow-up PR. When the lib returns its
/// `SignError::Unsupported(...)` placeholder, this handler surfaces it
/// as `Error::Rpc` so the operator sees the honest deferral status.
///
/// Q7 + #463 dual-source chainId gate (CLI handler tier, defense-in-depth).
///
/// Validates that `typed_data.domain.chainId`, if present, is a
/// non-negative integer u64 ∈ {137, 80002} (Polygon PoS) AND matches
/// `cli_chain_id`. Returns `Ok(())` (fallthrough to CLI gate) when:
///
/// - `domain` field absent entirely — CLI gate is authoritative
/// - `domain` is non-object (`null` / string / number / array) —
///   `Value::get` cannot extract a sub-key, so chainId sub-gate is
///   silently skipped; CLI gate still applies
/// - `domain.chainId` absent — loosest documented behavior per #463
///
/// Strict integer decode: `serde_json::Value::as_u64()` rejects strings,
/// floats (incl. `137.0`), negatives, null, arrays, bools, and nested
/// objects. EIP-712 §7 specifies `chainId` as `uint256` — float-encoded
/// values from non-strict encoders (some ethers.js / viem paths) are
/// rejected with a clear static error message. Both gate surfaces
/// reuse `assert_polygon_chain_id` so the v0.2 `PolygonChain::ZkEvm`
/// addition lands without CLI-side churn.
fn assert_domain_chain_id_consistency(
    parsed: &serde_json::Value,
    cli_chain_id: u64,
) -> polygon_wallet_core::Result<()> {
    // `parsed.get("domain")` returns Some(&Value) even for non-object
    // values (null, string, array, etc.) — guard with `is_object()` so
    // the chainId sub-gate only fires for legitimate domain objects.
    let domain = match parsed.get("domain") {
        Some(d) if d.is_object() => d,
        _ => return Ok(()),
    };
    let chain_id_value = match domain.get("chainId") {
        Some(c) => c,
        None => return Ok(()),
    };
    // Strict u64 decode. Static error message — never echoes the
    // attacker-controlled `Value` (which could carry arbitrary bytes
    // for log-injection / DoS volume).
    let domain_chain_id = chain_id_value.as_u64().ok_or_else(|| {
        Error::InvalidInput(
            "eip712: domain.chainId must be a non-negative integer per EIP-712 uint256; \
             got string, float, negative, or non-integer value"
                .into(),
        )
    })?;
    // Domain gate: domain.chainId must be Polygon PoS.
    assert_polygon_chain_id(domain_chain_id)?;
    // Match check: CLI arg must equal authoritative domain value.
    if domain_chain_id != cli_chain_id {
        return Err(Error::InvalidInput(format!(
            "eip712: CLI --chain-id ({cli_chain_id}) must match \
             typed.domain.chainId ({domain_chain_id})"
        )));
    }
    Ok(())
}

/// **Dispatch wiring deferred to T6 follow-up PR** — see `sign_message`
/// doc above for the same `WalletManager::unlock` deferral rationale.
#[allow(dead_code)] // wired in main.rs dispatch in T6 follow-up PR
pub fn sign_typed_data(
    signer: &alloy_signer_local::PrivateKeySigner,
    typed_data_json: &str,
    chain_id: u64,
    verify_address: Option<alloy_primitives::Address>,
) -> polygon_wallet_core::Result<String> {
    // Q7 + C1 gate (step 1): CLI --chain-id must be Polygon PoS.
    assert_polygon_chain_id(chain_id)?;
    // Q7 + #463 gate (step 2): parse typed_data_json, gate domain.chainId.
    // Deliberately `InvalidInput` (exit 2), not the lib-side `WalletCorrupt`
    // (exit 5) that `From<serde_json::Error>` would yield — malformed
    // typed-data JSON from the CLI is operator input error, not wallet
    // file corruption. (Behavior reclassification flagged in PR body.)
    let parsed: serde_json::Value = serde_json::from_str(typed_data_json)
        .map_err(|e| Error::InvalidInput(format!("eip712: typed_data_json parse: {e}")))?;
    assert_domain_chain_id_consistency(&parsed, chain_id)?;
    let typed_blob = typed_data_json.as_bytes();
    let sig = evm_wallet_core::sign_typed_data(signer, typed_blob).map_err(|e| {
        use evm_wallet_core::SignError;
        match e {
            SignError::InvalidAddress(s) | SignError::InvalidRequest(s) => {
                Error::InvalidInput(format!("eip712: {s}"))
            }
            SignError::Sign(s) | SignError::Unsupported(s) => Error::Rpc(format!("eip712: {s}")),
        }
    })?;
    // --verify deferred alongside the alloy eip712 feature gate.
    // Maps to Error::InvalidInput (caller-side: requested a flag we
    // can't honour yet) — exit 2, not exit 3 (Rpc).
    if verify_address.is_some() {
        return Err(Error::InvalidInput(
            "eip712 --verify deferred to follow-up PR alongside alloy eip712 feature gate".into(),
        ));
    }
    // When the alloy eip712 feature lands and the lib returns Ok(sig),
    // encode the real signature. Until then the `?` above exits early.
    Ok(format!(
        "0x{}",
        alloy_primitives::hex::encode(sig.as_bytes())
    ))
}

#[cfg(test)]
mod tests {
    //! Batch D tests (per design doc §6.4): EIP-712 chain_id gate.
    use super::{assert_polygon_chain_id, polygon_chain_from_id};
    use polygon_wallet_core::{Error, PolygonChain};

    /// Batch D test #1 (failing seed per design doc §6.4): chain_id=1
    /// (Ethereum mainnet) rejected — cross-chain replay blocked at the
    /// type level.
    #[test]
    fn assert_polygon_chain_id_rejects_chain_id_1() {
        let r = assert_polygon_chain_id(1);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "chain_id=1 (Ethereum mainnet) must be rejected; got {r:?}"
        );
    }

    /// Batch D test #2: chain_id=11155111 (Sepolia) rejected.
    #[test]
    fn assert_polygon_chain_id_rejects_chain_id_sepolia() {
        let r = assert_polygon_chain_id(11155111);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "chain_id=11155111 (Sepolia) must be rejected; got {r:?}"
        );
    }

    /// Batch D test #3: chain_id=137 (Polygon PoS mainnet) accepted.
    #[test]
    fn assert_polygon_chain_id_accepts_chain_id_137() {
        assert!(assert_polygon_chain_id(137).is_ok());
    }

    /// Batch D test #4: chain_id=80002 (Polygon PoS amoy) accepted.
    #[test]
    fn assert_polygon_chain_id_accepts_chain_id_80002() {
        assert!(assert_polygon_chain_id(80002).is_ok());
    }

    /// Batch D test #5: unknown chain_id rejected.
    #[test]
    fn assert_polygon_chain_id_rejects_unknown_chain_id() {
        let r = assert_polygon_chain_id(99999);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "chain_id=99999 must be rejected; got {r:?}"
        );
    }

    /// Batch D test #6 (companion): polygon_chain_from_id round-trips
    /// with PolygonChain::chain_id().
    #[test]
    fn polygon_chain_from_id_round_trips() {
        assert_eq!(
            polygon_chain_from_id(PolygonChain::Mainnet.chain_id()).unwrap(),
            PolygonChain::Mainnet
        );
        assert_eq!(
            polygon_chain_from_id(PolygonChain::Amoy.chain_id()).unwrap(),
            PolygonChain::Amoy
        );
    }

    // ===== T6d-3 Batch D-2 tests: sign_message + sign_typed_data handlers =====
    //
    // Per design doc §6.4 (extended for the handler wrappers added in T6d-3).
    // The first 6 tests above cover the Q7 gate helper; the tests below
    // cover the public CLI handler wrappers (`sign_message`, `sign_typed_data`).

    /// Build a test signer (deterministic, abandon×11 phrase at index 0).
    /// Mirrors `evm-wallet-core/src/signer.rs:186` so CLI tests use the
    /// same key material — cross-crate consistency for any future
    /// golden-signature fixtures.
    fn cli_test_signer() -> alloy_signer_local::PrivateKeySigner {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        alloy_signer_local::MnemonicBuilder::english()
            .phrase(phrase)
            .index(0)
            .expect("valid index")
            .build()
            .expect("build")
    }

    /// T6d-3 test #7: `sign_message` returns deterministic 65-byte signature
    /// as `0x`-prefixed hex. EIP-191 personal_sign — no chain_id involvement.
    #[test]
    fn sign_message_returns_0x_prefixed_hex_signature() {
        let signer = cli_test_signer();
        let msg = b"hello, polygon";
        let sig = super::sign_message(&signer, msg, None).expect("sign");
        // 0x + 130 hex chars = 65 raw bytes.
        assert!(sig.starts_with("0x"), "must be 0x-prefixed hex; got {sig}");
        assert_eq!(
            sig.len(),
            132,
            "0x + 130 hex chars = 65 raw bytes; got {} chars",
            sig.len()
        );
    }

    /// T6d-3 test #8: `sign_message` is deterministic for given (key, message).
    /// Mirrors `evm-wallet-core/src/signer.rs:238`.
    #[test]
    fn sign_message_is_deterministic_per_key_and_message() {
        let signer = cli_test_signer();
        let msg = b"deterministic-test";
        let sig1 = super::sign_message(&signer, msg, None).expect("sig1");
        let sig2 = super::sign_message(&signer, msg, None).expect("sig2");
        assert_eq!(sig1, sig2, "sign_message must be deterministic");
    }

    /// T6d-3 test #9: `sign_message` --verify round-trips when expected
    /// address matches the signer.
    #[test]
    fn sign_message_verify_round_trips_to_signer_address() {
        let signer = cli_test_signer();
        let msg = b"verify-me";
        let signer_addr = signer.address();
        let sig = super::sign_message(&signer, msg, Some(signer_addr)).expect("verify pass");
        assert!(sig.starts_with("0x"));
    }

    /// T6d-3 test #10: `sign_message` --verify with WRONG address returns
    /// `Error::InvalidInput` (exit 2) — protects against silent address
    /// substitution attacks.
    #[test]
    fn sign_message_verify_mismatch_returns_invalid_input() {
        let signer = cli_test_signer();
        let msg = b"verify-me";
        // All-zero address — guaranteed not to be the test signer's address.
        let wrong_addr = alloy_primitives::Address::ZERO;
        let r = super::sign_message(&signer, msg, Some(wrong_addr));
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "--verify mismatch must return InvalidInput (exit 2); got {r:?}"
        );
    }

    /// T6d-3 test #11: `sign_typed_data` rejects chain_id=1 (Ethereum
    /// mainnet) at the GATE — BEFORE the lib call. Q7 critical-tier
    /// mitigation: cross-chain replay blocked at the type level.
    #[test]
    fn sign_typed_data_rejects_chain_id_1_at_gate() {
        let signer = cli_test_signer();
        let r = super::sign_typed_data(&signer, r#"{"types":{}}"#, 1, None);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "chain_id=1 must be rejected at gate; got {r:?}"
        );
    }

    /// T6d-3 test #12: `sign_typed_data` rejects chain_id=11155111
    /// (Sepolia) at the GATE.
    #[test]
    fn sign_typed_data_rejects_chain_id_sepolia_at_gate() {
        let signer = cli_test_signer();
        let r = super::sign_typed_data(&signer, r#"{"types":{}}"#, 11_155_111, None);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "chain_id=Sepolia must be rejected at gate; got {r:?}"
        );
    }

    /// T6d-3 test #13: `sign_typed_data` accepts chain_id=137 (Polygon
    /// PoS mainnet) and reaches the lib call. The lib is currently
    /// stubbed (alloy eip712 feature gate deferred per signer.rs:164),
    /// so we expect either the gate-passed path (real `0x` hex sig
    /// once lib lands) OR the lib's `Error::Rpc`. Either way the gate
    /// DID NOT fire — which is the contract we're verifying here.
    #[test]
    fn sign_typed_data_chain_id_137_passes_gate() {
        let signer = cli_test_signer();
        let r = super::sign_typed_data(&signer, r#"{"types":{}}"#, 137, None);
        match r {
            // Once alloy eip712 lands, the handler returns the real
            // 65-byte signature encoded as `0x` + 130 hex chars.
            Ok(sig) => assert!(
                sig.starts_with("0x") && sig.len() == 132,
                "gate-passed result must be 0x-prefixed 65-byte hex; got {sig}"
            ),
            // Today the lib stub returns Error::Rpc — gate DID NOT fire.
            Err(Error::Rpc(msg)) => assert!(
                msg.contains("eip712"),
                "lib-side error must surface honestly; got {msg}"
            ),
            other => panic!(
                "chain_id=137 must pass gate (Ok sig or Rpc); got {other:?} \
                 — gate must NOT reject Polygon chain_ids"
            ),
        }
    }

    /// T6d-3 test #14: `sign_typed_data` accepts chain_id=80002 (Amoy).
    #[test]
    fn sign_typed_data_chain_id_80002_passes_gate() {
        let signer = cli_test_signer();
        let r = super::sign_typed_data(&signer, r#"{"types":{}}"#, 80_002, None);
        match r {
            Ok(sig) => assert!(sig.starts_with("0x") && sig.len() == 132),
            Err(Error::Rpc(_)) => {} // honest lib-side deferral
            other => panic!("chain_id=80002 must pass gate; got {other:?}"),
        }
    }

    // ===== #463 follow-up tests: domain.chainId gate =====
    //
    // EIP-712 §7: `typed.domain.chainId` is authoritative; the CLI
    // `--chain-id` arg is convenience metadata. Without this gate an
    // attacker (or careless operator) can pass `--chain-id 137` with a
    // payload declaring `domain.chainId=1` (Ethereum mainnet) — current
    // CLI gate accepts (137 ∈ {137, 80002}), lib signs the Ethereum
    // payload, cross-chain replay hole.
    //
    // These tests enforce the #463 mitigation BEFORE the alloy `eip712`
    // feature lands (lib is currently stubbed at signer.rs:164). Gate
    // fires regardless of lib state — defense-in-depth.

    /// #463 test #1 (bypass scenario): CLI `--chain-id 137` passes the
    /// CLI gate, BUT `typed.domain.chainId=1` (Ethereum mainnet) MUST
    /// be rejected with `Error::InvalidInput`. The dual-source gate is
    /// what closes the cross-chain replay hole.
    #[test]
    fn sign_typed_data_with_domain_chain_id_1_rejected_even_if_cli_says_137() {
        let signer = cli_test_signer();
        let json = r#"{"domain":{"chainId":1,"name":"Ether Mail"},"types":{}}"#;
        let r = super::sign_typed_data(&signer, json, 137, None);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "domain.chainId=1 must be rejected even when CLI --chain-id=137 \
             (cross-chain replay blocked); got {r:?}"
        );
    }

    /// #463 test #2 (mismatch scenario): domain.chainId is a valid
    /// Polygon chain_id (80002 = Amoy), but the CLI arg is 137
    /// (mainnet). Domain is authoritative — must reject with
    /// `Error::InvalidInput` and surface the mismatch.
    #[test]
    fn sign_typed_data_cli_and_domain_chain_ids_must_match() {
        let signer = cli_test_signer();
        let json = r#"{"domain":{"chainId":80002,"name":"X"},"types":{}}"#;
        let r = super::sign_typed_data(&signer, json, 137, None);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "CLI --chain-id=137 with domain.chainId=80002 must be rejected \
             as a mismatch; got {r:?}"
        );
    }

    /// #463 test #3 (fallthrough scenario, loosest documented behavior):
    /// domain field exists but has NO `chainId` key. CLI gate (137)
    /// passes → domain gate skipped (no authoritative value to check)
    /// → lib call. Lib stub returns `Error::Rpc` today; once alloy
    /// `eip712` lands it returns the real `Ok(sig)`. Both are valid
    /// outcomes — the contract is "don't reject just because domain
    /// omits chainId; CLI arg becomes authoritative in that case".
    #[test]
    fn sign_typed_data_domain_without_chain_id_falls_through_to_cli_gate() {
        let signer = cli_test_signer();
        let json = r#"{"domain":{"name":"X"},"types":{}}"#;
        let r = super::sign_typed_data(&signer, json, 137, None);
        match r {
            Ok(sig) => assert!(
                sig.starts_with("0x") && sig.len() == 132,
                "fallthrough to CLI gate must yield a 0x-prefixed 65-byte hex sig; got {sig}"
            ),
            Err(Error::Rpc(msg)) => assert!(
                msg.contains("eip712"),
                "lib-side deferral must surface honestly; got {msg}"
            ),
            other => panic!(
                "domain without chainId + CLI --chain-id=137 must NOT reject \
                 (fallthrough to CLI gate); got {other:?}"
            ),
        }
    }

    // ===== #463 edge-case tests: domain.chainId type/shape rejection =====
    //
    // Pins `assert_domain_chain_id_consistency` strict-integer decode.
    // If serde_json ever changes its integer-vs-float representation
    // (e.g. `arbitrary_precision` feature), these tests fail loudly
    // instead of silently widening the accepted set. EIP-712 §7
    // specifies `chainId` as `uint256` (integer-only); float / string /
    // negative / null / array / bool / overflow are all non-conformant.

    /// chainId is JSON string `"137"` — rejected by `as_u64()`.
    #[test]
    fn sign_typed_data_rejects_string_domain_chain_id() {
        let signer = cli_test_signer();
        let json = r#"{"domain":{"chainId":"137"},"types":{}}"#;
        let r = super::sign_typed_data(&signer, json, 137, None);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "string domain.chainId must be rejected (EIP-712 uint256 is integer-only); got {r:?}"
        );
    }

    /// chainId is float `137.0` — strict EIP-712 rejects (uint256).
    /// Per security audit: no float leniency; operator must re-encode.
    #[test]
    fn sign_typed_data_rejects_float_domain_chain_id() {
        let signer = cli_test_signer();
        let json = r#"{"domain":{"chainId":137.0},"types":{}}"#;
        let r = super::sign_typed_data(&signer, json, 137, None);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "float domain.chainId (137.0) must be rejected per EIP-712 uint256 conformance; got {r:?}"
        );
    }

    /// chainId is negative `-1` — rejected by `as_u64()`.
    #[test]
    fn sign_typed_data_rejects_negative_domain_chain_id() {
        let signer = cli_test_signer();
        let json = r#"{"domain":{"chainId":-1},"types":{}}"#;
        let r = super::sign_typed_data(&signer, json, 137, None);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "negative domain.chainId must be rejected; got {r:?}"
        );
    }

    /// chainId is JSON `null` — rejected by `as_u64()`.
    #[test]
    fn sign_typed_data_rejects_null_domain_chain_id() {
        let signer = cli_test_signer();
        let json = r#"{"domain":{"chainId":null},"types":{}}"#;
        let r = super::sign_typed_data(&signer, json, 137, None);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "null domain.chainId must be rejected; got {r:?}"
        );
    }

    /// chainId is array `[137]` — rejected by `as_u64()`.
    #[test]
    fn sign_typed_data_rejects_array_domain_chain_id() {
        let signer = cli_test_signer();
        let json = r#"{"domain":{"chainId":[137]},"types":{}}"#;
        let r = super::sign_typed_data(&signer, json, 137, None);
        assert!(
            matches!(r, Err(Error::InvalidInput(_))),
            "array domain.chainId must be rejected; got {r:?}"
        );
    }

    /// domain is `null` — `get("chainId")` returns None on non-object,
    /// so the chainId sub-gate is silently skipped and execution
    /// falls through to the CLI gate. Pins the silent-skips
    /// non-object-domain behavior.
    #[test]
    fn sign_typed_data_null_domain_falls_through_to_cli_gate() {
        let signer = cli_test_signer();
        let json = r#"{"domain":null,"types":{}}"#;
        let r = super::sign_typed_data(&signer, json, 137, None);
        match r {
            Ok(sig) => assert!(sig.starts_with("0x") && sig.len() == 132),
            Err(Error::Rpc(_)) => {} // honest lib-side deferral
            other => panic!(
                "null domain + CLI --chain-id=137 must NOT reject \
                 (non-object domain → fall through to CLI gate); got {other:?}"
            ),
        }
    }

    /// Malformed JSON — parse error maps to `InvalidInput` (exit 2),
    /// NOT `WalletCorrupt` (exit 5). Operator input error class.
    #[test]
    fn sign_typed_data_rejects_malformed_json_as_invalid_input() {
        let signer = cli_test_signer();
        let r = super::sign_typed_data(&signer, "not-json", 137, None);
        match &r {
            Err(Error::InvalidInput(msg)) => assert!(
                msg.contains("parse"),
                "parse error must mention 'parse'; got {msg}"
            ),
            other => panic!(
                "malformed JSON must map to InvalidInput (exit 2), not WalletCorrupt; got {other:?}"
            ),
        }
    }
}
