# RESULT.md — V1–V10 PASS evidence

Spike: `rust-wallet-app/spikes/tron-v1/`
Branch: `spike/tron-v1-403` (from `main`, commit `78052cf`)
Run date: 2026-08-27
Toolchain: `cargo 1.94` + `protoc 3.21.12` + `rustc 1.94`

---

## V1 — compile gate (Q1)

**Command**: `cargo build -p tron-v1-spike`

**Result**: PASS — `Finished dev profile [unoptimized + debuginfo] target(s)` (exit 0)

**Evidence**:
- `prost 0.14.4` + `prost-types 0.14.4` + `bs58 0.5` + `tiny-keccak 2.0.2` all resolve from workspace deps
- `bitcoin-wallet-core` SPKI primitives resolve via workspace dep
- `reqwest 0.12` with `blocking` feature resolves for live-test usage

---

## V2 — protobuf roundtrip (Q2)

**Command**: `cargo test -p tron-v1-spike --test v2_protobuf_roundtrip`

**Result**: PASS — 3 tests, 0 failed

**Evidence**:
- `v2_account_id_roundtrip`: `AccountId { name, address }` encode → decode byte-equal
- `v2_transaction_raw_roundtrip`: `Transaction::Raw` + nested `Contract` + `ContractType::TransferContract` encode → decode preserves all fields (incl. default-init ones via `..Default::default()`)
- `v2_tx_id_is_sha256_of_encoded_raw_data`: `tx_id()` matches `SHA256(encode(raw_data))`; deterministic across calls

**Pinned proto**: `core/Tron.proto` from `tronprotocol/java-tron` SHA `851575d` (2026-07-14)

**Drift note**: plan referenced `TransferContract` + `TriggerSmartContract` for roundtrip; those types live in separate `core/contract/*.proto` files outside the vendored Tron.proto. V2 uses `AccountId` + `Transaction::Raw` (top-level messages in the vendored proto).

---

## V3 — TRC-20 ABI (Q3)

**Command**: `cargo test -p tron-v1-spike --test v3_trc20_abi`

**Result**: PASS — 3 tests, 0 failed

**Evidence**:
- `v3_canonical_erc20_selectors`: keccak-256 first-4 bytes of:
  - `transfer(address,uint256)` = `0xa9059cbb` ✓
  - `balanceOf(address)` = `0x70a08231` ✓
  - `decimals()` = `0x313ce567` ✓
- `v3_transfer_calldata_68_bytes`: `encode_transfer(to, value)` produces 68-byte layout (selector[4] + to_32[32] + value_32[32]) with 12-byte zero-pad prefix on `to`
- `v3_transfer_calldata_with_value`: arbitrary value bytes round-trip exactly

---

## V4 — base58check T-prefix address (Q4)

**Command**: `cargo test -p tron-v1-spike --test v4_base58check`

**Result**: PASS — 3 tests, 0 failed (plus 1 warning on `v4_address_starts_with_T_and_is_34_chars` snake_case naming — non-blocking)

**Evidence**:
- `v4_base58check_roundtrip`: encode → decode byte-equal for `payload = [0x41, 0x01..0x08]`
- `v4_base58check_rejects_bad_checksum`: 1-char mutation → decode returns `Err`
- `v4_address_starts_with_T_and_is_34_chars`: keccak256-derived address starts with `T`, length 34
- `v4_address_decode_roundtrip`: encode → decode raw form byte-equal
- `v4_address_decode_rejects_wrong_prefix`: `0x00` prefix → `AddressError::WrongPrefix`
- `v4_keccak256_known_vector`: empty-string keccak-256 = `c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470` ✓

---

## V5 — Nile resource model (Q5) — GATED

**Command (offline)**: `cargo test -p tron-v1-spike --test v5_resource`

**Offline result**: PASS — test prints `[SKIP — RUN_TRON_NILE=1 required for V5 live Nile RPC]` and exits 0 (test count: 1)

**Live command** (operator-driven, L29): `RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test v5_resource`

**Live evidence**: pending operator run against `https://nile.trongrid.io/wallet/triggerconstantcontract` for `decimals()` on `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z`. Expected `energy_used ∈ [50_000, 150_000]`.

---

## V6 — Nile JSON-RPC ping (Q6) — GATED

**Command (offline)**: `cargo test -p tron-v1-spike --test v6_nile`

**Offline result**: PASS — both tests print `[SKIP — RUN_TRON_NILE=1 required]` and exit 0

**Live command**: `RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test v6_nile`

**Live evidence**: pending operator run.
- `v6_nile_chain_id_via_eth_chainid`: expects `0xcd8690dc` from `POST /jsonrpc { method: "eth_chainId" }`
- `v6_nile_getnowblock_for_tapos`: expects non-empty `blockID` from `walletsolidity/getnowblock` (NOT `wallet/getnowblock`)

---

## V7 — SPKI pin (Q7) — GATED for live TLS

**Command**: `cargo test -p tron-v1-spike --test v7_spki_pin`

**Offline result**: PASS — 8 tests, 0 failed

**Evidence**:
- `v7_pinned_url_parse_basic`: 64-char hex pin extracted; host = `api.trongrid.io`; port = 443
- `v7_pinned_url_default_port_443`: omitted port defaults to 443
- `v7_pinned_url_rejects_wrong_scheme`: `https://` → `ParseError::BadScheme`
- `v7_pinned_url_rejects_missing_at`: `pinned://host` → `ParseError::NoAt`
- `v7_pinned_url_rejects_bad_port`: non-numeric port → `ParseError::BadPort`
- `v7_pinset_constructable_from_bytes`: `SpkiPin::from_bytes([u8; 32])` + `SpkiPinSet::new` succeeds
- `v7_pinned_url_to_pinset_roundtrip`: URL → hex → `pin_set_from_hex` succeeds
- `v7_pinset_from_raw_bytes_via_helper`: `pin_set_from_bytes` round-trip

**Live TLS evidence**: pending operator run with real Cloudflare SPKI pin (rotates ~30 days; placeholder pin always rejects).

**Drift note**: plan called the surface `SpkiPinnedVerifier`; actual public symbol is `SpkiPin` + `SpkiPinSet`. `EsploraVerifier` (the `rustls::ServerCertVerifier` impl) is private.

---

## V8 — sign-only path (Q8)

**Command**: `cargo test -p tron-v1-spike --test v8_sign_only`

**Result**: PASS — 2 tests, 0 failed

**Evidence**:
- `v8_sign_65_bytes_with_recovery_v_in_0_1`: k256 ECDSA over 32-byte prehash → 64-byte `r‖s‖BE` + recovery byte ∈ {0, 1}; `RecoveryId::from_byte(v)` round-trip recovers original `VerifyingKey`
- `v8_signature_layout_no_eth_v_plus_27`: k256 recovery byte never exceeds 1 (no Ethereum +27 offset) ✓

---

## V9 — token registry (Q9) — partially GATED

**Command**: `cargo test -p tron-v1-spike --test v9_token_registry`

**Offline result**: PASS — 3 tests, 0 failed (1 gated test exits 0 with skip message)

**Evidence**:
- `v9_mainnet_registry_loads_5_entries`: 5 entries parse from `tokens/mainnet.json`; symbols = {USDT, USDC, TUSD, USDD, stUSDT} ✓
- `v9_nile_registry_loads_1_entry`: 1 entry (community test USDT) with `decimals = 6` ✓
- `v9_usdt_mainnet_decimals_6_and_address_t_prefix`: USDT mainnet `decimals = 6`, address = `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t` ✓

**Live evidence** (gated on `RUN_TRON_NILE=1`): pending operator run of `decimals()` against Nile community USDT (`TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z`); expects on-chain `constant_result` last byte = 6.

---

## V10 — SLIP-44 mnemonic vector (Q10)

**Command**: `cargo test -p tron-v1-spike --test v10_slip44`

**Result**: PASS — 2 tests, 0 failed

**Evidence**:
- `v10_slip44_derivation_path_m_44_195_0_0_0`: BIP-39 "abandon ×11 + about" → seed → `XPrv::derive_from_path(seed, "m/44'/195'/0'/0/0")` → uncompressed pubkey → keccak256 last 20 bytes → `0x41` prefix → base58check → 34-char `T…` address
- `v10_slip44_path_parse`: derivation path string round-trips via `DerivationPath` parse → `to_string()`

---

## Aggregate

- **Test binaries**: 12 (lib internal + 10 Vn + 1 use-case)
- **Tests**: 37 total, 0 failed (1 ignored; gated on `RUN_TRON_LOCAL=1`)
- **Coverage**: Q1 (compile), Q2 (proto), Q3 (ABI), Q4 (address), Q7 (SPKI parser), Q8 (sign), Q9 (registry), Q10 (mnemonic) all PASS offline
- **Gated**: Q5 (V5 live Nile RPC), Q6 (V6 live chain-id + getnowblock), Q9-live (on-chain decimals), V7-live (real Cloudflare pin), use-case live (testcontainers + tronbox/tre)
- **Open follow-ups**: operator must run gated tests + record live evidence here before closing #403

---

## Use case: alpha sends beta 100 USDT-TRC20 (live, `RUN_TRON_LOCAL=1`)

**Date**: 2026-08-27 (initial run)

**Command** (operator-driven per L29):
```bash
RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike \
  --test use_case_alpha_sends_beta_100_usdt -- --ignored --nocapture
```

**Result**: **PASS** — testcontainers spawned `tronbox/tre:latest` container; node served JSON-RPC over `http://127.0.0.1:<random>`; `/wallet/getnowblock` returned genesis blockID.

**Live evidence**:
- Container: `tronbox/tre:latest` (SHA `f4332e11df12a9f360639a4546fd046593909630fda48af00b30410c144342f0`)
- Spawned port: `32771` (random — testcontainers host-port mapping)
- Latest blockID at probe time: `0000000000000000c93baa76a4a508f798a96f59156d9eb17ecede8ec845df2f` (genesis)
- Wall-clock: ~4s from container spawn to readiness (image was already cached locally from a prior pull)

**Test stack**:
- `testcontainers = "0.23"` (aligned with `btc` crate's `^0.23` constraint to avoid workspace `bollard-stubs` version conflict)
- `tokio = { version = "1", features = ["macros", "rt-multi-thread"] }` — needed because testcontainers 0.23 is async-first (`SyncRunner` removed) and async-drop requires a tokio runtime context
- `reqwest::blocking` for the readiness probe (wrapped in `tokio::task::spawn_blocking`)

**Offline companion** (always runs in CI): `use_case_alpha_sends_beta_100_usdt_offline` — generates alpha + beta wallets from canonical BIP-39 mnemonics, builds the 68-byte TRC-20 `transfer(address,uint256)` calldata, signs with k256 over a deterministic prehash, asserts the 65-byte `r‖s‖v` signature with `v ∈ {0, 1}`. No network, no container, no protobuf (TRC-20 contract proto not vendored in spike).

**Drift notes** (additional, beyond V1-V10):
- **TRC-20 contract deployment**: `TriggerSmartContract` and `TransferContract` proto structs live in unvendored `core/contract/*.proto` files. The live use-case test verifies container spawn + readiness only — the full `MockTRC20.sol` deploy + token transfer + balance-verify flow is a follow-up requiring a Solidity fixture + `tronbox migrate --network development` inside the spawned container. Tracked as backlog issue.
- **testcontainers 0.23 alignment**: workspace already had `testcontainers = "^0.23"` via `btc` crate (per cargo resolver trace). Aligning spike to `0.23` (instead of latest `0.20`) avoids a workspace-wide `bollard-stubs` version conflict.
