# L20 Constant Audit — bitcoin-wallet-core v0.2

> **Issue**: #30 — Backlog: L20 constant audit — sweep compile-time constants for out-of-range risk
> **Date**: 2026-08-09
> **Branch**: `task/30-l20-constant-audit`
> **Scope**: `rust-wallet-app/crates/bitcoin-wallet-core/src/crypto/{argon2,aes_gcm,bip137,mod}.rs`

## Goal

Replace every hand-checked `pub const` with a compile-time invariant so that
changing a literal to an out-of-range value fails the build (`const _: ()`)
instead of producing a runtime panic inside `Params::new`, `Key::from_slice`,
or a silently insecure default.

Pattern discovered during Task 5 (#16) L12 review — clippy flagged
`clippy::assertions_on_constants` on Argon2 parameters. The fix (const-eval
block at module scope) generalises to every crypto module.

## Audit findings

13 `pub const` evaluated; 3 were already covered; 10 gained new or extended
compile-time guards.

| # | Module | Constant | Pre-audit | Post-audit | Source of truth |
|---|--------|----------|-----------|------------|-----------------|
| 1 | argon2 | `ARGON2_M_COST_KIB` | asserted | asserted (> 0) | parameter sanity |
| 2 | argon2 | `ARGON2_T_COST` | asserted | asserted (≥ 1) | Argon2 spec |
| 3 | argon2 | `ARGON2_P_COST` | asserted | asserted (≥ 1) | Argon2 spec |
| 4 | argon2 | `SALT_LEN` | **uncovered** | **asserted (≥ 4)** | RFC 9106 §3.1 |
| 5 | argon2 | `DERIVED_KEY_LEN` | partial (≥ 4) | partial (≥ 4) + cross-check | FIPS 197 + crypto::mod |
| 6 | aes_gcm | `NONCE_LEN` | **uncovered** | **asserted (= 12)** | NIST SP 800-38D §5.2.1.1 |
| 7 | aes_gcm | `TAG_LEN` | **uncovered** | **asserted (= 16)** | NIST SP 800-38D §5.2.1.2 |
| 8 | aes_gcm | `KEY_LEN` | **uncovered** | **asserted (= 32)** | FIPS 197 |
| 9 | aes_gcm | `KEY_LEN ↔ DERIVED_KEY_LEN` | **uncovered** | **asserted (==)** | cross-module invariant |
| 10 | bip137 | `MAGIC_PREFIX` | type-enforced `[u8; 25]` | unchanged | BIP-137 spec (length) |
| 11 | bip137 | `HEADER_OFFSET_COMPRESSED` | derived | asserted ( > UNCOMPRESSED) | BIP-137 (split: 27..=30 vs 31..=34) |
| 12 | bip137 | `HEADER_OFFSET_UNCOMPRESSED` | derived | unchanged | BIP-137 |
| 13 | bip137 | `SIGNATURE_LEN` | **uncovered** | **asserted (= 65)** | BIP-137 |
| 14 | bip137 | `COMPACT_SIG_LEN` | **uncovered** | **asserted (= 64)** | BIP-137 |
| 15 | bip137 | `SIGNATURE_LEN == COMPACT_SIG_LEN + 1` | **uncovered** | **asserted** | derived invariant |

Out-of-scope: `keys::derivation::Bip32Path::purpose() const fn` — derived from
enum variant, not hand-checked; cannot be edited without changing the enum.

## Cross-module invariant

`crypto::mod::const _: ()` asserts `DERIVED_KEY_LEN == KEY_LEN` across module
boundaries. If either side drifts to a different value, the build breaks here
before any code calls `Key::from_slice` with a wrong-length key (which would
panic at runtime — DoS vector).

```rust
// rust-wallet-app/crates/bitcoin-wallet-core/src/crypto/mod.rs
const _: () = {
    assert!(
        crate::crypto::argon2::DERIVED_KEY_LEN == crate::crypto::aes_gcm::KEY_LEN,
        "DERIVED_KEY_LEN (argon2) must equal KEY_LEN (AES-256) — both must be 32 bytes"
    );
};
```

## Verification

```text
$ cargo build -p bitcoin-wallet-core
   Compiling bitcoin-wallet-core v0.2.0
    Finished `dev` profile in 11.39s

$ cargo test -p bitcoin-wallet-core
test result: ok. 149 passed; 0 failed; 2 ignored; 0 measured
```

All const asserts pass at compile time; all existing unit/integration tests
pass at runtime. No behaviour change (const-eval blocks generate zero code
when the assertion holds).

## Out of scope

- `keys::derivation` — only `purpose()` const fn; no hand-checked literals
- `chain::esplora`, `chain::spki`, `chain::config` — no hand-checked pub
  consts surfaced (test fixtures `ZEROS_HASH_B64` / `ZEROS_BYTES_B64` are
  private, embedded base64, not a bounds risk)
- `error`, `keys::mnemonic`, `keys::secret`, `keys::signer`, `wallet`,
  `script`, `address`, `util::*` — no hand-checked `pub const` outside
  the crypto layer

## Risk

**Low** — purely additive (const _ blocks). No signature change. No runtime
overhead. If a literal is later changed to an OOR value, the build fails with
a self-explanatory message that cites the spec (NIST / FIPS / RFC 9106 /
BIP-137).

## Follow-ups

None for L20 itself. Related backlog items:

- #35 reject userinfo in Esplora URL (security hardening, separate audit lane)
- #36 EsploraUrl newtype (typed URL validation, not a constant audit concern)
- #37 require SPKI pin on non-regtest (TLS pinning policy, separate audit lane)

None of these are touched by this PR.
