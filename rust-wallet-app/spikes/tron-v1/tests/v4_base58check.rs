//! V4 — base58check T-prefix address (Q4).
//!
//! Plan §Q4: `keccak256(pubkey_uncompressed[1..65])` → take last 20 bytes →
//! `[0x41] ++ last_20_bytes` = 21-byte raw → base58check → 34-char T... string.
//! Decode round-trips. `0x41` prefix universal across mainnet/Shasta/Nile.

use tron_v1_spike::address::{
    from_base58check, raw_21_from_uncompressed_pubkey, to_base58check, PREFIX_MAINNET,
};
use tron_v1_spike::base58check::{decode as bc_decode, encode as bc_encode};

#[test]
fn v4_base58check_roundtrip() {
    let payload = [0x41u8, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    let s = bc_encode(&payload);
    let decoded = bc_decode(&s).unwrap();
    assert_eq!(decoded, payload);
}

#[test]
fn v4_base58check_rejects_bad_checksum() {
    let payload = [0x41u8, 0x01, 0x02, 0x03];
    let mut s = bc_encode(&payload);
    // Flip a character to break the checksum.
    let last = s.pop().unwrap();
    let replacement = if last == 'A' { 'B' } else { 'A' };
    s.push(replacement);
    assert!(bc_decode(&s).is_err());
}

#[test]
fn v4_address_starts_with_t_and_is_34_chars() {
    // Build a synthetic pubkey (0x04 + 64 bytes). Keccak-256 over [1..65] is deterministic;
    // the resulting base58check string must start with `T` and be 34 chars long.
    let mut pubkey = [0u8; 65];
    pubkey[0] = 0x04;
    for (i, slot) in pubkey.iter_mut().enumerate().skip(1) {
        *slot = i as u8;
    }
    let raw = raw_21_from_uncompressed_pubkey(&pubkey);
    assert_eq!(raw[0], PREFIX_MAINNET);

    let s = to_base58check(&raw);
    assert!(s.starts_with('T'), "address must start with T: {s}");
    assert_eq!(s.len(), 34, "base58check T-address must be 34 chars: {s}");
}

#[test]
fn v4_address_decode_roundtrip() {
    let mut pubkey = [0u8; 65];
    pubkey[0] = 0x04;
    pubkey[1] = 0x42;
    let raw = raw_21_from_uncompressed_pubkey(&pubkey);
    let s = to_base58check(&raw);
    let raw_back = from_base58check(&s).unwrap();
    assert_eq!(raw, raw_back);
}

#[test]
fn v4_address_decode_rejects_wrong_prefix() {
    // 21 bytes but starting with 0x00 (BTC mainnet prefix), not 0x41.
    let raw = [0x00u8; 21];
    let s = bc_encode(&raw);
    assert_eq!(
        from_base58check(&s),
        Err(tron_v1_spike::address::AddressError::WrongPrefix)
    );
}

#[test]
fn v4_keccak256_known_vector() {
    // Keccak-256 of empty string = c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
    use tron_v1_spike::keccak::keccak256;
    let h = keccak256(b"");
    assert_eq!(
        hex::encode(h),
        "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    );
}
