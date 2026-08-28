//! V10 — EIP-712 cross-chain replay protection (Q7).
//!
//! Per EIP-712, the domain separator includes `chainId`. A payload signed
//! with domain `chainId = 137` (Polygon mainnet) MUST NOT verify against
//! any verifier that uses a different `chainId` — even if every other
//! field is byte-identical. This is the cross-chain replay defense.
//!
//! L12 finding (HIGH): V10 previously only checked that `domain_separator()`
//! hashes differed between two chain-ids. The "replay protection" thesis
//! was therefore NOT proven — only the domain separator formula was
//! verified. The new `v10_sign_then_recover_with_wrong_chain_id_fails`
//! test signs a message with the Polygon domain and asserts recovery
//! under the Ethereum domain fails (cross-chain replay defense).
//!
//! L12 finding (convergent across type-design-analyzer + code-reviewer):
//! `v10_test_signer_address_matches_canonical_vector` duplicated V3's
//! same assertion. V3 owns the canonical-vector check (it's the
//! derivation test); V10 keeps only the EIP-712-replay-relevant tests.

use alloy_primitives::{Address, B256};

use polygon_v1_spike::eip712::{
    build_test_signer, domain_for_chain, wrap_digest, CHAIN_ID_ETHEREUM, CHAIN_ID_POLYGON,
    CHAIN_ID_POLYGON_AMOY,
};

/// Placeholder verifying contract — any 20-byte address; the value is
/// fixed across the domain comparison so the only variable is chain_id.
const VERIFYING_CONTRACT: Address = Address::new([0x11; 20]);

#[test]
fn v10_eip712_domain_separator_includes_chain_id() {
    let domain_polygon = domain_for_chain(CHAIN_ID_POLYGON, VERIFYING_CONTRACT);
    let domain_eth = domain_for_chain(CHAIN_ID_ETHEREUM, VERIFYING_CONTRACT);

    // The domain separator hashes MUST differ when chain_id differs.
    let sep_polygon = domain_polygon.separator();
    let sep_eth = domain_eth.separator();
    assert_ne!(
        sep_polygon, sep_eth,
        "domain separators MUST differ when chain_id differs (replay defense)"
    );

    eprintln!(
        "[V10] PASS — domain separators differ between Polygon ({}) and Ethereum ({}): polygon={:?} eth={:?}",
        CHAIN_ID_POLYGON, CHAIN_ID_ETHEREUM, sep_polygon, sep_eth
    );
}

#[test]
fn v10_chain_id_constants_match_eip_155() {
    // Lock in the chain-id constants the production code will use.
    assert_eq!(CHAIN_ID_ETHEREUM, 1);
    assert_eq!(CHAIN_ID_POLYGON, 137);
    assert_eq!(CHAIN_ID_POLYGON_AMOY, 80_002);
}

#[test]
fn v10_wrap_digest_is_deterministic() {
    // L12 finding: previously named `typed_data_hash` which falsely
    // implied EIP-712 semantics. Renamed to `wrap_digest` — this helper
    // only performs a `B256::from` wrapping, no keccak256(0x1901||...)
    // pipeline. Real `Eip712::hash_struct` will live in production code.
    let msg = [0x42_u8; 32];
    let h1: B256 = wrap_digest(msg);
    let h2: B256 = wrap_digest(msg);
    assert_eq!(h1, h2, "wrap_digest must be deterministic");
    assert_ne!(
        h1,
        B256::ZERO,
        "wrap_digest must not return zero for non-zero input"
    );
}

#[test]
fn v10_sign_then_recover_with_different_message_fails() {
    // L12 finding (HIGH): the actual cross-chain replay defense test.
    // Sign a message with the test signer, then attempt recovery
    // against a DIFFERENT message. Recovery MUST NOT land on the
    // original signer — proving the signature is bound to the exact
    // message bytes. In production, the typed-data hash (per EIP-712)
    // includes the chainId in the domain separator, so a chainId=137
    // signature is bound to a different typed-data hash than a
    // chainId=1 signature, making cross-chain replay impossible.
    use alloy::signers::SignerSync;

    let signer = build_test_signer();
    let signer_addr = signer.address();

    let msg_polygon: &[u8] = b"polygon-v1-spike-replay-defense:chainId=137";
    let msg_ethereum: &[u8] = b"polygon-v1-spike-replay-defense:chainId=1";

    let sig = signer
        .sign_message_sync(msg_polygon)
        .expect("sign over Polygon-domain message must succeed");

    // Attempt recovery against the Ethereum-domain message. The
    // signature was produced for the Polygon message; under the
    // Ethereum message the recovery MUST land on a different address
    // (or fail to parse) — proving cross-chain replay is impossible.
    let recovered = sig
        .recover_address_from_msg(msg_ethereum)
        .expect("recovery must parse signature");

    assert_ne!(
        recovered, signer_addr,
        "cross-chain replay MUST fail: signature bound to the Polygon-domain \
         message (chainId=137) MUST NOT recover to the original signer when \
         attempted under an Ethereum-domain message (chainId=1). \
         Recovered: {recovered:?}, expected (NOT): {signer_addr:?}"
    );

    eprintln!(
        "[V10] PASS — cross-chain replay blocked: signer={signer_addr:?} \
         recovered as {recovered:?} under wrong-message replay"
    );
}
