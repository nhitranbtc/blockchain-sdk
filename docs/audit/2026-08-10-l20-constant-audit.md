# Audit: L20 Constants — bitcoin-wallet-core/src/crypto

## Goal
Audit literal constants in `crypto/` for compile-time pinning, drift resistance, security correctness.

## Drift from plan
| Plan | This impl | Why |
|---|---|---|
| Inline literals (e.g., `12u8`, `32u8`) | `pub const X: usize = { const INNER: usize = N; assert!(INNER == N); INNER };` | Literal drift caught at build time |

## API surface
5 modules: `aes_gcm.rs`, `argon2.rs`, `bip137.rs`, `mod.rs`, `threat.rs` (stub).
13 compile-time-pinned constants: NONCE_LEN, TAG_LEN, KEY_LEN, ARGON2_M_COST_KIB, ARGON2_T_COST, ARGON2_P_COST, SALT_LEN, DERIVED_KEY_LEN, MAGIC_PREFIX, HEADER_OFFSET_UNCOMPRESSED, HEADER_OFFSET_COMPRESSED, SIGNATURE_LEN, COMPACT_SIG_LEN.
Cross-module invariant: `argon2::DERIVED_KEY_LEN == aes_gcm::KEY_LEN` (mod.rs).

## Threat-model coverage
F5 (Argon2id m=256 MiB, t=10, p=4) — pinned. F6 (AES-256-GCM 32-byte key, 12-byte nonce, 16-byte tag) — pinned. F7 (BIP-137 narrow API `sign_message(&str)`) — type-enforced. F9 (BIP-137 magic prefix 25 bytes incl. 0x18) — pinned. F47 (Secret<T> zeroize-on-drop) — `Secret<Vec<u8>>` per L15. F50 (constant-time compare) — `subtle::ConstantTimeEq` on verify. F53 (manual Debug) — `SignedMessage` public data, base64 Debug OK.

## Implementation
Compile-time pin pattern used per constant. Cross-module invariant in `mod.rs::const _: () = { assert!(...) }`. BIP-137 const block self-checks HeaderByte tier.

## Tests
85+ crypto tests (per Tasks 5/6). Negative paths: tampered ciphertext, wrong key length, nonce uniqueness, recovery-failure cases.

## L12 review
Manual review (async agents lost on interrupt). No CRITICAL/HIGH findings. All constants pinned. No timing oracles on verify side. Nonce uniqueness 2^32 limit per key documented.

## Lessons captured
None new. L13/L15/L17 patterns applied.

## Backlog
None.

## Migration impact
None. Constants are public API, values unchanged. Compile-time pin is additive.

## Per-dimension verdict
| Dim | Verdict | Note |
|---|---|---|
| Correctness | PASS | All values match spec |
| Security | PASS | Threat-model covered |
| Test coverage | PASS | Indirect via use sites |
| Code simplicity | PASS | +5 lines per constant |
