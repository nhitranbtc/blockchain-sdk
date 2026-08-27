# TRON spike V1–V10 (Issue #403)

Verification harness for the open questions from PR #402 (TRON Rust SDK deep-dive) and
Issue #399 (TRON wallet roadmap). Lives in the `rust-wallet-app` umbrella workspace per
plan §File Structure; production code lives in `crates/tron-wallet-core/`.

Each Vn ties to one open question (Q1–Q10) and produces PASS evidence in [RESULT.md](RESULT.md).

| Vn | Q | What it proves |
|----|---|----------------|
| V1 | Q1 | `cargo build -p tron-v1-spike` passes (workspace deps `prost` 0.14.4, `prost-types` 0.14.4, `bs58` 0.5, `tiny-keccak` 2.0.2 all resolve) |
| V2 | Q2 | `prost-build` compiles pinned `core/Tron.proto` (SHA `851575d`) → Rust types; round-trip encode/decode on representative top-level messages (`AccountId`, `Transaction::Raw`); `txID = SHA-256(protobuf-serialize(raw_data))` |
| V3 | Q3 | Hand-rolled `transfer(address,uint256)` calldata = 68 bytes; canonical selectors `0xa9059cbb`, `0x70a08231`, `0x313ce567` match keccak-256 of signatures |
| V4 | Q4 | Keccak-256 + base58check T-address derivation; `0x41` prefix universal; 34-char `T…` string + decode round-trip |
| V5 | Q5 | Nile `triggerconstantcontract` `energy_used` ∈ [50k, 150k] for USDT-TRC20 `decimals()` — confirms Stake 2.0 / DEM model (GATED on `RUN_TRON_NILE=1`) |
| V6 | Q6 | Nile chain-id `0xcd8690dc` via `POST /jsonrpc eth_chainId`; `walletsolidity/getnowblock` for TAPOS (GATED) |
| V7 | Q7 | `bitcoin_wallet_core::chain::spki` SPKI primitives (`SpkiPin` + `SpkiPinSet`) reusable; `pinned://<64-hex-spki>@host[:port]` URL parser; live TLS pin verify gated on Cloudflare rotation (~30 day cadence) |
| V8 | Q8 | k256 ECDSA sign + recovery byte ∈ {0, 1} (NOT Ethereum `v+27`); 65-byte canonical `r‖s‖v` form |
| V9 | Q9 | Bundled `tokens/{mainnet,nile}.json` load (5 + 1 entries); USDT mainnet `decimals = 6`; on-chain `decimals()` verify (GATED) |
| V10 | Q10 | SLIP-44 coin type 195 = TRX; `m/44'/195'/0'/0/0` from canonical "abandon ×11 + about" BIP-39 mnemonic → 34-char `T…` address |

## Run

### Offline (V1, V2, V3, V4, V7, V8, V9-offline, V10)

```bash
cargo test -p tron-v1-spike --tests
```

CI-friendly. No network, no API keys. **36 tests** in 11 binaries all PASS.

### Live (V5, V6, V7-live, V9-live) — operator-driven per L29

```bash
RUN_TRON_NILE=1 cargo test -p tron-v1-spike --tests
```

Requires outbound HTTPS to `https://nile.trongrid.io`. Optional `TRON-PRO-API-KEY` env var
for higher rate limits (not needed for these tests).

### CI build deps

`protoc ≥ 3.12` must be in PATH at build time (CI install dep). On Debian/Ubuntu:
`apt-get install -y protobuf-compiler`. On macOS: `brew install protobuf`.

Verified locally: `protoc 3.21.12` (libprotoc 3.21.12).

## Drift notes (vs plan §File Structure / Issue #403)

- **V2 proto types:** plan referenced `TransferContract` + `TriggerSmartContract` for
  roundtrip tests, but those types live in separate `core/contract/*.proto` files outside
  the vendored Tron.proto. Spike only vendors `Tron.proto` + its `core/Discover.proto` and
  `core/contract/common.proto` imports; full contract proto tree is production-only.
  V2 exercises the roundtrip on `AccountId` + `Transaction::Raw` (top-level messages).
- **V7 SPKI surface:** plan called the surface `SpkiPinnedVerifier`; actual public symbols
  are `SpkiPin` + `SpkiPinSet` (F20 typed primitives). `EsploraVerifier` (the
  `rustls::ServerCertVerifier` impl) is private. Spike uses the public surface; production
  constructs `EsploraVerifier` from `SpkiPinSet`.
- **V7 pin encoding:** `pinned://` URL uses 64-char hex SPKI SHA-256; `SpkiPin::from_str`
  parses base64. The spike adds `pin_set_from_hex` to bridge the URL format to
  `SpkiPin::from_bytes([u8; 32])`.
- **bip32 0.5 API:** `XPrv::to_keypair()` doesn't exist. Use `xprv.public_key().public_key()`
  for the k256 `VerifyingKey`.

## File map

```
spikes/tron-v1/
├── Cargo.toml                                # spike crate manifest (workspace member)
├── build.rs                                  # prost-build compiles proto/core/Tron.proto
├── proto/core/
│   ├── Tron.proto                            # vendored SHA 851575d (2026-07-14)
│   ├── Discover.proto                        # imported by Tron.proto
│   └── contract/common.proto                 # imported by Tron.proto
├── src/
│   ├── lib.rs                                # module root
│   ├── keccak.rs                             # tiny-keccak wrapper (Q3/Q4)
│   ├── base58check.rs                        # bs58 + 4-byte SHA-256d checksum (Q4)
│   ├── address.rs                            # T-base58check address derivation (Q4)
│   ├── protobuf.rs                           # tx_id = SHA-256(encode(raw_data)) (Q2)
│   ├── abi.rs                                # TRC-20 ABI encoder (Q3)
│   ├── rpc.rs                                # JSON-RPC types (Q6)
│   └── spki.rs                               # SPKI pin URL parser (Q7)
├── tokens/
│   ├── mainnet.json                          # 5 TRC-20 entries (Q9)
│   └── nile.json                             # 1 community test USDT
└── tests/
    └── v{1..10}_*.rs                         # 10 verification sub-tests
```
