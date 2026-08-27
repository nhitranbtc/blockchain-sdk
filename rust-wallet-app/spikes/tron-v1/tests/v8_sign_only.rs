//! V8 — sign-only path (Q8) — partially gated (L29).
//!
//! Plan §Q8: k256 ECDSA sign over raw tx hash → 65-byte `r‖s‖v` with `v ∈ {0, 1}`.
//! NOT Ethereum convention (`v + 27`). `k256::ecdsa::Signature::from_sliced_64(...)` +
//! `k256::ecdsa::VerifyingKey::recover_from_prehash(...)` for recovery-byte computation.
//!
//! This test does the offline sign + recovery locally; live broadcast verification
//! against Nile is gated.

use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::{RecoveryId, SigningKey};

#[test]
fn v8_sign_65_bytes_with_recovery_v_in_0_1() {
    // Random test message hash (32 bytes).
    let prehash = [0xab; 32];

    let signing_key = SigningKey::from_bytes(&[0x11u8; 32].into()).unwrap();
    let verifying_key = signing_key.verifying_key();

    let sig: k256::ecdsa::Signature = signing_key.sign_prehash(&prehash).unwrap();

    // Serialize → 64-byte `r‖s‖BE`.
    let rs = sig.to_bytes();
    assert_eq!(rs.len(), 64);

    // TRON recovery byte must be ∈ {0, 1} per plan §Q2.
    let v: u8 = signing_key
        .sign_prehash_recoverable(&prehash)
        .unwrap()
        .1
        .to_byte();
    assert!(v <= 1, "TRON v byte must be ∈ {{0, 1}}, got {v}");

    // Recovery round-trip.
    let rid = RecoveryId::from_byte(v).expect("v in {0,1} maps to valid RecoveryId");
    let recovered = k256::ecdsa::VerifyingKey::recover_from_prehash(&prehash, &sig, rid).unwrap();
    assert_eq!(recovered, *verifying_key);

    // 65-byte canonical form = `r ‖ s ‖ v`.
    let mut canonical = [0u8; 65];
    canonical[..64].copy_from_slice(&rs);
    canonical[64] = v;
    assert_eq!(canonical.len(), 65);
}

#[test]
fn v8_signature_layout_no_eth_v_plus_27() {
    // Plan §Q2/Q8: NOT Ethereum's v+27 ∈ {27, 28}. Confirm k256 does NOT emit high-bit v.
    let prehash = [0xcd; 32];
    let signing_key = SigningKey::from_bytes(&[0x22u8; 32].into()).unwrap();
    let (_recoverable_sig, recovery_id) = signing_key.sign_prehash_recoverable(&prehash).unwrap();
    let v = recovery_id.to_byte();
    assert!(
        v <= 1,
        "k256 recovery byte must be ≤1 (no Ethereum +27 offset): got {v}"
    );
}
