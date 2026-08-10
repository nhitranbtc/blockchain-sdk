//! BIP-137 message signing + verification (Bitcoin Core + Trezor conventions).
//!
//! **Drift from plan §Task 6:**
//!
//! | Plan said | This implementation | Why |
//! |---|---|---|
//! | `sign_message_bip137(message, address_pkh, sighash_fn)` takes `Sighash`/`MessageClass` from `threat.rs` | `sign_message(message, &Signer, &Address)` | `threat.rs` ships Task 9; `MessageClass::Bip137Message` is wrapped inside `sign_message`, not caller-facing (F21 documentary only in Task 6) |
//! | Single `sign_message`, no `verify_message` | Added `verify_message(addr, sig, msg) -> Result<bool>` | F50 + F7 demand verify side for cross-tool interop (Bitcoin Core `verifymessage` RPC, Trezor Suite, Electrum) |
//! | `address_pkh: &[u8; 20]` | `&Address` with internal pkh extraction via `address.pubkey_hash()` | Type-safe caller contract; rejected non-P2PKH addresses get explicit error |
//! | Returns `String` (base64 sig) | Returns `SignedMessage` (Display + FromStr + Debug shows base64) | Public-data newtype, NOT `finish_non_exhaustive` (that's for sensitive types per L17) |
//! | Header byte 31-34 only | Sign emits 31-34; verify accepts 27-30 (Bitcoin Core uncompressed) and 31-34 (Trezor compressed) | Cross-tool interop: Bitcoin Core `verifymessage` signs 27-30 with uncompressed pubkeys |
//! | Hand-rolled `double_sha256` | `bitcoin::hashes::sha256d::Hash::hash` | Re-exported from `bitcoin` crate; avoids drift |
//! | `[u8; N] ==` for hash compare | `subtle::ConstantTimeEq::ct_eq` | F50 timing oracle defense (verify takes attacker-controlled inputs) |
//! | 3 varint tests | 20 tests incl. cross-tool interop + header byte matrix + base64 edges | F9 + F50 + cross-tool interop coverage |
//! | `pub sign_recoverable` on Signer | `pub(crate)` on Signer | F7 hygiene — `pub` 32-byte signing surface is a phishing vector |
//! | `Error::Sign(String)` | `Error::Bip137(String)` | F43 pattern (per-protocol variant); keeps transaction-signing errors distinct |
//!
//! **Defends against:** F7 (U5 narrow API — only `sign_message` is public signing entry;
//! `sign_recoverable` is `pub(crate)`), F9 (Bitcoin varint per BIP-137 spec), F21 (Sighash/MessageClass
//! deferred to Task 9 — `sign_message` is the documentary contract), F50 (recovery-id byte 27-34,
//! `subtle::ConstantTimeEq` for hash160 compare defends timing oracle on attacker-controlled verify).
//!
//! **Cross-tool interop:** Sign emits Trezor-style header 31-34 (compressed P2PKH). Verify accepts
//! both Bitcoin Core (27-30, uncompressed) and Trezor (31-34, compressed) so signatures produced by
//! either tool verify here.
//!
//! **Deferred:** F9 cross-verification against Bitcoin Core `verifymessage` RPC (v0.1.1 per plan).

use base64::Engine;
use bdk_wallet::bitcoin::hashes::{hash160, sha256d, Hash};
use bdk_wallet::bitcoin::secp256k1::ecdsa::{RecoverableSignature, RecoveryId};
use bdk_wallet::bitcoin::secp256k1::{Message, Secp256k1};
use bdk_wallet::bitcoin::Address;
use subtle::ConstantTimeEq;

use crate::error::{Error, Result};
use crate::keys::Signer;

/// BIP-137 magic prefix: `0x18` (= 24, length of "Bitcoin Signed Message:\n") +
/// "Bitcoin Signed Message:\n". 25 bytes total. Per BIP-137 spec.
/// Compile-time pinned: length-byte drift is caught at build time
/// (body content is checked by `magic_prefix_is_25_bytes` test).
/// See Issue #30.
pub const MAGIC_PREFIX: &[u8; 25] = {
    const INNER: &[u8; 25] = b"\x18Bitcoin Signed Message:\n";
    const BODY_LEN: usize = b"Bitcoin Signed Message:\n".len();
    assert!(
        INNER[0] as usize == BODY_LEN,
        "MAGIC_PREFIX length byte must equal the body string length"
    );
    INNER
};

/// Header byte offset for uncompressed P2PKH signatures (Bitcoin Core convention).
/// Verify-only range 27..=30. Sign never emits this range.
/// Compile-time pinned — see Issue #30.
pub const HEADER_OFFSET_UNCOMPRESSED: u8 = {
    const INNER: u8 = 27;
    assert!(
        INNER == 27,
        "HEADER_OFFSET_UNCOMPRESSED must be 27 per BIP-137"
    );
    INNER
};

/// Header byte offset for compressed P2PKH signatures (Trezor convention).
/// Sign emits headers in range 31..=34. Verify accepts this range + 27..=30.
/// Compile-time pinned — see Issue #30.
pub const HEADER_OFFSET_COMPRESSED: u8 = {
    const INNER: u8 = 27 + 4;
    assert!(
        INNER == 31,
        "HEADER_OFFSET_COMPRESSED must be 31 (= 27 + 4) per BIP-137"
    );
    INNER
};

/// Full BIP-137 signature length: 1 header byte + 64 compact-sig bytes.
/// Compile-time pinned — see Issue #30.
pub const SIGNATURE_LEN: usize = {
    const INNER: usize = 65;
    assert!(INNER == 65, "SIGNATURE_LEN must be 65 bytes per BIP-137");
    INNER
};

/// Compact ECDSA signature length (excluding header byte).
/// Compile-time pinned — see Issue #30.
pub const COMPACT_SIG_LEN: usize = {
    const INNER: usize = 64;
    assert!(INNER == 64, "COMPACT_SIG_LEN must be 64 bytes per BIP-137");
    INNER
};

/// BIP-137 signed message: base64 of `header_byte (1) || compact_sig (64)` (65 bytes total).
///
/// Public data (the base64 string is meant to be displayed + shared), so Debug shows
/// the base64 contents (NOT `finish_non_exhaustive` — that's for sensitive types per L17).
/// Provides `Display` and `FromStr` for ergonomics.
pub struct SignedMessage(String);

impl std::fmt::Debug for SignedMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SignedMessage").field(&self.0).finish()
    }
}

impl std::fmt::Display for SignedMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for SignedMessage {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let bytes = decode_base64_signature(s)?;
        let header = bytes[0];
        if !is_accepted_header(header) {
            return Err(Error::Bip137(format!(
                "invalid header byte {header} (must be 27..=30 or 31..=34)"
            )));
        }
        Ok(SignedMessage(s.to_string()))
    }
}

impl AsRef<str> for SignedMessage {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Sign `message` with `signer`. Returns a base64-encoded BIP-137 signature
/// (header byte 31..=34 + 64-byte compact ECDSA sig).
///
/// **Algorithm (per BIP-137 spec):**
///
/// 1. `buf = MAGIC_PREFIX || varint(message.len()) || message.as_bytes()`
/// 2. `hash = sha256d(&buf)` (double-SHA256)
/// 3. `(rec_id, compact) = signer.sign_recoverable(&hash)?`
/// 4. Recover pubkey from `(rec_id, compact, hash)`; verify recovered-pubkey
///    hash160 matches `address.pubkey_hash()` (defensive: catches caller error
///    passing wrong address for the signing key).
/// 5. `header = HEADER_OFFSET_COMPRESSED + rec_id`
/// 6. `base64::STANDARD.encode([header; 1] || compact[0..64])`
///
/// **Caller contract:** `address` MUST be a P2PKH address (`1...`/`m...`/`n...`).
/// Non-P2PKH addresses (P2WPKH, P2SH, P2TR) return `Error::Bip137`.
pub fn sign_message(message: &str, signer: &Signer, address: &Address) -> Result<SignedMessage> {
    let hash = bip137_hash(message);
    let (rec_id, compact) = signer.sign_recoverable(&hash)?;
    let secp = Secp256k1::new();
    let rec_sig = RecoverableSignature::from_compact(&compact, rec_id)
        .map_err(|e| Error::Bip137(format!("recover signature: {e}")))?;
    let recovered_pk = secp
        .recover_ecdsa(&Message::from_digest(hash), &rec_sig)
        .map_err(|e| Error::Bip137(format!("recover pubkey: {e}")))?;
    let recovered_hash160 = hash160::Hash::hash(&recovered_pk.serialize());
    let expected_hash160 = p2pkh_hash_from_address(address)?;
    if recovered_hash160.to_byte_array() != expected_hash160 {
        return Err(Error::Bip137(
            "recovered pubkey hash160 does not match address pubkey hash".into(),
        ));
    }
    let header = HEADER_OFFSET_COMPRESSED + rec_id.to_i32() as u8;
    let mut blob = [0u8; SIGNATURE_LEN];
    blob[0] = header;
    blob[1..].copy_from_slice(&compact);
    Ok(SignedMessage(
        base64::engine::general_purpose::STANDARD.encode(blob),
    ))
}

/// Verify that `sig` is a valid BIP-137 signature of `message` by `address`.
///
/// Returns `true` if the recovered pubkey hash160 matches the address's
/// pubkey hash; `false` if the signature is structurally valid but does not
/// sign for this address. Returns `Err` for malformed input (invalid base64,
/// wrong length, invalid header byte, recovery failure).
///
/// **Constant-time:** hash160 comparison uses `subtle::ConstantTimeEq` to defend
/// against timing oracles on attacker-controlled inputs (F50).
pub fn verify_message(address: &Address, sig: &SignedMessage, message: &str) -> Result<bool> {
    let blob = decode_base64_signature(&sig.0)?;
    let header = blob[0];
    let compact: [u8; COMPACT_SIG_LEN] = blob[1..]
        .try_into()
        .expect("blob[1..65] is 64 bytes by length check above");
    let rec_id_i32 = match header {
        27..=30 => (header - HEADER_OFFSET_UNCOMPRESSED) as i32,
        31..=34 => (header - HEADER_OFFSET_COMPRESSED) as i32,
        _ => {
            return Err(Error::Bip137(format!(
                "invalid header byte {header} (must be 27..=30 or 31..=34)"
            )));
        }
    };
    let rec_id = RecoveryId::from_i32(rec_id_i32)
        .map_err(|e| Error::Bip137(format!("invalid recovery id: {e}")))?;
    let hash = bip137_hash(message);
    let secp = Secp256k1::new();
    let rec_sig = RecoverableSignature::from_compact(&compact, rec_id)
        .map_err(|e| Error::Bip137(format!("parse recoverable signature: {e}")))?;
    let recovered_pk = secp
        .recover_ecdsa(&Message::from_digest(hash), &rec_sig)
        .map_err(|e| Error::Bip137(format!("recover pubkey: {e}")))?;
    let recovered_hash160 = hash160::Hash::hash(&recovered_pk.serialize());
    let expected_hash160 = p2pkh_hash_from_address(address)?;
    let choice: subtle::Choice = recovered_hash160.to_byte_array().ct_eq(&expected_hash160);
    Ok(bool::from(choice))
}

/// Compute the BIP-137 message hash: `sha256d(MAGIC_PREFIX || varint(len) || msg)`.
fn bip137_hash(message: &str) -> [u8; 32] {
    let mut buf = Vec::with_capacity(MAGIC_PREFIX.len() + 9 + message.len());
    buf.extend_from_slice(MAGIC_PREFIX);
    encode_varint(&mut buf, message.len());
    buf.extend_from_slice(message.as_bytes());
    sha256d::Hash::hash(&buf).to_byte_array()
}

/// Bitcoin varint encoding (compact size). Per BIP-137 / Bitcoin Core spec.
fn encode_varint(out: &mut Vec<u8>, n: usize) {
    if n < 0xfd {
        out.push(n as u8);
    } else if n <= 0xffff {
        out.push(0xfd);
        out.extend_from_slice(&(n as u16).to_le_bytes());
    } else if n <= 0xffff_ffff {
        out.push(0xfe);
        out.extend_from_slice(&(n as u32).to_le_bytes());
    } else {
        out.push(0xff);
        out.extend_from_slice(&(n as u64).to_le_bytes());
    }
}

/// Decode base64 sig blob and length-check it to exactly SIGNATURE_LEN bytes.
fn decode_base64_signature(b64: &str) -> Result<[u8; SIGNATURE_LEN]> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| Error::Bip137(format!("invalid base64: {e}")))?;
    if bytes.len() != SIGNATURE_LEN {
        return Err(Error::Bip137(format!(
            "invalid signature length: got {} bytes, expected {SIGNATURE_LEN}",
            bytes.len()
        )));
    }
    bytes
        .try_into()
        .map_err(|_| Error::Bip137("signature length mismatch".into()))
}

/// Return `true` if `header` is in either accepted range (27..=30 OR 31..=34).
fn is_accepted_header(header: u8) -> bool {
    matches!(header, 27..=34)
}

/// Extract the 20-byte pubkey hash from a P2PKH `Address`. Returns
/// `Error::Bip137` for non-P2PKH addresses.
fn p2pkh_hash_from_address(address: &Address) -> Result<[u8; 20]> {
    address
        .pubkey_hash()
        .map(|h| *h.as_ref())
        .ok_or_else(|| Error::Bip137("address is not P2PKH".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::Secret;
    use bdk_wallet::bitcoin::secp256k1::PublicKey;
    use bdk_wallet::bitcoin::{Network, PublicKey as BtcPublicKey};
    use std::str::FromStr;

    fn test_signer_and_p2pkh() -> (Signer, Address) {
        let sk_bytes = [0x42u8; 32];
        let signer = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
        let secp = bdk_wallet::bitcoin::secp256k1::Secp256k1::new();
        let sk = bdk_wallet::bitcoin::secp256k1::SecretKey::from_slice(&sk_bytes).unwrap();
        let pk = PublicKey::from_secret_key(&secp, &sk);
        let btc_pk = BtcPublicKey::new(pk);
        let address = Address::p2pkh(btc_pk, Network::Testnet);
        (signer, address)
    }

    #[test]
    fn varint_encoding_zero() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 0);
        assert_eq!(buf, vec![0x00]);
    }

    #[test]
    fn varint_encoding_short() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 10);
        assert_eq!(buf, vec![10]);
    }

    #[test]
    fn varint_encoding_252() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 0xfc);
        assert_eq!(buf, vec![0xfc]);
    }

    #[test]
    fn varint_encoding_253() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 253);
        assert_eq!(buf, vec![0xfd, 253, 0]);
    }

    #[test]
    fn varint_encoding_300() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 300);
        assert_eq!(buf, vec![0xfd, 44, 1]);
    }

    #[test]
    fn varint_encoding_max_u16() {
        let mut buf = Vec::new();
        encode_varint(&mut buf, 0xffff);
        assert_eq!(buf, vec![0xfd, 0xff, 0xff]);
    }

    #[test]
    fn magic_prefix_is_25_bytes() {
        assert_eq!(MAGIC_PREFIX.len(), 25);
        assert_eq!(MAGIC_PREFIX[0], 0x18);
        assert_eq!(&MAGIC_PREFIX[1..], b"Bitcoin Signed Message:\n");
    }

    #[test]
    fn sign_message_produces_base64() {
        let (signer, address) = test_signer_and_p2pkh();
        let signed = sign_message("hello", &signer, &address).expect("sign");
        let blob = base64::engine::general_purpose::STANDARD
            .decode(signed.as_ref())
            .expect("base64 decodes");
        assert_eq!(blob.len(), SIGNATURE_LEN);
    }

    #[test]
    fn sign_message_header_byte_in_31_to_34() {
        let (signer, address) = test_signer_and_p2pkh();
        let signed = sign_message("hello", &signer, &address).expect("sign");
        let blob = base64::engine::general_purpose::STANDARD
            .decode(signed.as_ref())
            .unwrap();
        let header = blob[0];
        assert!(
            (HEADER_OFFSET_COMPRESSED..=HEADER_OFFSET_COMPRESSED + 3).contains(&header),
            "header {header} out of compressed range"
        );
    }

    #[test]
    fn sign_then_verify_roundtrip() {
        let (signer, address) = test_signer_and_p2pkh();
        let signed = sign_message("hello world", &signer, &address).expect("sign");
        let verified = verify_message(&address, &signed, "hello world").expect("verify");
        assert!(verified);
    }

    #[test]
    fn verify_rejects_tampered_signature() {
        let (signer, address) = test_signer_and_p2pkh();
        let signed = sign_message("hello", &signer, &address).expect("sign");
        let mut bytes = signed.0.into_bytes();
        let mid = bytes.len() / 2;
        bytes.swap(mid, mid + 1);
        let tampered = SignedMessage(String::from_utf8(bytes).unwrap());
        let verified = verify_message(&address, &tampered, "hello").expect("verify");
        assert!(!verified);
    }

    #[test]
    fn verify_rejects_tampered_message() {
        let (signer, address) = test_signer_and_p2pkh();
        let signed = sign_message("hello", &signer, &address).expect("sign");
        let verified = verify_message(&address, &signed, "hellz").expect("verify");
        assert!(!verified);
    }

    #[test]
    fn verify_rejects_wrong_address() {
        let (signer, address) = test_signer_and_p2pkh();
        let signed = sign_message("hello", &signer, &address).expect("sign");
        let sk_other = [0x99u8; 32];
        let _other_signer = Signer::from_secret_bytes(Secret::new(sk_other.to_vec()));
        let secp = bdk_wallet::bitcoin::secp256k1::Secp256k1::new();
        let sk = bdk_wallet::bitcoin::secp256k1::SecretKey::from_slice(&sk_other).unwrap();
        let pk_other = PublicKey::from_secret_key(&secp, &sk);
        let btc_pk_other = BtcPublicKey::new(pk_other);
        let other_address = Address::p2pkh(btc_pk_other, Network::Testnet);
        let verified = verify_message(&other_address, &signed, "hello").expect("verify");
        assert!(!verified);
    }

    #[test]
    fn verify_accepts_bitcoin_core_uncompressed_header() {
        let (signer, address) = test_signer_and_p2pkh();
        let hash = bip137_hash("interop test");
        let (rec_id, compact) = signer.sign_recoverable(&hash).expect("sign recoverable");
        let secp = Secp256k1::new();
        let recovered = secp
            .recover_ecdsa(
                &Message::from_digest(hash),
                &RecoverableSignature::from_compact(&compact, rec_id).unwrap(),
            )
            .expect("recover");
        let header_uncompressed = HEADER_OFFSET_UNCOMPRESSED + rec_id.to_i32() as u8;
        let mut blob = [0u8; SIGNATURE_LEN];
        blob[0] = header_uncompressed;
        blob[1..].copy_from_slice(&compact);
        let b64 = base64::engine::general_purpose::STANDARD.encode(blob);
        let signed = SignedMessage(b64);
        let verified = verify_message(&address, &signed, "interop test").expect("verify");
        assert!(verified);
        assert!(!recovered.serialize().is_empty());
    }

    #[test]
    fn verify_rejects_invalid_header_byte() {
        let mut blob = [0u8; SIGNATURE_LEN];
        blob[0] = 0;
        let b64 = base64::engine::general_purpose::STANDARD.encode(blob);
        let err = SignedMessage::from_str(&b64).expect_err("must reject");
        assert!(matches!(err, Error::Bip137(_)));
        assert!(err.to_string().contains("invalid header byte"));
    }

    #[test]
    fn verify_rejects_invalid_base64() {
        let err = SignedMessage::from_str("not-valid-base64!!!").expect_err("must reject");
        assert!(matches!(err, Error::Bip137(_)));
        assert!(err.to_string().contains("invalid base64"));
    }

    #[test]
    fn verify_rejects_truncated_signature() {
        let truncated = base64::engine::general_purpose::STANDARD.encode([0x1f, 0, 0, 0]);
        let err = SignedMessage::from_str(&truncated).expect_err("must reject");
        assert!(matches!(err, Error::Bip137(_)));
        assert!(err.to_string().contains("length"));
    }

    #[test]
    fn sign_message_rejects_non_p2pkh_address() {
        let sk_bytes = [0x42u8; 32];
        let secp = bdk_wallet::bitcoin::secp256k1::Secp256k1::new();
        let sk = bdk_wallet::bitcoin::secp256k1::SecretKey::from_slice(&sk_bytes).unwrap();
        let pk = PublicKey::from_secret_key(&secp, &sk);
        let compressed = bdk_wallet::bitcoin::CompressedPublicKey(pk);
        let segwit_address = Address::p2wpkh(&compressed, Network::Testnet);
        let signer = Signer::from_secret_bytes(Secret::new(sk_bytes.to_vec()));
        let err = sign_message("hi", &signer, &segwit_address).expect_err("must reject segwit");
        assert!(matches!(err, Error::Bip137(_)));
        assert!(err.to_string().contains("not P2PKH"));
    }

    #[test]
    fn sign_message_empty_message() {
        let (signer, address) = test_signer_and_p2pkh();
        let signed = sign_message("", &signer, &address).expect("sign empty");
        let verified = verify_message(&address, &signed, "").expect("verify");
        assert!(verified);
    }

    #[test]
    fn sign_message_unicode_message() {
        let (signer, address) = test_signer_and_p2pkh();
        let msg = "héllo 🌍";
        let signed = sign_message(msg, &signer, &address).expect("sign unicode");
        let verified = verify_message(&address, &signed, msg).expect("verify");
        assert!(verified);
    }

    #[test]
    fn signed_message_debug_shows_base64() {
        let (signer, address) = test_signer_and_p2pkh();
        let signed = sign_message("hello", &signer, &address).expect("sign");
        let dbg = format!("{signed:?}");
        assert!(dbg.contains("SignedMessage"));
        assert!(dbg.contains(&signed.0));
    }

    #[test]
    fn signed_message_display_and_fromstr_roundtrip() {
        let (signer, address) = test_signer_and_p2pkh();
        let signed = sign_message("hello", &signer, &address).expect("sign");
        let displayed = format!("{signed}");
        let parsed: SignedMessage = displayed.parse().expect("parse fromstr");
        assert_eq!(signed.0, parsed.0);
        let verified = verify_message(&address, &parsed, "hello").expect("verify");
        assert!(verified);
    }
}
