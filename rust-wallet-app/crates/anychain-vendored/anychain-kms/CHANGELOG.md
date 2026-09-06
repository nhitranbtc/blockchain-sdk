# CHANGELOG — anychain-kms (vendored)

Local patches layered on top of vendored copy. Upstream tag `v0.1.23` does not
exist on the upstream remote; vendored copy is sourced from
`0xcregis/anychain @ cf3aa2d` (see `SOURCE.md`).

## 2026-09-06 local patches

### Zeroizing gap fix in `secp256k1_sign` (Risk Register #3 + plan Task 0.7)

- **What:** in `src/lib.rs::secp256k1_sign`, the raw `sk: &[u8]` is now copied
  into a `Zeroizing<[u8; 32]>` buffer before being passed to
  `libsecp256k1::SecretKey::parse_slice`. `Zeroizing::Drop` overwrites the
  buffer on scope exit. Added `use zeroize::Zeroizing;` at the top of `lib.rs`.
- **Why:** upstream holds the secret in a plain `let sk = ...` binding with no
  hygiene, so the raw bytes remain on the stack (and in any internal
  `libsecp256k1` scratch) past function return. Caller-side `Zeroizing<Vec<u8>>`
  in `tron-wallet-core` is belt-and-suspenders.
- **Test:** regression coverage lives at the caller side
  (`tron-wallet-core::tx::sign::sign_tx` already wraps `sk` in `Zeroizing`).
  Internal kms hygiene is opaque to integration tests, so this entry stands as
  the patch record only.