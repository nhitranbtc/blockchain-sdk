# TRON spike V1–V10 + use-case (Issues #403 / #409)

Verification harness for the open questions from PR #402 (TRON Rust SDK deep-dive) and
Issue #399 (TRON wallet roadmap). Lives in the `rust-wallet-app` umbrella workspace per
plan §File Structure; production code lives in `crates/tron-wallet-core/`.

Each Vn ties to one open question (Q1–Q10); see the V-table below for what each proves.

| Vn | Q | What it proves |
|----|---|----------------|
| V1 | Q1 | `cargo build -p tron-v1-spike` passes (workspace deps `prost` 0.14.4, `prost-types` 0.14.4, `bs58` 0.5, `tiny-keccak` 2.0.2 all resolve) |
| V2 | Q2 | `prost-build` compiles pinned `core/Tron.proto` (SHA `851575d`) → Rust types; round-trip encode/decode on representative top-level messages (`AccountId`, `Transaction::Raw`); `txID = SHA-256(protobuf-serialize(raw_data))` |
| V3 | Q3 | Hand-rolled `transfer(address,uint256)` calldata = 68 bytes; canonical selectors `0xa9059cbb`, `0x70a08231`, `0x313ce567` match keccak-256 of signatures |
| V4 | Q4 | Keccak-256 + base58check T-address derivation; `0x41` prefix universal; 34-char `T…` string + decode round-trip |
| V5 | Q5 | Nile `triggerconstantcontract` `energy_used` ∈ [50k, 150k] for USDT-TRC20 `decimals()` — confirms Stake 2.0 / DEM model (GATED on `RUN_TRON_NILE=1`) |
| V6 | Q6 | Nile chain-id `0xcd8690dc` via `POST /jsonrpc eth_chainId`; `walletsolidity/getnowblock` for TAPOS (GATED) |
| V7 | Q7 | `bitcoin_wallet_core::chain::spki` SPKI primitives (`SpkiPin` + `SpkiPinSet`) reusable; `pinned://<64-hex-spki>@host[:port]` URL parser; live TLS pin verify gated on Cloudflare rotation (~30 day cadence). **Honest gap:** the pin is parsed and recorded on `JsonRpcClient`, but `post_*` helpers currently use Rustls default verification — wiring the SPKI verifier into a custom reqwest `ClientBuilder` is the #408 ship-gate follow-up. |
| V8 | Q8 | k256 ECDSA sign + recovery byte ∈ {0, 1} (NOT Ethereum `v+27`); 65-byte canonical `r‖s‖v` form |
| V9 | Q9 | Bundled `tokens/{mainnet,nile}.json` load (5 + 1 entries); USDT mainnet `decimals = 6`; on-chain `decimals()` verify (GATED) |
| V10 | Q10 | SLIP-44 coin type 195 = TRX; `m/44'/195'/0'/0/0` from canonical "abandon ×11 + about" BIP-39 mnemonic → 34-char `T…` address |
| Use-case | #409 | End-to-end live Nile broadcast: alpha → beta 1 USDT-TRC20. Pulls together Q1-Q10 (build, protobuf, ABI, base58, energy, chain-id, SPKI URL, signature, token registry, address derivation) + 4 wire-format fixes (raw_data field, poll binding, balanceOf shape, balanceOf selector). Last passing tx: [`c69a2105…`](https://nile.tronscan.org/#/transaction/c69a2105cbd0beefc3b5e84fefcec1e41b3011995e21c122feb6a758f86be26f) (recipient holds 7 USDT). |

## Run

### Offline (V1, V2, V3, V4, V7, V8, V9-offline, V10, use-case-offline)

```bash
cargo test -p tron-v1-spike --tests
```

CI-friendly. No network, no API keys. **42 lib + integration tests** PASS (2026-08-27).

### Live — operator-driven per L29

Three live paths, each gated on a distinct env-var set (missing vars → test skips silently):

| Path | Env vars | Purpose |
|---|---|---|
| V5 (energy estimate), V6 (chain-id + TAPOS), V9 (USDT decimals) | `RUN_TRON_NILE=1` | Read-only live calls to `nile.trongrid.io` |
| V7-live (SPKI pin verify) | `RUN_TRON_NILE=1` | Live TLS handshake against `nile.trongrid.io:443` |
| `use_case_alpha_sends_beta_usdt_live_local_node` | `RUN_TRON_LOCAL=1` + Docker `tronbox/tre` image | Spawn local TRON devnet via testcontainers |
| **`use_case_alpha_sends_beta_usdt_live_nile`** | **`TRON_NILE_PRIVATE_KEY` + `TRON_NILE_RECIPIENT_ADDRESS` + `TRON_NILE_SPKI_PIN`** | End-to-end live broadcast (#409) |

Run live paths:

```bash
# V5 / V6 / V7-live / V9-live
RUN_TRON_NILE=1 cargo test -p tron-v1-spike --tests

# Local TRON devnet (requires `docker pull tronbox/tre:latest` first)
RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike --test use_case_alpha_sends_beta_usdt -- --ignored

# Full Nile e2e (operator-only — see "Faucet & test tokens" below for setup)
set -a; source tests/.env; set +a
cargo test -p tron-v1-spike --test use_case_alpha_sends_beta_usdt \
  use_case_alpha_sends_beta_usdt_live_nile -- --ignored --nocapture
```

Requires outbound HTTPS to `https://nile.trongrid.io`. Optional `TRON-PRO-API-KEY` env var
for higher rate limits (not needed for these tests).

### CI build deps

`protoc ≥ 3.12` must be in PATH at build time (CI install dep). On Debian/Ubuntu:
`apt-get install -y protobuf-compiler`. On macOS: `brew install protobuf`.

Verified locally: `protoc 3.21.12` (libprotoc 3.21.12).

## Faucet & test tokens (Nile)

Live tests need TRX (gas) + USDT-TRC20 (token transfer) on a funded address. Both are
free on the Nile testnet; mainnet requires real TRX.

### Fund TRX (gas)

Nile faucet — sign in with GitHub, paste your T-base58check address, claim. The faucet
drips ~5,000 TRX per request with a cooldown. Use the `TRON_NILE_RECIPIENT_ADDRESS`
from `tests/.env` as the destination.

- <https://nileex.io/join/getJoinPage>

### Get test USDT-TRC20

The spike's bundled `tokens/nile.json` pins the community test USDT contract
`TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf` — the same address the faucet dispenses.
Verify on-chain:

- <https://nile.tronscan.org/contract/TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf/code>

The contract owner can mint to any address on request. If your account shows zero
USDT after the TRX drip, query the contract on Tronscan and ask in the
[TronGrid Telegram](https://t.me/Trongrid) — owners usually oblige.

The bundle lives at [`tokens/nile.json`](tokens/nile.json) (single entry, schema mirrors
[`tokens/mainnet.json`](tokens/mainnet.json)). The `TRON_NILE_USDT_TOKEN_ADDRESS` in
`tests/.env` must match the `address` field of that JSON entry.

## Local testnet (testcontainers + tronbox/tre)

The spike ships a separate live path that boots a real local TRON devnet via
the [`testcontainers`](https://crates.io/crates/testcontainers) crate using the
official [`tronbox/tre`](https://hub.docker.com/r/tronbox/tre) Docker image.
This lets operators exercise the V1-V10 stack end-to-end without depending
on Nile uptime or external faucets.

### Why `tronbox/tre` (no Rust alternative)

`tronbox/tre` is the Java + Node [TronBox](https://github.com/tronprotocol/tron-box)
private network — the closest thing to Ethereum's Anvil for TRON. There is
**no pure-Rust TRON devnet today** (per `docs/wallets/2026-08-27-tron-rust-sdks-deep-dive.md`).
The `rust-bitcoin-wallet/v0.1` plan documents this gap and the production
crate (`tron-wallet-core`) inherits the choice.

### One-time setup

```bash
docker pull tronbox/tre:latest
docker images tronbox/tre           # confirm tag + size
```

Required: Docker daemon reachable from the test process. The spike uses the
testcontainers **default provider**, which inspects `DOCKER_HOST` env var
and falls back to the local socket. On macOS Docker Desktop and most CI
hosts this is automatic.

### Run the local test

```bash
RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike \
  --test use_case_alpha_sends_beta_usdt -- --ignored --nocapture
```

### What the test does (4-stage readiness probe)

The `use_case_alpha_sends_beta_usdt_live_local_node` test
(`tests/use_case_alpha_sends_beta_usdt.rs:200`):

1. **Spawns** the container via `testcontainers::runners::AsyncRunner`:

   ```rust
   GenericImage::new("tronbox/tre", "latest")
       .with_exposed_port(9090.tcp())
       .start()
       .await
   ```
2. **Maps host port** via `get_host_port_ipv4(ContainerPort::Tcp(9090))` —
   Docker assigns a random ephemeral port; testcontainers exposes it.
3. **Probes** `/wallet/getnowblock` over plain HTTP on `127.0.0.1:<host_port>`
   in a `spawn_blocking` task (defensive — keeps the tokio runtime free for
   testcontainers' async drop). Retries every 2s with a 180s deadline.
4. **Asserts** `blockID` is non-empty, then drops the container (auto-cleanup
   via testcontainers' async runtime — see "Common pitfalls" below).

### Common pitfalls

| Pitfall | Fix |
|---|---|
| `Docker not found` / `Cannot connect to Docker daemon` | Start Docker Desktop (macOS/Windows) or `systemctl start docker` (Linux). Verify with `docker ps` |
| Test hangs > 180s with no output | `tronbox/tre` cold-start can exceed the readiness deadline on slow hosts. Re-run with `--test-threads=1` to avoid contention, or bump `READY_PROBE_TIMEOUT` in the test source |
| Container dropped before probe completes | testcontainers **requires** `#[tokio::test]` for its async runner; the test uses that. Mixing blocking + async drops breaks cleanup — keep the probe in `spawn_blocking` as written |
| `port 9090 already in use` on host | testcontainers maps to a random ephemeral host port, NOT 9090. The mapped port is what the test uses, not a fixed value |
| `Docker-in-Docker` (CI runners) | Mount `/var/run/docker.sock` or use [sysbox/testcontainers-cloud](https://github.com/testcontainers/testcontainers-cloud). The spike does not currently detect DinD explicitly |
| `bind-mount /tmp` permission errors on Linux | testcontainers writes a temp compose file. Set `TESTCONTAINERS_RYUK_DISABLED=true` if the reaper (ryuk) cannot run — the spike does not depend on it for cleanup, only for orphan reaping |

### Limitations (per the test's docstring)

- **TRC-20 contract deploy + transfer + balance-verify** require running
  `tronbox migrate --network development` inside the container — **not
  implemented** in this spike (backlog issue). The local-node test only
  proves the spike can talk to a TRON fullnode and reach a sane state
  (`blockID != ""`).
- **Only port 9090 exercised.** The full TRON HTTP surface (`/wallet/*`,
  `/walletsolidity/*`) is reachable from the host once the container maps
  the port, but the spike doesn't yet cover `triggersmartcontract`,
  `broadcasttransaction`, or `gettransactionbyid` against the local node.
- **TRC-20 deployment is hand-rolled work.** To turn this into a real local
  e2e, drop a `MockTRC20.sol` fixture into `tests/fixtures/`, add a
  `tronbox migrate` invocation (via `testcontainers::Image::exec` or a
  separate `docker exec` shell-out), then extend the test to call
  `transfer` + `balanceOf` against the local fullnode — no SPKI pin, no
  faucet, no public-network dependency.

## First-time setup (operator checklist)

End-to-end runbook for the `use_case_alpha_sends_beta_usdt_live_nile` path.

### 1. System deps

```bash
# Rust + Cargo (pinned via rust-toolchain.toml at workspace root)
rustup show                              # expect 1.94 stable

# protobuf compiler for `prost-build`
# Debian/Ubuntu:  apt-get install -y protobuf-compiler
# macOS:           brew install protobuf
protoc --version                         # expect ≥ 3.12 (verified 3.21.12)
```

### 2. Generate a sender keypair

The sender T-address must derive from a secp256k1 scalar you control. Use
any tool that derives from BIP-39 mnemonic + SLIP-44 path `m/44'/195'/0'/0/0`:

- **TronWeb:** `tronWeb.fromMnemonic(mnemonic, "m/44'/195'/0'/0/0").privateKey`
- **tronbox:** `const account = await tronWeb.createAccount(); account.privateKey`
- **This spike's test helper:** `tests/use_case_alpha_sends_beta_usdt.rs::fresh_wallet`
  (deterministic with `abandon ×11 + about` mnemonic; never use in production)

Output: 32-byte hex private key + 34-char T address (`T…`).

### 3. Capture SPKI pin for `nile.trongrid.io`

`TRON_NILE_SPKI_PIN` is the SHA-256 of the DER-encoded SubjectPublicKeyInfo of
the fullnode TLS cert, lowercase hex (64 chars). Capture once per cert
rotation (~30-day Cloudflare cadence per V7 row):

```bash
openssl s_client -connect nile.trongrid.io:443 -servername nile.trongrid.io \
  </dev/null 2>/dev/null \
  | openssl x509 -pubkey -noout \
  | openssl pkey -pubin -outform der \
  | openssl dgst -sha256 -binary | xxd -p -c 256
```

The trailing `xxd -p -c 256` emits pure lowercase hex with no `(stdin)=`
prefix. The `-hex` variant shown in `tests/env.example` would need stripping.

### 4. Fund the sender (TRX gas)

Nile faucet — <https://nileex.io/join/getJoinPage>. Sign in with GitHub, paste
the sender T address from step 2, claim. ~5,000 TRX per request, cooldown
applies. Verify on <https://nile.tronscan.org>.

### 5. Get test USDT-TRC20

The faucet dispenses community test USDT to your funded account. If your
balance is zero after the TRX drip, see "Faucet & test tokens" above for the
Telegram fallback.

### 6. Author `tests/.env`

```bash
cp tests/env.example tests/.env
# Edit tests/.env:
#   TRON_NILE_PRIVATE_KEY=<64 hex chars from step 2>
#   TRON_NILE_RECIPIENT_ADDRESS=<T address of any other wallet>
#   TRON_NILE_SPKI_PIN=<64 hex chars from step 3>
#   TRON_NILE_USDT_TOKEN_ADDRESS=TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf
```

`tests/.env` is gitignored via the root `.gitignore`. `tests/env.example` is
the committed template. Never commit `.env`.

### 7. Run the live test

```bash
set -a; source tests/.env; set +a
cargo test -p tron-v1-spike --test use_case_alpha_sends_beta_usdt \
  use_case_alpha_sends_beta_usdt_live_nile -- --ignored --nocapture
```

On success: `[use_case/nile] PASS — https://nile.tronscan.org/#/transaction/<txid>`.

### 8. Cleanup (security hygiene — L54)

```bash
unset TRON_NILE_PRIVATE_KEY TRON_NILE_RECIPIENT_ADDRESS TRON_NILE_SPKI_PIN TRON_NILE_USDT_TOKEN_ADDRESS
```

The privkey in `tests/.env` is operator-controlled and never appears in CI
logs (lib tests + integration tests skip live paths unless the env vars are
explicitly set).

## Live testnet Nile — operator deep-dive

Beyond the bare run command: what each test path asserts, how to debug
failures, and how to interpret the PASS log.

### What `use_case_alpha_sends_beta_usdt_live_nile` does

(`tests/use_case_alpha_sends_beta_usdt.rs:289`)

1. **Derives sender T-address from `TRON_NILE_PRIVATE_KEY`** — uncompressed
   pubkey → keccak256 → last-20-bytes → `0x41` prefix → base58check. The
   signing keypair matches the from-address by construction, so no drift
   between `.env` pair is possible.
2. **Asserts sender ≠ recipient** (sanity check).
3. **Builds + signs** a `TriggerSmartContract` envelope for `transfer(beta, 1 USDT)`
   via `tron_v1_spike::tx::build_signed_trc20_transfer`. The signature covers
   `SHA-256(raw_data_bytes)` (the txID), 65-byte `r‖s‖v` with `v ∈ {0, 1}`.
4. **Broadcasts** via SPKI-pinned RPC. On success, logs the tx_id.
5. **Polls** `/wallet/gettransactioninfobyid` every 3s for up to 120s. Confirms
   `id == tx_id` (request-binding) AND `receipt.result == "SUCCESS"`.
6. **Queries `balanceOf(recipient)`** via `/wallet/triggerconstantcontract`.
   Asserts ≥ 1 USDT (1_000_000 raw, 6-dec).

### Acceptance criteria (mirrors #409)

- `tx_id` appears in the PASS log line
- `[use_case/nile] confirmed after ≤120s`
- `[use_case/nile] recipient balanceOf >= 1000000 raw`

### Debugging failures

| Symptom | Likely cause |
|---|---|
| `panic: sender ≠ recipient` | T-address derivation drift — verify `TRON_NILE_PRIVATE_KEY` decodes to the from-address shown in Tronscan for your account |
| `broadcast tx: malformed` / NPE-shaped | Server-side; check [nile.tronscan.org](https://nile.tronscan.org) for the tx. If absent, server rejected (sender lacks energy/bandwidth — see V5 row for sizing formula) |
| `tx confirmation poll: Timeout` | Tx stuck in mempool — sender likely needs explicit energy staking. Use `getaccount` on Tronscan to see frozen balance |
| `balanceOf query: ...` decode error | TronGrid API contract change — file a spike issue, do not silently downgrade the parser |
| `0 raw` balance | Sender doesn't hold USDT-TRC20 (step 5 of setup) OR `TRON_NILE_USDT_TOKEN_ADDRESS` doesn't match the bundle |

### Reading the PASS log

```text
[use_case/nile] tx_id  = c69a2105cbd0beefc3b5e84fefcec1e41b3011995e21c122feb6a758f86be26f
[use_case/nile] confirmed after ≤120s
[use_case/nile] recipient balanceOf = 7000000 raw (6-dec)
[use_case/nile] PASS — https://nile.tronscan.org/#/transaction/c69a2105...
```

- `tx_id` — SHA-256 of protobuf-serialized `raw_data`, hex-encoded
- `confirmed after ≤120s` — receipt matched `id` + `result=SUCCESS` in <120s
- `balanceOf = N raw` — `N / 1_000_000` USDT (6-dec for USDT-TRC20)
- Last line — direct link to the Tronscan tx page

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

### Additional drift — discovered 2026-08-27 in #409 live-broadcast work

#### Plan assumptions that turned out wrong

- **Q6 chain-id.** Plan §Q6 originally cited Shasta's chain-id `0x94a9059e`.
  Correct Nile value is `0xcd8690dc` (verified live against `nile.trongrid.io`
  via `/jsonrpc eth_chainId`). The comment in `src/rpc.rs:11` carries the
  correction — any reader cross-referencing the plan file should treat
  the plan as stale on this point.
- **Q8 signature convention.** Plan §Q8 says "TRON `v` byte ∈ {0, 1}, NOT
  `v + 27`". The spike honors this (tx.rs:315 + debug_assert). Older
  off-the-shelf Ethereum-style signers may emit `v + 27` and silently
  produce invalid signatures on TRON — verify before reusing code from
  ETH/BSC tooling.
- **`triggersmartcontract` calldata contract.** Plan implied the server
  strips the 4-byte selector and the client sends full calldata. Reality is
  inverted: the server **prepends** the selector to the `parameter` field,
  and the client sends only the encoded args. The spike now strips the
  selector for both `transfer` (tx.rs:354) and `balanceOf` (tx.rs:114);
  initial spike sent full 68/36-byte calldata and got `SUCCESS` from the
  server but an empty return value.
- **`gettransactionbyid` echo.** Plan §"broadcast" implies the by-id
  endpoint echoes the submitted `txID` so the poll loop can confirm
  response-to-request binding. It does not — the response is the bare
  transaction record (`ret[0].contractRet` only). Spike switched to
  `/wallet/gettransactioninfobyid` (receipt endpoint) which DOES echo the
  submitted `id` field. Old path deleted.
- **`balanceOf` response shape.** Plan treated `/wallet/triggerconstantcontract`
  as returning `{ "result": { "result": "<hex>" } }`. TronGrid actually
  returns `{ "constant_result": ["<hex>"], "result": { "result": true } }`
  (the nested `result.result` is a **boolean** success flag, not the
  balance). Spike parser now prefers `constant_result[0]` and falls back to
  `result.result` only when it's a string.

#### Initial spike bugs surfaced by the live test

- **#409 NPE on `/wallet/broadcasttransaction`.** Initial spike dropped the
  structured `raw_data` JSON object from the broadcast body — only `raw_data_hex`,
  `txID`, `signature`, and `visible` survived. Per [BroadcastServlet spec](
  https://github.com/tronprotocol/documentation-en/blob/master/docs/api/http/tx-build-and-broadcast/broadcasttransaction.md)
  the node re-serializes `raw_data` to protobuf for signature verification
  and throws NPE when it's missing. Fixed by parsing `transaction.raw_data`
  in `TriggerSmartResponse` and passing it through into `broadcast_body`.
- **#409 poll timeout.** Initial `tx_visible_in_response` looked for a `txID`
  field in the by-id response (which doesn't exist) and polled indefinitely.
  Fixed by switching to the receipt endpoint and requiring `id == tx_id` +
  `receipt.result == "SUCCESS"` (defense-in-depth, also flagged by automated
  security review).
- **#409 `balanceOf` returns 0.** Two compounding bugs: full 36-byte calldata
  sent (selector + arg, server prepends another selector → wrong call) +
  parser reads the boolean flag instead of the balance hex. Both fixed in
  this session.

#### Security model drift

- **V7 SPKI enforcement gap.** Plan assumed SPKI pin enforcement was on by
  default once the URL parser accepted the pin. Reality (per `src/rpc.rs:121-124`):
  the pin is parsed and **recorded** on `JsonRpcClient`, but the `post_*`
  helpers use Rustls default verification. Wiring the SPKI verifier into
  a custom reqwest `ClientBuilder` is `#408` ship-gate follow-up work —
  the spike's live path currently trusts the system trust store plus the
  URL-pinned endpoint identity (Nile is one well-known host).

#### Stale identifiers

- **V9 USDT contract address.** Plan referenced `TXYZopuvdm45dLTs6eYCeq8Nx6FvF2hU1z`
  which does **not exist** on Nile. The bundle `tokens/nile.json` now pins
  the correct community-test USDT contract `TXYZopYRdj2D9XRtbG411XZZ3kM5VkAeBf`
  — verified live via `triggersmartcontract decimals() = 6` and
  `symbol() = "USDT"`. If you find the old address in any other file, treat
  it as a search-and-replace candidate.
- **Removed test:** `use_case_alpha_sends_beta_100_usdt.rs` was deleted in
  the 2026-08-27 session; it duplicated the renamed
  `use_case_alpha_sends_beta_usdt.rs` (the "100" was a leftover from the
  initial draft that planned a 100 USDT transfer — actual spike does 1 USDT).
- **Deleted parser:** `tx_visible_in_response` + `GetTransactionByIdResponse`
  removed in the #409 hardening — replaced by `tx_confirmed_in_receipt` +
  `GetTransactionInfoByIdResponse` (request-binding via `id` field).

#### Scope additions beyond original V1-V10 plan

- **`use_case_alpha_sends_beta_usdt`** is not in the original plan — it was
  added as the #409 ship-gate demo. Plan §File Structure leaves room for it
  under the spike's mandate ("verification harness for the open questions")
  but the explicit e2e flow is new.
