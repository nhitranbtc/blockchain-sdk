//! Spike V8 — sign-only path.
//!
//! Plan: `docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`
//! Phase 1 Task 1.5. Feeds the Phase 7 V8 PASS gate.
//!
//! Two upstream behaviours are pinned here because the plan's Risk Register
//! accepts them rather than forking `anychain`:
//!
//! * **Risk 2** — `anychain_tron` computes a transaction id with a single
//!   SHA-256, but TRON's wire format hashes twice. `tx::sign::txid` applies the
//!   caller-side workaround, and `txid_is_double_sha256` fails if a future
//!   `anychain` release "fixes" this and silently changes every id we produce.
//! * **Q8** — TRON expects a recovery id of 0 or 1, not Ethereum's 27/28
//!   offset. `tx::sign::sign_hash` rejects anything else rather than passing it
//!   on to a broadcast that would be refused by the node.

use tron_wallet_core::keys::{derive_keypair, Language, Mnemonic};
use tron_wallet_core::tx::sign::{sign_hash, txid, RecoveryId, SIGNATURE_LEN};
use zeroize::Zeroizing;

const CANONICAL_PHRASE: &str = "abandon abandon abandon abandon abandon abandon \
     abandon abandon abandon abandon abandon about";

const TRON_PATH: &str = "m/44'/195'/0'/0/0";

fn canonical_secret() -> Zeroizing<[u8; 32]> {
    let mnemonic = Mnemonic::from_phrase(CANONICAL_PHRASE, Language::English).expect("phrase");
    let keypair = derive_keypair(&mnemonic, "", &TRON_PATH.parse().expect("path"))
        .expect("derivation must succeed");
    keypair.secret_bytes().clone()
}

#[test]
fn sign_hash_returns_64_byte_signature() {
    let signed = sign_hash(&canonical_secret(), &[0x11; 32]).expect("signing must succeed");

    assert_eq!(SIGNATURE_LEN, 64, "r || s, 32 bytes each");
    assert_eq!(signed.signature().as_bytes().len(), SIGNATURE_LEN);
}

/// Q8: the recovery id TRON accepts is 0 or 1. Ethereum's 27/28 form must never
/// leave this crate.
#[test]
fn recovery_id_is_zero_or_one_never_eip155_offset() {
    // A spread of message hashes, so this is not a single lucky draw.
    for byte in [0x00u8, 0x01, 0x7f, 0x80, 0xfe, 0xff] {
        let signed = sign_hash(&canonical_secret(), &[byte; 32]).expect("signing must succeed");
        let v = signed.recovery_id().to_u8();

        assert!(
            v == 0 || v == 1,
            "recovery id must be 0 or 1 for message byte {byte:#04x}, got {v}"
        );
    }
}

/// The wire form cannot be assembled with a recovery id TRON rejects, because
/// `RecoveryId` has no other constructor.
#[test]
fn eip155_recovery_ids_cannot_be_constructed() {
    for rejected in [2u8, 3, 27, 28] {
        assert!(
            RecoveryId::new(rejected).is_err(),
            "constructed a recovery id TRON rejects: {rejected}"
        );
    }
}

#[test]
fn wire_form_is_65_bytes_ending_in_the_recovery_id() {
    let signed = sign_hash(&canonical_secret(), &[0x11; 32]).expect("signing must succeed");
    let wire = signed.to_tron_bytes();

    assert_eq!(wire.len(), 65);
    assert_eq!(&wire[..64], signed.signature().as_bytes());
    assert_eq!(wire[64], signed.recovery_id().to_u8());
}

#[test]
fn signing_is_deterministic_for_the_same_input() {
    let msg = [0x42; 32];

    let first = sign_hash(&canonical_secret(), &msg).expect("signing must succeed");
    let second = sign_hash(&canonical_secret(), &msg).expect("signing must succeed");

    assert_eq!(first, second, "RFC 6979 nonce");
}

#[test]
fn distinct_messages_produce_distinct_signatures() {
    let a = sign_hash(&canonical_secret(), &[0x01; 32]).expect("signing must succeed");
    let b = sign_hash(&canonical_secret(), &[0x02; 32]).expect("signing must succeed");

    assert_ne!(a.signature().as_bytes(), b.signature().as_bytes());
}

#[test]
fn signing_rejects_an_out_of_range_secret() {
    // All-zero is not a valid secp256k1 scalar.
    assert!(sign_hash(&Zeroizing::new([0u8; 32]), &[0x11; 32]).is_err());
}

/// Risk 2 regression. The expected value is SHA-256 applied twice to the empty
/// input, confirmed independently:
///
/// ```text
/// $ printf '' | sha256sum
/// e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
/// $ printf '' | sha256sum | cut -d' ' -f1 | xxd -r -p | sha256sum
/// 5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456
/// ```
#[test]
fn txid_is_double_sha256() {
    let expected = "5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456";

    assert_eq!(hex::encode(txid(b"")), expected);
}

#[test]
fn txid_differs_from_single_sha256() {
    use sha2::{Digest, Sha256};

    let raw = b"tron raw transaction bytes";
    let single: [u8; 32] = Sha256::digest(raw).into();

    assert_ne!(
        txid(raw),
        single,
        "a single-SHA256 txid means the Risk 2 workaround was dropped"
    );
}
