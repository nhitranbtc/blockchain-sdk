# `bcs-bitcoin` v1 Implementation Reference — Rust Crates & SDKs

**Purpose:** Focused deep-research sweep for what `bcs-bitcoin` needs to consume when targeting a real wallet app. Supplements the architecture/library research in `2026-08-04-bcs-bitcoin-unified-reference.md`.
**Method:** Four parallel Exa searches + targeted docs.rs / GitHub / crates.io fetches.
**Date:** 2026-08-04.

---

## Use cases & features

`bcs-bitcoin` v1 supports a focused set of wallet operations. Each row maps a user-facing use case to the technical capability, the crate path in this document, and any v2 deferral.

### Use cases

| #   | Use case                             | Who calls it                        | Technical capability                                                                                                                                                         | Crate / section                          |
| --- | ------------------------------------ | ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| 1   | **Create new single-sig wallet**     | Host on first launch or per-account | Generate 12-word BIP39 mnemonic, derive m/86'/0'/0' (Taproot) or m/84'/0'/0' (Native SegWit) root xprv, build `wpkh` or `tr` descriptor, persist via `Wallet::create_single` | §1 BDK, §9 add-wallet                    |
| 2   | **Import existing wallet**           | Host on restore from backup         | Parse user-entered mnemonic, validate checksum, derive same root xprv, recreate same descriptor, load from SQLite                                                            | §1 BDK, §9 add-wallet                    |
| 3   | **Generate receive address**         | Host per UX request                 | Derive next external keychain address from descriptor (`wallet.reveal_next_address(KeychainKind::External)`), display as bech32 (Taproot) or bech32 (Native SegWit)          | §1 BDK, §2 transaction-flows             |
| 4   | **Sync chain state**                 | Host on startup + periodic          | Pull blocks/addresses via `bdk_esplora` or `bdk_electrum`; `wallet.start_full_scan` for new wallets, `wallet.start_sync_with_revealed_spks` for known-history wallets        | §1 BDK, §3 (storage), §4 (sync)          |
| 5   | **Send transaction**                 | Host on send button                 | `wallet.build_tx()` with recipient + fee rate, sign via `Wallet::sign` (or `Psbt::sign` for external signer), broadcast via chain client                                     | §1 BDK, §2 transaction-flows             |
| 6   | **Bump stuck transaction fee (RBF)** | Host on user "speed up" action      | `wallet.build_fee_bump(txid)` returns new `TxBuilder`, sign, broadcast                                                                                                       | §5 RBF                                   |
| 7   | **Estimate fee rate**                | Host on send form                   | `esplora.get_fee_estimates()` returns 4-tier (fastest/30min/1h/economy) sat/vB                                                                                               | §5 fee                                   |
| 8   | **Encrypt seed at rest**             | Host on save                        | Argon2id password → 256-bit key; AES-256-GCM encrypt mnemonic + xprv; zeroize plaintext                                                                                      | §8 security (paired doc) + §A Cargo.toml |
| 9   | **Label addresses & transactions**   | Host on user action                 | `bdk-labels` add_label(txid/address, label), export to BIP-329 JSONL, persist to backend                                                                                     | §6 BIP-329                               |
| 10  | **Read-only / watch-only wallet**    | Host for cold storage viewer        | Import public-only descriptor, no xprv in scope; sync + show balance/UTXOs                                                                                                   | §1 BDK                                   |

### What `bcs-bitcoin` v1 explicitly does NOT do (deferred to v2 or out of scope)

- **Multisig / multi-account wallets** — single-sig only; revisit when user demand materializes (§B.8)
- **Hardware-wallet signer integration** — out of scope for v1 (§C.4)
- **Plausible deniability (multi-bucket storage)** — design for it (multi-bucket API), ship single-bucket in v1
- **P2P / Lightning payments** — `ldk-node 0.7` deferred until Lightning becomes a product goal
- **CoinJoin / PayJoin** — `payjoin 0.16` opt-in feature flag; not a v1 default
- **Cloud-based encrypted backup** — user backs up manually; BIP-139 export file is the portable format
- **P2SH multisig via `wsh` (native segwit multisig)** — deferred; use Taproot / single-sig in v1
- **Silent Payments (BIP-352)** — not in Cargo.toml
- **Fully async runtime (tokio everywhere)** — `bdk-wallet::rusqlite` is sync; switch to `bdk-sqlite` if the host service is fundamentally async

---

## Stablecoin on Lightning (phase 1)

`bcs-bitcoin` v1 supports stablecoin flows over the Lightning Network via three integration paths. The locked choice is **Boltz for phase 1** (atomic-swap transport); **Taproot Assets and LWK are parallel paths** for v1.5/v2.

### Use cases (stablecoin scope)

| #   | Use case                                                             | Who calls it                              | Technical capability                                                           | Crate / path                                                         |
| --- | -------------------------------------------------------------------- | ----------------------------------------- | ------------------------------------------------------------------------------ | -------------------------------------------------------------------- |
| 11  | **Pay Lightning invoice with USDT** (or any Boltz-supported asset)   | Host on user "pay with stablecoin" action | Build submarine swap: lock LN-side, receiver gets USDT on target chain         | `boltz-rs` (or HTTP client to Boltz API)                             |
| 12  | **Receive USDT, settle as Lightning**                                | Host on stablecoin receive                | Inbound USDT → atomic swap → user gets LN balance                              | `boltz-rs` (or HTTP client to Boltz API)                             |
| 13  | **Hold Liquid issued assets (L-BTC, L-USDT, etc.)**                  | Host on user "add Liquid wallet" action   | Confidential transactions, L-BTC, issued assets via Blockstream's `lwk_wollet` | `lwk_wollet` (sibling crate or feature-gated)                        |
| 14  | **Taproot Assets balance (USDt on Bitcoin via Lightning)** — v2 only | Host on Taproot Assets wallet             | MS-SMT commitments, multi-asset channels, proof verification                   | `ffranr/taproot-assets-rs` (community port; pre-1.0) — feature-gated |

### Architecture choices

| Layer                   | Phase 1 (`bcs-bitcoin` v1)                                                                        | Phase 1.5 (next minor)                                                             | Phase 2 (v2)                                                  |
| ----------------------- | ------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| **Transport over LN**   | Boltz HTTP/REST API (submarine swaps) — `boltz-rs` (Rust client) or thin wrapper around `reqwest` | Taproot Assets (`taproot-assets-rs` pre-1.0, behind `taproot-assets` feature flag) | Native multi-asset channels via LDK + Taproot Assets          |
| **Settlement**          | On-chain USDT (via Boltz routing through Arbitrum / tBTC) or L-BTC (via `lwk`)                    | On-chain USDt via Taproot Assets                                                   | Off-chain LN                                                  |
| **Asset custody model** | Non-custodial HTLC atomic swap (Boltz) — no operator trust                                        | Non-custodial issuance + statechain (Taproot)                                      | Non-custodial LN channels                                     |
| **Wallet pairing**      | `bcs-bitcoin` (BTC L1) + `lwk_wollet` (Liquid, separate chain) for L-USDT; Boltz for cross-chain  | `bcs-bitcoin` + Taproot Assets (single chain, multi-asset)                         | `bcs-bitcoin` + Taproot Assets (LN channels carry stablecoin) |

### Features (stablecoin capability matrix)

| Capability                                                  | v1                     | Notes                                                                      |
| ----------------------------------------------------------- | ---------------------- | -------------------------------------------------------------------------- |
| Pay Lightning invoice from on-chain stablecoin (Boltz swap) | ✅ Yes                 | Boltz supports USDT/USDC/L-BTC; receiver gets target asset on target chain |
| Receive stablecoin, auto-swap to LN sats (Boltz)            | ✅ Yes                 | Boltz `swap-out` API                                                       |
| Lightning-native multi-asset channel (Taproot Assets)       | ❌ No (v2)             | Rust SDK pre-1.0; wait for upstream stability                              |
| Confidential L-BTC + L-USDT (Liquid)                        | ✅ Yes (sibling crate) | Via `lwk_wollet`; different transaction type than Bitcoin L1               |
| Stablecoin RGB issuance (Tether on RGB, planned mid-2026)   | ❌ No (v2)             | RGB mainnet v0.11.1 (Jul 2025) but Tether launch deferred                  |
| Stablecoin Spark USDB (statechain)                          | ❌ No (v2)             | Different architecture (no channels); out of `bcs-bitcoin` scope           |
| Submarine swap status polling (Boltz)                       | ✅ Yes                 | Polling until swap completes or refunds                                    |

### Crate stack (v1 stablecoin)

```text
boltz-rs OR reqwest + manual Boltz API  ← Boltz HTTP client (atomic-swap transport)
lwk_wollet                            ← Liquid wallet (L-BTC + issued assets)
serde                                 ← (already in BDK dep tree)
bcs-common                            ← (existing path dep — share descriptor primitives)
```

For v1.5 / v2:

```text
taproot-assets-rs (ffranr)             ← multi-asset LN (pre-1.0; feature-gated)
taproot-assets-types                  ← shared types
taproot-assets-rpc                    ← gRPC client to tapd daemon
```

### Locked decision: Boltz for phase 1

**Use Boltz HTTP/REST API as the v1 stablecoin transport.** Rationale:

1. **No protocol change required.** Boltz works over existing LN channels — `bcs-bitcoin` doesn't need to add multi-asset channel support, just a new "swap" RPC.
2. **Mature since 2019.** Tether USDT Swaps live since Mar 2026; protocol non-custodial via HTLC atomicity; supports BTC/L-BTC/USDT/USDC + 14+ chains.
3. **Single integration point.** One HTTP client (or `boltz-rs` wrapper) covers all four swap directions (LN↔onchain, LN↔Liquid, onchain↔Lightning, LN↔USDT).
4. **Aligns with `lwk_wollet` for L-BTC.** Both can coexist as feature-gated sibling crates.
5. **Defer Taproot Assets to v2** — Rust SDK still pre-1.0; risk for v1 is too high.

### What `bcs-bitcoin` v1 stablecoin explicitly does NOT do (deferred)

- **Native multi-asset LN channels (Taproot Assets)** — requires `taproot-assets-rs` to mature to 1.0+; defer to v2.
- **Spark USDB (statechain)** — out of scope; requires Spark SDK and operator-trust model.
- **RGB USDT** — Tether launch deferred to mid-2026; wait for stable RGB-LN integration.
- **Liquid issued-asset issuance** (only L-BTC holding + receiving) — out of scope; `lwk_wollet` is read-only for the L-BTC side.

### How Boltz fits in the existing `bcs-bitcoin` structure

```text
bcs-bitcoin (BTC L1)
   │
   ├── feature: boltz-stablecoin (v1)   ← Boltz HTTP client, swap orchestration
   │
   ├── feature: liquid-wallet (v1.5)   ← lwk_wollet for L-BTC / L-USDT
   │
   ├── feature: taproot-assets (v2)    ← multi-asset LN
   │
   └── shared: bcs-common             ← descriptor + address primitives
```

Default features: `boltz-stablecoin = true` (v1 ships with Boltz support enabled). `liquid-wallet` and `taproot-assets` default to `false` (v1.5/v2 feature-gated).

### Stablecoin on Lightning — Crates & SDK detail

#### Crate inventory (with production-readiness evidence)

| Crate / SDK                      | Version              | Architecture                                     | Repo / stars / status                                                                                                                                               | Role in `bcs-bitcoin`                         | Used for                                                                     | Priority        | v1?                                                 |
| -------------------------------- | -------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- | ---------------------------------------------------------------------------- | --------------- | --------------------------------------------------- |
| `boltz-client`                   | 0.4.1 (2026-06-19)   | Boltz atomic-swap (HTLC)                         | [crates.io/boltz-client](https://crates.io/crates/boltz-client) · 12,093 total / 521 recent · 1 dependent · **Active (BoltzExchange + community)**                  | v1 stablecoin transport (LN ↔ USDT/L-BTC)     | Submarine + reverse + chain swaps; BOLT11 + BOLT12; cooperative claim/refund | **P0**          | ✅ Yes — default-on                                 |
| `boltz-rust`                     | (latest snapshot)    | Boltz atomic-swap reference impl                 | [SatoshiPortal/boltz-rust](https://github.com/SatoshiPortal/boltz-rust) · 41 ★ / 24 forks                                                                           | v1 reference / fallback                       | Manual script construction, custom swap orchestration                        | **P2**          | ⚠️ Optional — only if migrating from `boltz-client` |
| `lwk_wollet`                     | 0.18.1 (2026-06-19)  | Liquid (L-BTC + issued assets, confidential txs) | [Blockstream/lwk](https://github.com/Blockstream/lwk) · 110 ★ · **Active (Blockstream)** · 36k downloads / 7 dependents · 19-version stable track                   | v1.5 Liquid L-BTC + L-USDT holding            | L-BTC, L-USDT, confidential txs, Simplicity support                          | **P1**          | ⚠️ Optional — sibling crate or feature-gated        |
| `lightning` (LDK)                | 0.2.3                | LN channel protocol (core)                       | [lightningdevkit/rust-lightning](https://github.com/lightningdevkit/rust-lightning) · **1,360 ★** / 474 forks · 150 contributors · **Active (Spiral-funded)**       | v2 LN-node integration (multi-asset channels) | Channel state machine, HTLC interception, BOLT specs                         | **P2**          | ❌ No (v2) — only if shipping a full LN node in v2  |
| `ldk-node`                       | 0.7.0                | Ready-to-go LN node                              | [lightningdevkit/ldk-node](https://github.com/lightningdevkit/ldk-node) · **Active**                                                                                | v2 full-node companion                        | BDK-backed on-chain wallet + LN node in one                                  | **P2**          | ❌ No (v2)                                          |
| `lightning-invoice`              | 0.34.0               | BOLT11 invoice parser/serializer                 | part of LDK workspace · **Active**                                                                                                                                  | v1 invoice generation + parsing               | BOLT11 encode/decode                                                         | **P0**          | ✅ Yes — required by `boltz-client` v0.4+           |
| `taproot-assets`                 | 0.0.2 (2026-02-18)   | Multi-asset LN (Tether USDT over LN)             | [ffranr/taproot-assets-rs](https://github.com/ffranr/taproot-assets-rs) · **0 ★ / 0 forks** · 602 downloads / 1 dependent · **🚧 WIP — pre-1.0, single maintainer** | v2 multi-asset channel LN                     | MS-SMT commits, multi-asset HTLC routing                                     | **P3**          | ❌ No — wait for 1.0                                |
| `taproot-assets-types`           | 0.0.2                | Shared types                                     | same repo · 1,248 downloads / 5 dependents                                                                                                                          | v2 type definitions                           | Serialization helpers                                                        | **P3**          | ❌ No (v2)                                          |
| `taproot-assets-core`            | 0.0.2                | `no_std` proof verification                      | same repo · 247 downloads / 3 dependents                                                                                                                            | v2 verification                               | MS-SMT proof logic                                                           | **P3**          | ❌ No (v2)                                          |
| `taproot-assets-rpc`             | 0.0.2                | gRPC client to `tapd`                            | same repo · 80% documented                                                                                                                                          | v2 client bindings                            | Talks to Lightning Labs daemon                                               | **P3**          | ❌ No (v2)                                          |
| `boltz-exchange` (HTTP API)      | n/a                  | Service                                          | [api.boltz.exchange](https://api.docs.boltz.exchange) · **Production**                                                                                              | v1 transport target                           | Submarine/reverse/chain swap service                                         | n/a (service)   | ✅ Yes — `boltz-client` calls it                    |
| LNbits Boltz extension           | n/a                  | Web UI                                           | [lnbits/boltz](https://github.com/lnbits/boltz)                                                                                                                     | v1 reference UI (not a Rust dep)              | Merchants UI for Boltz swaps                                                 | n/a             | ❌ No (web only)                                    |
| `tortuga-swap`                   | (latest)             | A2L (Schnorr adaptor + CL puzzles)               | [yan-pi/tortuga-swap](https://github.com/yan-pi/tortuga-swap) · 1 ★ · **Research-grade**                                                                            | v2+ privacy upgrade                           | Replaces HTLC preimage linking with unlinkable adaptor signatures            | **P3**          | ❌ No — research                                    |
| `Bitfrost`                       | (latest)             | Fiber ⇄ LN edge-node                             | [FadhilMulinya/bitfrost](https://github.com/FadhilMulinya/bitfrost) · **Hackathon**                                                                                 | v2+ reference architecture                    | A2L + RFQ for stablecoin cross-chain hub                                     | **P3**          | ❌ No (hackathon)                                   |
| `taproot-assets-cli` (Go daemon) | v0.8.0 (Jun 2026)    | Multi-asset LN daemon                            | [lightninglabs/taproot-assets](https://github.com/lightninglabs/taproot-assets) · **520 ★** / 143 forks · **Active (Lightning Labs)**                               | v2 daemon (call via gRPC from `bcs-bitcoin`)  | Multi-asset routing, RFQ, edge-node pricing                                  | **P2**          | ❌ No (v2 daemon dependency)                        |
| `Boltz Client` (Go)              | v3.13.0 (2026-05-08) | Boltz daemon                                     | [BoltzExchange/boltz-client](https://github.com/BoltzExchange/boltz-client) · **AGPL-3.0**                                                                          | v2 companion daemon                           | Channel rebalancing, autoswap                                                | n/a (Go daemon) | ❌ No (different language)                          |

#### Verdict — Use or Not in v1

| Crate                            | v1 verdict                                              | Why                                                                                                                                                                                                                                |
| -------------------------------- | ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `boltz-client` 0.4+              | ✅ **Use** (P0, default-on)                             | MIT, 12k downloads, 1 dependent, 0.4 milestone = 1.0-track. Production-validated in Breez, BlueWallet, Cake Wallet, LNbits. v1 stablecoin transport via Boltz atomic-swap (LN ↔ USDT/L-BTC).                                       |
| `boltz-rust`                     | ❌ Don't use                                            | Superseded by `boltz-client`. Reference impl only.                                                                                                                                                                                 |
| `lwk_wollet` 0.18.1              | ✅ **Use as sibling crate in v1.5** (P1, feature-gated) | Blockstream-maintained, 19-version stable track (no breaking API), 36k downloads / 7 dependents. Liquid L-BTC + L-USDT, confidential txs, Simplicity. Different transaction type from `bdk_wallet`'s Bitcoin-only — keep separate. |
| `lightning-invoice` 0.34+        | ✅ **Use** (P0)                                         | Required by `boltz-client` v0.4+ for BOLT11/12.                                                                                                                                                                                    |
| `lightning` 0.2.3 (LDK core)     | ⚠️ Defer to v2 (P2)                                     | Only if shipping a full LN node in-process; Boltz covers v1 transport need.                                                                                                                                                        |
| `ldk-node` 0.7+                  | ⚠️ Defer to v2 (P2)                                     | Heavy dep; 1,360 ★ but only useful for v2 multi-asset channel needs.                                                                                                                                                               |
| `taproot-assets*` (ffranr) 0.0.2 | ❌ Don't use in v2 yet                                  | Pre-1.0, 0 ★, 0 forks, 1 dependent, single maintainer (`ffranr`), self-flagged "🚧 WIP — APIs may change without notice". Wait for upstream Lightning Labs first-party Rust client.                                                |
| `taproot-assets-cli` (Go daemon) | ⚠️ Use in v2 if at all                                  | 520 ★, official Lightning Labs daemon, but Go not Rust — call via gRPC if/when v2 wants multi-asset channels.                                                                                                                      |
| `Boltz Client` (Go daemon)       | ❌ Don't bundle                                         | AGPL-3.0; different language; `bcs-bitcoin` uses `boltz-client` (MIT) directly.                                                                                                                                                    |
| `breez/boltz-client`             | ❌ Don't use                                            | Reverse-only (LN→stablecoin); 1 ★, 0 forks; too narrow scope for v1.                                                                                                                                                               |
| LNbits Boltz extension           | ❌ Don't use                                            | Web UI only; not a Rust dep.                                                                                                                                                                                                       |
| `tortuga-swap` (A2L)             | ❌ Don't use in v1                                      | Research-grade; v2+ privacy upgrade for stablecoin HTLC unlinkability.                                                                                                                                                             |
| `Bitfrost` (Fiber ⇄ LN)          | ❌ Don't use                                            | Hackathon project; not production.                                                                                                                                                                                                 |
| `boltz-exchange` HTTP API        | ✅ Service target                                       | `boltz-client` calls it.                                                                                                                                                                                                           |

#### AGPL-3.0 analysis — does it taint `bcs-bitcoin`?

The Boltz backend (`boltz-exchange/boltz-backend`) and the npm `boltz-core` are both **AGPL-3.0**, but the **Rust client `boltz-client` is MIT**. Per the [AGPL-3.0 for SaaS explainer (fastCRW, Jul 2026)](https://fastcrw.com/blog/agpl-3-for-saas-explained), Section 13's network-source clause only fires if **you modify the AGPL program itself** and let users interact with that modified version over the network. Three scenarios for `bcs-bitcoin`:

| Scenario                                                                            | Network-source obligation                                                                                                                                              |
| ----------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bcs-bitcoin` calls `boltz.exchange` API over HTTPS via `boltz-client` (MIT)        | **None.** `bcs-bitcoin` is a network _client_ of an AGPL service. The AGPL obligation belongs to the operator, not to us, and it doesn't touch `bcs-bitcoin`'s source. |
| Self-host the Boltz backend inside `bcs-bitcoin`'s infra and call it over localhost | **None on `bcs-bitcoin`.** We run the unmodified engine; our app calling it is a separate work at arm's length.                                                        |
| Modify the Boltz backend and let external users interact with the modified engine   | **Yes — we'd owe source of the modified engine to our users.** But this is a hypothetical we never do in v1.                                                           |

**Bottom line:** AGPL-3.0 on `boltz-backend` is not a concern for `bcs-bitcoin` v1. If corporate policy forbids any AGPL in the dep graph regardless, switch to a self-hosted Boltz or a commercial license — but for the overwhelmingly common "call the API" pattern, no action needed.

#### Why this stack for v1 — locked decision rationale

`★ Insight ─────────────────────────────────────`

- **Boltz is the only v1-safe path in 2026.** 12k downloads + 1 dependent + 0.4 milestone + 7 npm dependents of `boltz-core` (Breez, BlueWallet, Cake Wallet, Klever all use it). The 1.0 line is the next major version; the API is stable.
- **AGPL-3.0 fear is misplaced.** The Rust client is MIT. We don't fork the engine, so Section 13 doesn't fire. Network clients of AGPL services have zero copyleft on their own code.
- **LWK 0.18's 19-version track is the strongest maturity signal** in the Liquid Rust space. Blockstream ships every 4-6 weeks. Use as a sibling crate, not a merged dep, because Liquid's transaction type is incompatible with `bdk_wallet`'s Bitcoin-only `bdk_chain`.

#### v1 locked stack

```text
boltz-client 0.4+         ← P0 (Boltz atomic-swap transport)
lwk_wollet 0.0.60+       ← P1 (Liquid L-BTC + L-USDT, sibling crate)
lightning-invoice 0.34+   ← P0 (required by boltz-client)
bitcoin 0.32+             ← P0 (already in bdk_wallet dep tree)
elements 0.26+            ← P1 (required by boltz-client L-BTC path)
secp256k1 0.32+           ← P0 (already in bdk_wallet dep tree)
reqwest 0.12+             ← P0 (required by boltz-client HTTP)
tokio 1.43+               ← P0 (async runtime)
serde / serde_json         ← P0 (already in workspace)
bcs-common (existing)     ← shared descriptor + address primitives
```

**v2 deferred:**

- `lightning` 0.2.3 (LDK core)
- `ldk-node` 0.7.0
- `taproot-assets*` (wait for 1.0 from upstream)

**v1 locked stack:**

```text
boltz-client 0.4+         ← P0 (Boltz atomic-swap transport)
lwk_wollet 0.0.60+       ← P1 (Liquid L-BTC + L-USDT, sibling crate)
lightning-invoice 0.34+   ← P0 (required by boltz-client)
bitcoin 0.32+             ← P0 (already in bdk_wallet dep tree)
elements 0.26+            ← P1 (required by boltz-client L-BTC path)
secp256k1 0.32+           ← P0 (already in bdk_wallet dep tree)
reqwest 0.12+             ← P0 (required by boltz-client HTTP)
tokio 1.43+               ← P0 (async runtime)
serde / serde_json         ← P0 (already in workspace)
bcs-common (existing)     ← shared descriptor + address primitives
```

**v2 deferred:**

- `lightning` 0.2.3 (LDK core)
- `ldk-node` 0.7.0
- `taproot-assets*` 0.0.2 (wait for 1.0)

---

## How a Bitcoin Transaction Flows

A Bitcoin transaction moves satoshis from inputs (UTXOs owned by the sender) to outputs (new UTXOs the recipient can later spend). The wallet stack turns a user intent ("send X to Y at fee Z") into a confirmed on-chain transaction through four stages. Each stage has dedicated crates in the Rust ecosystem.

### Lifecycle

```text
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 1 — BUILD                                                    │
│  Inputs: UTXOs the wallet owns (from chain state)                   │
│  Outputs: recipient script + change script                         │
│  Fee: sat/vB rate × estimated vbytes                                │
│  Crates: bdk_wallet (TxBuilder), miniscript (policy → Script),      │
│          bitcoin (Transaction type, PSBT)                           │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓ PSBT (Partially Signed Bitcoin Transaction)
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 2 — SIGN                                                     │
│  Inputs: PSBT + signing key                                         │
│  Outputs: PSBT with partial signatures → finalized Transaction     │
│  Crates: secp256k1 (ECDSA/Schnorr/MuSig2 signatures),              │
│          bitcoin (PSBT finalize → extract_tx),                      │
│          bip32 (HD derivation if signer is in-process)              │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓ raw Transaction bytes
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 3 — BROADCAST                                               │
│  Inputs: finalized Transaction                                      │
│  Outputs: txid (accepted to mempool)                                │
│  Crates: bdk_esplora / bdk_electrum / electrum-client /             │
│          esplora-client / rustywallet-mempool                       │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓ mempool acceptance
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 4 — CONFIRM (off-stage from sender)                          │
│  Inputs: txid                                                      │
│  Outputs: confirmation height (chain state update)                 │
│  Crates: bdk_esplora / bdk_electrum / bdk_kyoto / bip157 (sync)    │
└──────────────────────────────────────────────────────────────────────┘
```

### PSBT — the bridge between build and sign

The Partially Signed Bitcoin Transaction (BIP-174) is the standard interchange format between the builder (host wallet) and the signer (external device, hardware wallet, separate keystore service). BDK 3.0 formalizes this: the host calls `wallet.build_tx().finish()` to produce a `Psbt`, then calls `Psbt::sign(&secp, &signer)` (or `wallet.sign()` if the signer is in-process) to add signatures, then `psbt.extract_tx()` to get the finalized `Transaction` for broadcast.

PSBT (BIP-174) is the standard interchange format between build and sign. `wallet.build_tx().finish()` produces the unsigned PSBT; `Psbt::sign(&secp, &signer)` adds signatures; `psbt.extract_tx()` finalizes for broadcast.

### Crates by transaction stage

| Stage      | Crate(s)                               | Role                                                                |
| ---------- | -------------------------------------- | ------------------------------------------------------------------- |
| Build      | `bdk_wallet 3.1` (`TxBuilder`)         | UTXO selection, fee calc, output set, PSBT construct                |
| Build      | `miniscript 12.3`                      | Compile spending-policy descriptor into Bitcoin Script              |
| Build/Sign | `bitcoin 0.32` (`Transaction`, `Psbt`) | Type definitions, PSBT encode/decode/finalize                       |
| Sign       | `secp256k1 0.32`                       | ECDSA + Schnorr + MuSig2 signature primitives                       |
| Sign       | `bip32 0.5`                            | Derive signing key from mnemonic + path (when signer is in-process) |
| Broadcast  | `bdk_esplora 0.22`                     | Esplora REST `POST /tx`                                             |
| Broadcast  | `bdk_electrum 0.23`                    | Electrum `blockchain.transaction.broadcast`                         |
| Broadcast  | `electrum-client 0.25`                 | Low-level Electrum protocol (used by `bdk_electrum`)                |
| Broadcast  | `esplora-client 0.13`                  | Low-level Esplora REST (used by `bdk_esplora`)                      |
| Broadcast  | `rustywallet-mempool 0.2`              | Mempool.space REST `POST /tx` (P3, feature-gated)                   |
| Privacy    | `payjoin 0.16`                         | BIP-77/78 collaborative PayJoin transactions                        |
| Lightning  | `ldk-node 0.7`                         | On-chain funding + closing transactions for Lightning channels      |

### Crates by transaction feature

**Send (build + sign + broadcast a single transaction):**

- `bdk_wallet` — `build_tx() → finish() → sign() → extract_tx() → broadcast()` is the canonical end-to-end path.
- `bdk_esplora` / `bdk_electrum` — provide the `broadcast(&tx)` step.
- `bitcoin` + `secp256k1` — power the type system and signing under the hood.

**Receive (generate address for incoming payment):**

- `bdk_wallet` — `reveal_next_address(KeychainKind::External)` returns a fresh address from the wallet's external keychain.
- `miniscript` — descriptor compilation produces the scriptPubKey the address encodes.
- `bip32` — derives the next child key when the descriptor is private.

**Sync (discover incoming + confirm outgoing transactions):**

- `bdk_esplora` / `bdk_electrum` — fetch block headers, address histories, transactions; apply via `wallet.apply_update(update)`.
- `bdk_kyoto` / `bip157` / `niebla-158` — same job, via BIP-157/158 compact block filters (privacy-preserving sync).
- `bdk_chain` — chain state types (`TxGraph`, `LocalChain`) that track confirmed vs unconfirmed.

**Label / classify transactions:**

- `bdk-labels` — BIP-329 labels on txids/addresses/UTXOs (see §6).

**Bump fees on unconfirmed transactions (RBF / CPFP):**

- `bdk_wallet` — `wallet.build_fee_bump(txid)` for RBF (sender-side replace). CPFP is automatic when a later `build_tx` selects a parent UTXO with too-low fee.
- `bitcoin` — `Psbt::fee_rate()`, `Psbt::fee_amount()` for inspection.

**Sign externally (remote signer / secure enclave / separate keystore service):**

- `bdk_wallet 3.0` PSBT flow — `Psbt::sign(&secp, &signer)` where signer implements `bitcoin::psbt::Sign`.
- `bitcoin` — defines the `Sign` trait and the PSBT structure.

**Lightning on-chain (funding, closing, sweeps):**

- `ldk-node` — manages channel transactions through its own `bitcoin` re-exports.

**Privacy-enhanced send:**

- `payjoin` — replaces the simple spend with a PayJoin (sender + receiver collaborative); breaks common-input-ownership heuristics.

### Minimal end-to-end pipeline for `bcs-bitcoin`

The minimum crate set to send one transaction:

```text
bdk_wallet   — descriptor, UTXO selection, PSBT construct, RBF, finalize
secp256k1   — signing primitives (required if signer is in-process)
bdk_esplora — broadcast to mempool
```

If the signer is in-process, also add `bip32` for descriptor key derivation.

That is five crates (or four if the signer is external). Every other crate in this doc is either a swap-in alternative for one of those five, or for non-transaction concerns (labels, storage backend, sync backend, privacy, Lightning).

### What is NOT a "transaction crate"

These are part of the wallet stack but do not construct, sign, or broadcast transactions:

- `bip0039` — mnemonic → seed only; never touches a transaction
- `bdk-wallet::rusqlite` / `bdk-sqlite` / `bdk_redb` / `redb_wallet_storage` — persistence only
- `bdk_chain` — chain-state types only (TxGraph is a container, not a builder)
- `bip157` / `niebla-158` — sync engine only (no transaction builder)
- `bitcoin-fees` — fee primitives only
- `hd-wallet` / `rust-hdwallet` / `bip0032` / `khodpay-bip*` — HD key derivation only
- `bdk-labels` — labels existing transactions (does not build or sign)

An implementer wiring `bcs-bitcoin` should know which crates are transaction-aware and which are supporting infrastructure. Mixing them up leads to pulling in five crates when three suffice.

---

## BDK (`bdk_wallet 3.1`) — wallet core

### Overview

**Role:** Descriptor-based wallet. Owns the `Wallet` type, chain-state sync, coin selection, PSBT building. BDK 3.0 separates **wallet** from **signer**: private keys never enter `Wallet` — `Psbt::sign` is called externally.

**Production users (mid-2026):** Bitkey (Block), ProtonWallet, AnchorWatch, Bull Bitcoin, Liana (Wizardsardine), Alby, Arké, Fedimint, MetaMask (via `bdk-wasm`), Bark (Ark protocol), Coinbase (portions), Cake Wallet.

**Repo / stars:**

- `bdk_wallet`: [bitcoindevkit/bdk_wallet](https://github.com/bitcoindevkit/bdk_wallet) — 50 ★ (own repo), 1.1M+ all-time downloads, 35 dependents, 113k downloads/month
- Parent monorepo: [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) — **1.06k ★**, 467 forks, 87 open issues
- **MSRV:** 1.85.0
- **License:** MIT OR Apache-2.0

**Why BDK exists (vs raw `rust-bitcoin`):** `rust-bitcoin` is consensus-layer primitives (Transaction, Script, PSBT encode/decode). Building wallet logic on top (UTXO tracking, coin selection, descriptors, sync) is ~10k LOC and has historically been reinvented badly. BDK packages this — the `Wallet` type is the safe starting point. The cost is API opinions you'll fight if your use case is unusual.

### BDK ecosystem crate inventory

| Crate                 | Use in v1?                     | Version             | Repo / stars                                                                                                                         | Role                                                                           |
| --------------------- | ------------------------------ | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| `bdk_wallet`          | ✅ **Yes — core**              | 3.1.0 (2026-06-14)  | [bitcoindevkit/bdk_wallet](https://github.com/bitcoindevkit/bdk_wallet) · 50 ★, 1.1M+ downloads, 35 dependents, 113k downloads/month | High-level `Wallet` type, descriptors, PSBT build/sign, TxBuilder, persistence |
| `bdk_chain`           | ✅ **Yes — required dep**      | 0.23.x              | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) (mono-repo) · 1.06k ★                                                      | TxGraph, Indexer, LocalChain — chain-state primitives                          |
| `bdk_core`            | ✅ **Yes — required dep**      | (mono-repo)         | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk)                                                                            | Low-level types used by `bdk_chain`, `bdk_wallet`, and chain-data crates       |
| `bdk_esplora`         | ✅ **Yes — chain source (P0)** | 0.22.2 (2026-03-26) | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) · 1.06k ★ · 705k downloads · 15 dependents · 154k recent                   | Esplora HTTP chain source via `EsploraExt` / `EsploraAsyncExt`                 |
| `bdk_electrum`        | ✅ **Yes — fallback (gated)**  | 0.24.0 (2026-05-08) | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) · 1.06k ★ · 567k downloads · 10 dependents · 167k recent                   | Electrum chain source via `BdkElectrumClient`                                  |
| `bdk_bitcoind_rpc`    | 🟡 Optional (self-hosted)      | 0.22.x              | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) · 1.06k ★                                                                  | Bitcoin Core RPC chain source via `bitcoincore-rpc`                            |
| `bdk_file_store`      | ❌ No — testing only           | 0.22.x              | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk)                                                                            | File-based persistence (testing/development only — not for production)         |
| `bdk-wasm`            | ❌ No (Rust-only v1)           | (BDK org)           | [bitcoindevkit/bdk-wasm](https://github.com/bitcoindevkit/bdk-wasm)                                                                  | WebAssembly bindings (used by MetaMask for Bitcoin)                            |
| `bdk-ffi`             | ❌ No — deferred to v2         | (BDK org)           | [bitcoindevkit/bdk-ffi](https://github.com/bitcoindevkit/bdk-ffi) · 127 ★                                                            | UniFFI bindings: Swift (iOS), Kotlin (Android), Python, JVM                    |
| `bdk-dart` / `bdk-rn` | ❌ No — deferred to v2         | (BDK org)           | bdk-dart · bdk-rn                                                                                                                    | Flutter / React Native bindings (integration testing stage, early 2026)        |
| `bdk-cli`             | ❌ No — reference only         | (BDK org)           | [bitcoindevkit/bdk-cli](https://github.com/bitcoindevkit/bdk-cli) · 138 ★                                                            | Reference CLI wallet demo                                                      |

**Total BDK family dependents (direct):** 32 crates, 54 including indirect.

### BDK vs other Rust Bitcoin libraries (alternatives for `bcs-bitcoin`)

| Library                    | Repo / stars                                                                                      | Language                                      | Scope                   | Pros                                                                                                                                                                                                                                                                  | Cons                                                                                                                                                                                                      |
| -------------------------- | ------------------------------------------------------------------------------------------------- | --------------------------------------------- | ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **BDK** (bitcoindevkit)    | [bdk](https://github.com/bitcoindevkit/bdk) · **1.06k ★**                                         | Rust (+ Swift/Kotlin/Python via UniFFI)       | On-chain wallet         | (1) Production-validated (Bitkey, ProtonWallet, Bull). (2) Descriptor/Miniscript first. (3) Multi-language bindings via UniFFI. (4) Pinned by Spiral (Block) + OpenSats grants. (5) Modular — swap persistence / chain source. (6) Taproot + Schnorr + MuSig2 native. | (1) API opinions you'll fight if non-standard. (2) 1.0 only stable Dec 2024 — still gaining maturity. (3) Complex use cases need source-code familiarity. (4) Mobile bindings don't expose full Rust API. |
| `rust-bitcoin`             | [rust-bitcoin/bitcoin](https://github.com/rust-bitcoin/bitcoin) · **2.6k ★**                      | Rust                                          | Consensus primitives    | (1) CC0-1.0 public domain. (2) 12M+ total downloads. (3) Maintained by Andrew Poelstra (Blockstream). (4) `no_std` for embedded/WASM. (5) Taproot/Schnorr native. (6) Same org as `rust-miniscript`.                                                                  | (1) **No wallet semantics** — you build UTXO/coin-selection yourself. (2) No descriptors, no Miniscript (separate `rust-miniscript` crate). (3) No chain sync. (4) No chain-source abstraction.           |
| `lwk_wollet` (Blockstream) | [Blockstream/lwk](https://github.com/Blockstream/lwk) · 110 ★                                     | Rust (+ FFI: Swift, Kotlin, Python, Java, JS) | Liquid + Bitcoin wallet | (1) Blockstream-grade production hardening. (2) Liquid + Bitcoin in one crate. (3) Sub-100ms response times. (4) Encrypted `EncryptedStore` pattern (AES-256-GCM-SIV). (5) Liquid-sidechain integration.                                                              | (1) Liquid-first; Bitcoin is younger. (2) Custom data model — not descriptor-first. (3) Smaller community than BDK. (4) Blockstream-specific deployment pattern.                                          |
| `rust-payjoin`             | [payjoin/rust-payjoin](https://github.com/payjoin/rust-payjoin) · 155 ★                           | Rust                                          | BIP-77/78 PayJoin       | (1) OpenSats-funded. (2) Bull Bitcoin + Cake Wallet production. (3) BIP-77 async + BIP-78 sync. (4) FFI bindings for all major languages. (5) Moving to 1.0 (current `0.x` to `1.0-rc.2`).                                                                            | (1) **Not a general wallet** — PayJoin-only. (2) Requires a counterparty that also supports PayJoin.                                                                                                      |
| `rust-bitcoincore-rpc`     | [rust-bitcoin/rust-bitcoincore-rpc](https://github.com/rust-bitcoin/rust-bitcoincore-rpc) · 386 ★ | Rust                                          | Core RPC client         | (1) Same org as `rust-bitcoin`. (2) Stable.                                                                                                                                                                                                                           | (1) RPC layer only. (2) Requires running Core.                                                                                                                                                            |

**Workspace layout** (`bitcoindevkit/bdk` mono-repo):

```text
crates/
├─ core/                    # bdk_core (low-level types)
├─ chain/                   # bdk_chain (TxGraph, Indexer, LocalChain)
├─ esplora/                 # bdk_esplora (Esplora HTTP client)
├─ electrum/                # bdk_electrum
├─ bitcoind_rpc/            # bdk_bitcoind_rpc
└─ file_store/              # bdk_file_store (testing only)
```

**`bdk_wallet 3.1.0` metadata:**

| Property       | Value                                                                                                  |
| -------------- | ------------------------------------------------------------------------------------------------------ |
| crates.io      | v3.1.0 (2026-06-14), 1.1M+ downloads, 35 dependents                                                    |
| MSRV           | 1.85.0                                                                                                 |
| Required deps  | `bdk_chain ^0.23.3`, `bitcoin ^0.32.8`, `miniscript ^12.3.5` (capped at <13.0 until BDK loosens bound) |
| Default feats  | `std`                                                                                                  |
| Optional feats | `rusqlite`, `file_store`, `compiler`, `test-utils`, `keys-bip39`, `all-keys`                           |

### Adding a new wallet — full pipeline

A "new wallet" in Bitcoin means **a fresh mnemonic** (or fresh key import) that derives to a brand-new address space, independent of any existing wallet. Distinct from "new account" (§2 in the merged reference, or see the short preview further down).

`bcs-bitcoin` v1 is built on BDK, so "add wallet" maps to: create a new `bdk_wallet::Wallet` instance with a freshly-generated (or imported) descriptor, persist it, sync from genesis or from the user's known birthday.

#### Lifecycle of adding a wallet

```text
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 1 — KEY GENERATION (or IMPORT)                                 │
│  New path:  Mnemonic::generate(12) + derive root xprv                 │
│  Import:    parse user-entered 12/24-word mnemonic OR import xprv     │
│  Crates:    bip0039 0.12, bip32 0.5                                  │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 2 — DESCRIPTOR BUILD                                           │
│  Pick script type (wpkh, wsh, tr). Pick BIP-44 purpose (44/49/84/86)│
│  Pick coin_type (0/1). Pick account_index = 0 for first account.     │
│  Build:  wpkh([fingerprint/84h/0h/0h]xprv.../0/*)                    │
│  Crates:    bitcoin 0.32, bdk_wallet 0.3.1 (Descriptor types)        │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 3 — WALLET INSTANCE                                            │
│  Wallet::create_single(descriptor)?                                  │
│  .network(Network::Signet)                                           │
│  .create_wallet(&mut connection)?                                    │
│  → returns Result<Wallet, CreateWithPersistError>                     │
│  Crates:    bdk_wallet 3.1, bdk-wallet::rusqlite (or bdk-sqlite)      │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 4 — FIRST SYNC                                                 │
│  Pick sync strategy: full_scan (new wallet, no history) OR            │
│  start_sync_with_revealed_spks (wallet has metadata).                 │
│  Apply via wallet.apply_update(update)?                              │
│  Persist with wallet.persist(&mut connection)?                       │
│  Crates:    bdk_esplora 0.22 (P0) — or bdk_electrum, bdk_kyoto     │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 5 — BACKUP PROMPT (host responsibility)                       │
│  Display mnemonic words (one at a time).                              │
│  Verify 2 random words (Cake pattern).                                │
│  Host encrypts + persists mnemonic separately if user opts in.       │
│  Crates:    aes-gcm, argon2, zeroize (paired security doc)         │
└──────────────────────────────────────────────────────────────────────┘
```

#### Crates per stage

| Stage                         | Crate                             | Version     | Role                                                                                          |
| ----------------------------- | --------------------------------- | ----------- | --------------------------------------------------------------------------------------------- |
| 1. Generate / import mnemonic | `bip0039` (rust-bitcoin)          | 0.12        | Mnemonic generation from CSPRNG; mnemonic parsing; PBKDF2 seed derivation; passphrase support |
| 1. Derive root xprv from seed | `bip32` (rust-bitcoin)            | 0.5         | BIP32 root key derivation; hardened/normal path derivation                                    |
| 2. Build descriptor           | `bitcoin` (rust-bitcoin)          | 0.32        | `Descriptor<PublicKey, SecretKey>` type + parse/format machinery                              |
| 2. Wrap descriptor for BDK    | `bdk_wallet::keys`                | 3.1         | `DescriptorSecretKey`, `DescriptorPublicKey`, `KeychainKind`                                  |
| 3. Create wallet instance     | `bdk_wallet`                      | 3.1         | `Wallet::create_single(descriptor)?.network(Network::Signet).create_wallet(&mut conn)?`       |
| 3. Persist wallet state       | `bdk-wallet::rusqlite` (built-in) | n/a         | SQLite persistence; works with multiple wallets in one DB                                     |
| 4. Chain sync                 | `bdk_esplora`                     | 0.22        | First-time full scan via `client.full_scan(req, STOP_GAP, PARALLEL_REQUESTS)?`                |
| 4. Apply sync                 | `bdk_wallet`                      | 3.1         | `wallet.apply_update(update)?`                                                                |
| 4. Persist sync result        | `bdk_wallet::rusqlite`            | n/a         | `wallet.persist(&mut conn)?`                                                                  |
| 5. Backup UX                  | `bip0039` + `zeroize`             | 0.12 + 1.7+ | `Zeroizing<Mnemonic>` for memory; `aes-gcm` + `argon2` for storage encryption (paired doc)    |

#### Canonical `bcs-bitcoin` v1 add-wallet flow

```rust
use bdk_wallet::keys::bip39::Mnemonic;
use bdk_wallet::keys::{DescriptorSecretKey, ExtendedKey, DerivationPath};
use bdk_wallet::keys::Descriptor;
use bdk_wallet::{Wallet, KeychainKind};
use bdk_wallet::bitcoin::{Network, secp256k1::XPrv};
use bdk_wallet::rusqlite::Connection;
use bdk_esplora::EsploraExt;
use esplora_client::Builder;
use zeroize::Zeroizing;

const STOP_GAP: usize = 50;
const PARALLEL_REQUESTS: usize = 8;

fn add_wallet(
    conn: &mut Connection,
    network: Network,
    mnemonic_words: usize,    // 12 or 24
    passphrase: &str,         // "" if none
) -> Result<(Wallet, Zeroizing<Mnemonic>), anyhow::Error> {
    // 1. Mnemonic (new or imported — host decides)
    let mnemonic = if mnemonic_words == 0 {
        Zeroizing::new(Mnemonic::generate(12)?)
    } else {
        Zeroizing::new(Mnemonic::parse_in(normalized, word_count, Language::English)?)
    };
    let seed = mnemonic.to_seed(passphrase);

    // 2. Root xprv → descriptor (account 0, BIP-86 single-key Taproot)
    let root: XPrv = XPrv::new(&seed)?;
    let account_path: DerivationPath = "m/86'/0'/0'".parse()?;
    let account_xprv = root.derive_path(&account_path)?;
    let descriptor: Descriptor<_, DescriptorSecretKey> =
        format!("tr({}/0/*)", account_xprv).parse()?;

    // 3. Create wallet instance + persist
    let wallet = Wallet::create_single(&descriptor)?
        .network(network)
        .create_wallet(conn)?;

    // 4. First-time full sync
    let client = Builder::new("https://mutinynet.com/api").build_blocking();
    let req = wallet.start_full_scan();
    let update = client.full_scan(req, STOP_GAP, PARALLEL_REQUESTS)?;
    wallet.apply_update(update)?;
    wallet.persist(conn)?;

    Ok((wallet, mnemonic))
}
```

#### Multi-wallet persistence

`bdk-wallet::rusqlite` supports multiple wallets per SQLite file via per-wallet table partitioning. For `bcs-bitcoin`:

- Each user session = one SQLite file
- Each wallet = one `bdk_wallet::Wallet` instance + a row in a top-level `wallets` table
- Descriptor, tx history, labels, UTXO graph — stored per-wallet
- Mnemonic / xprv — stored SEPARATELY (in a different encrypted file) so SQLite never sees plaintext

```sql
CREATE TABLE wallets (
    id          TEXT PRIMARY KEY,    -- UUID
    name        TEXT NOT NULL,
    network     INTEGER NOT NULL,
    descriptor  TEXT NOT NULL,        -- public-only; private kept in separate keystore
    birthday    INTEGER NOT NULL,
    created_at  INTEGER NOT NULL
);
```

#### New wallet vs new account

| New wallet                              | New account                                           |
| --------------------------------------- | ----------------------------------------------------- |
| New mnemonic (or imported xprv)         | Existing mnemonic, new BIP-44 account index           |
| Independent address space               | Same mnemonic, disjoint derivation path               |
| `Wallet::create_single(new_descriptor)` | `Wallet::create_single(bumped_descriptor)`            |
| Persist as a separate wallet row        | Same wallet table, new descriptor row                 |
| Recovery = restore mnemonic             | Recovery = restore mnemonic + use right account index |

---

## Fee estimation — `bdk_esplora` primary, BlockBook fallback

Fee estimation in `bcs-bitcoin` is a **chain-source concern**, not a wallet-core concern. `bdk_wallet` exposes a `FeeRate` type and accepts a fee policy for `TxBuilder`, but the actual fee oracle comes from the chain client (`bdk_esplora` / `bdk_electrum` / `bcs-providers`' BlockBook client).

### Where the fee estimate comes from

```text
┌──────────────────────────────────────────────────────────────────────┐
│  SOURCE 1 — Esplora REST API (bdk_esplora, P0)                       │
│  GET /v1/fees/recommended                                            │
│  Returns: fastestFee / halfHourFee / hourFee / economyFee / minimumFee│
│  Best for: low-overhead REST, simplest ops                           │
│  Dep:        bdk_esplora 0.22 (already P0 in Cargo.toml)            │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓
┌──────────────────────────────────────────────────────────────────────┐
│  SOURCE 2 — Electrum RPC (bdk_electrum, P2 fallback)                 │
│  blockchain.estimatefee(blocks=2, mode=CONSERVATIVE)                 │
│  Returns: fee_rate (BTC/kB); convert to sat/vB                       │
│  Best for: when Esplora endpoint down, fallback to Electrum server   │
│  Dep:        bdk_electrum 0.23 (feature-gated)                      │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓
┌──────────────────────────────────────────────────────────────────────┐
│  SOURCE 3 — BlockBook via bcs-providers (internal HTTP client)       │
│  GET /api/v2/fees/BTC (or BCH/LTC/DOGE per network)                  │
│  Returns: slow / normal / priority (sat/vB)                          │
│  Best for: BTC/BCH/LTC/DOGE (bcs-bitcoin core scope); same server    │
│             used for UTXO/account queries                            │
│  Dep:        reqwest in bcs-providers (already a workspace dep)      │
└──────────────────────────────────────────────────────────────────────┘
```

### Full fee-estimation SDK landscape (research, 2026-08)

| SDK                                          | Use in v1?                                    | Version                                                                                                         | Repo / Stars / Status                                                                                                                       | Endpoint / API                                                                                                                      | Pros                                                                                                                                                                                                                                                                                                                       | Cons                                                                                                                                                                                                                                                                                                                                                            |
| -------------------------------------------- | --------------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bdk_esplora`                                | ✅ **Yes — primary**                          | 0.22.2 (2026-03-26)                                                                                             | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) · **1.06k ★** · 154k recent downloads · Active (official BDK org)                 | `EsploraExt::get_fee_estimates()` → `FeeEstimates { fastest_fee, half_hour_fee, hour_fee, economy_fee, minimum_fee }` (sat/vB, u64) | (1) Already P0 in `Cargo.toml` for chain sync — zero new dep. (2) Maintained by bitcoindevkit org (35 dependents on `bdk_wallet`). (3) Blocking + async variants. (4) Esplora REST is standard — every Esplora-compatible server exposes the same path. (5) `FeeRate` typed return — no manual BTC/kB → sat/vB conversion. | (1) Esplora REST proxies Core's `estimatesmartfee` — historical methodology, conservative. (2) Mempool.space's deprecated `/fee-estimates` endpoint had non-numeric warning keys (fixed by `bitcoindevkit/rust-esplora-client` commit `884503c` Jun 2026). (3) Public Esplora servers may rate-limit.                                                           |
| `bdk_electrum`                               | ✅ **Yes — fallback**                         | 0.24.0 (2026-05-08)                                                                                             | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) · **1.06k ★** · 167k recent downloads · 10 dependents · Active (official BDK org) | `ElectrumApi::estimate_fee(target: usize) -> Result<f64, _>` (BTC/kB; convert to sat/vB)                                            | (1) Same bitcoindevkit org. (2) 567k all-time downloads. (3) Fallback already in `bcs-providers` for chain sync. (4) Electrum servers are widely deployed and self-hostable.                                                                                                                                               | (1) Returns BTC/kB float — must convert to sat/vB. (2) Electrum protocol doesn't expose multiple fee tiers — only one rate per confirmation target. (3) Same historical methodology as Esplora.                                                                                                                                                                 |
| `bdk_bitcoind_rpc`                           | 🟡 Optional (self-hosted)                     | 0.22.x (uses `bitcoind 0.32`)                                                                                   | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) · **1.06k ★** · Active                                                            | `estimatesmartfee conf_target [estimate_mode]` → JSON `{ feerate: BTC/kvB, errors, blocks }`                                        | (1) Same bitcoindevkit org. (2) Most accurate when self-hosted (no third-party trust). (3) Supports `ECONOMICAL` and `CONSERVATIVE` modes since Core v28.0. (4) Three time horizons (short/medium/long halflife). (5) Already in `bcs-providers` workspace tree (per §1) — only needs feature flag.                        | (1) Requires running a full Bitcoin Core node. (2) Requires warm-up: needs ≥2× target blocks of observation before valid estimates. (3) Conservative by design — overpays ~96% per Delving Bitcoin analysis. (4) Returns BTC/kvB; needs sat/vB conversion. (5) JSON RPC adds parsing layer.                                                                     |
| `bcs-providers` (BlockBook client, internal) | 🟡 Optional (deferred to v2)                  | n/a (in-repo)                                                                                                   | Internal HTTP client (reqwest)                                                                                                              | `GET /api/v2/fees/BTC` (or BCH/LTC/DOGE per network) → `{ slow, normal, priority }` (sat/vB)                                        | (1) Already a workspace dep. (2) Same server handles UTXO/account queries — no new TLS/auth config. (3) Self-hostable via BlockBook + Bitcoin Core. (4) Already required for `bcs-providers::UtxoNetworkProvider`.                                                                                                         | (1) BlockBook is a third-party project (not Core-aligned methodology). (2) Returns only 3 tiers (slow/normal/priority), not 5+. (3) Requires BlockBook server alongside Core.                                                                                                                                                                                   |
| `rustywallet-mempool`                        | ❌ **No — deferred** (2 ★, 0 dependents)      | 0.2.0 (2026-01-02)                                                                                              | [nirvagold/rustywallet](https://github.com/nirvagold/rustywallet) · **2 ★** · 41 total downloads · 0 dependents · **Single maintainer**     | `MempoolClient::get_fees()` → `{ fastest_fee, half_hour_fee, hour_fee, economy_fee, minimum_fee }`                                  | (1) Mempool-based (projected blocks, refreshed ~2s — better than Esplora's historical). (2) 5-tier output matches Esplora shape. (3) Same client covers UTXO queries + broadcast. (4) WebSocket support for fee stream. (5) Authoritative for mempool.space / self-hosted mempool instances.                               | (1) 2 ★, 41 downloads, **0 dependents** — single-maintainer bus-factor risk. (2) Newly created (Jan 2026) — production untested. (3) Adds a dep where `bdk_esplora` already covers chain + fee. (4) The mempool-based estimate is better but only matters in volatile fee markets. (5) Mempool.space API is unauthenticated with informal ~10 req/s rate limit. |
| `bitcoin-fees` (klebs6 fork)                 | ❌ **No — deferred** (single-maintainer fork) | 0.1.21                                                                                                          | [klebs6/bitcoin-rs](https://github.com/klebs6/bitcoin-rs) · niche                                                                           | n/a (primitive only — no fetcher)                                                                                                   | (1) `FeeRate` with `sat/kvB` and `sat/vB` conversion matching Core's arithmetic. (2) `FeeEstimateMode` enum (UNSET/ECONOMICAL/CONSERVATIVE). (3) `FeeEstimateHorizon` (short/medium/long half-life). (4) `FeeReason` enum for transparency. (5) `FeeFilterRounder` for mempool-privacy-precise bucketing.                  | (1) **Single-maintainer fork** of Bitcoin Core's fee logic — independent from Core. (2) Provides primitives, not a fetcher — you still need a source. (3) No data on maintenance cadence or bus factor. (4) Useful only when implementing your own Core-compatible estimator.                                                                                   |
| `mempool_space` (RandyMcMillan)              | 0.0.60 (2024-07-05)                           | [RandyMcMillan/mempool_space](https://github.com/RandyMcMillan/mempool_space) · niche                           | `get_fee_estimates`, `get_recommended_fees`, etc.                                                                                           | (1) Simple mempool.space REST wrapper.                                                                                              | (1) **Dormant** (last update mid-2024 — before rustywallet-mempool existed). (2) No recent downloads. (3) Replaced by `rustywallet-mempool` for active development.                                                                                                                                                        |
| `electrum-client` (raw)                      | 0.25                                          | [bitcoindevkit/rust-electrum-client](https://github.com/bitcoindevkit/rust-electrum-client) · **87 ★** · Active | `ElectrumApi::estimate_fee(blocks)`                                                                                                         | (1) Lowest-level access; `bdk_electrum` wraps it. (2) Same org as bdk.                                                              | (1) Already covered by `bdk_electrum` — no reason to use raw.                                                                                                                                                                                                                                                              |

### Decision criteria recap

| Criterion                                     | Why it matters                                                                                                                                                                                    |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Maintenance bus factor**                    | Single-maintainer crates (2 ★, 0 dependents) are supply-chain risks. Anything below ~100 dependents is elevated risk.                                                                             |
| **Dependency on existing `Cargo.toml` items** | If `bdk_esplora` is already P0 for chain sync, getting fee estimation from it costs nothing. Adding `rustywallet-mempool` is +1 dep for marginal accuracy gain.                                   |
| **Methodology (historical vs mempool)**       | Esplora / Electrum / Core `estimatesmartfee` are historical (conservative). Mempool.space is mempool-projected (sharper). For wallet UX, the difference is small unless fee markets are volatile. |
| **Self-hosting**                              | Bitcoin Core `estimatesmartfee` and BlockBook fee endpoint both self-host; mempool.space also self-hostable. Public endpoints are fine for v1.                                                    |
| **License + audit**                           | bitcoindevkit crates: MIT/Apache-2.0; `aes-gcm` is NCC-audited; `bitcoin-fees` from klebs6 fork — no audit trail.                                                                                 |

### What we explicitly do NOT ship in v1

| Crate                                   | Why skipped                                                                                                                                          |
| --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rustywallet-mempool 0.2.0` (nirvagold) | 2 ★, 41 downloads, 0 dependents, single maintainer. Bus-factor risk. Add later if mempool.space-specific features (WebSocket fee stream) are needed. |
| `bitcoin-fees 0.1.21` (klebs6 fork)     | Niche; only useful when implementing your own Core-compatible estimator. BlockBook + Esplora cover the wallet-facing case.                           |
| `mempool_space 0.0.60` (RandyMcMillan)  | Dormant (last update 2024); `rustywallet-mempool` is the current alternative. Don't pick a stale crate.                                              |

### v1 implementation — recommended

**Use `bdk_esplora`'s fee endpoint as primary.** It's already in the `Cargo.toml` (§A) for chain sync — no new dependency needed. Esplora's `GET /v1/fees/recommended` returns four fee targets (fastest / 30 min / 1 hour / economy) which cover every UX scenario without custom code.

```rust
use bdk_esplora::EsploraExt;
use bdk_wallet::bitcoin::FeeRate;

#[derive(Clone, Copy, Debug)]
pub enum FeeTarget {
    Fastest,
    HalfHour,
    Hour,
    Economy,
}

impl FeeTarget {
    /// Map UI target → Esplora `FeeEstimates` field name
    fn esplora_field(&self) -> &'static str {
        match self {
            Self::Fastest  => "fastestFee",
            Self::HalfHour => "halfHourFee",
            Self::Hour     => "hourFee",
            Self::Economy  => "economyFee",
        }
    }
}

pub fn estimate_fee_rate(
    esplora: &bdk_esplora::EsploraClient,
    target: FeeTarget,
) -> Result<FeeRate, bdk_esplora::Error> {
    esplora.get_fee_estimates()
        .map(|est| {
            // Esplora returns `FeeEstimates { fastest_fee, half_hour_fee, hour_fee, economy_fee, minimum_fee }`
            let sat_vb = match target {
                FeeTarget::Fastest  => est.fastest_fee,
                FeeTarget::HalfHour => est.half_hour_fee,
                FeeTarget::Hour     => est.hour_fee,
                FeeTarget::Economy  => est.economy_fee,
            };
            FeeRate::from_sat_per_vb(sat_vb.max(1))
        })
}
```

### Cargo.toml additions

None — `bdk_esplora 0.22` is already in §A. Fee estimation piggybacks on the existing chain-source dep.

### If `bdk_esplora` is unavailable (Electrum fallback)

```rust
use bdk_electrum::BdkElectrumClient;
use bdk_electrum::electrum_client::ElectrumApi;
use bdk_wallet::bitcoin::FeeRate;

pub fn estimate_fee_rate_electrum(
    client: &BdkElectrumClient,
    blocks: u16,
) -> Result<FeeRate, bdk_electrum::Error> {
    // Electrum returns BTC/kB; convert to sat/vB.
    let btc_per_kvb = client.estimate_fee(blocks)? as f64;
    let sat_per_vb = (btc_per_kvb * 100_000_000.0) / 1000.0;
    Ok(FeeRate::from_sat_per_vb(sat_per_vb.max(1.0) as u64))
}
```

### Locked decisions for v1

1. **Primary fee source:** `bdk_esplora` `GET /v1/fees/recommended`. No new dep.
2. **Fallback fee source:** `bdk_electrum` `blockchain.estimatefee` (requires `electrum` feature).
3. **No third-party fee crate** in v1 `Cargo.toml`. `rustywallet-mempool` and `bitcoin-fees` deferred to v2 (gated behind feature flags).
4. **No `mempool.space` HTTP client** in v1 — BlockBook's fee endpoint is already in `bcs-providers` and covers the same UX.

### RBF (BIP-125) fee bump — escape hatch for stuck transactions

Even with the best fee oracle, a transaction can get stuck (mempool evicted, block found elsewhere, fee market spikes after broadcast). `bdk_wallet` ships Replace-By-Fee (BIP-125) as the standard recovery mechanism.

**How it works in BDK 3.1:**

```rust
use bdk_wallet::bitcoin::FeeRate;

// Bump a stuck tx by raising the fee rate
let original_feerate = psbt.fee_rate().expect("has rate");
let new_feerate = FeeRate::from_sat_per_vb(
    original_feerate.to_sat_per_vb_ceil() + 1
);

let mut builder = wallet.build_fee_bump(txid).expect("can bump");
builder.fee_rate(new_feerate);
let mut bumped_psbt = builder.finish()?;
wallet.sign(&mut bumped_psbt, SignOptions::default())?;
let bumped_tx = bumped_psbt.extract_tx()?;
network_client.broadcast(&bumped_tx)?;
```

**Why use it:** RBF is the standard sender-side recovery mechanism. Without it, a low-fee tx is stuck until confirmation (often days during low-fee markets) or requires CPFP-bump via a child transaction (more complex UX). With RBF, the sender re-signs at a higher fee and broadcasts the replacement atomically — the original tx is replaced, not double-spent.

BDK's `build_fee_bump` removes the manual PSBT-diffing work: it returns a fresh `TxBuilder` that reuses the original transaction's recipient outputs and replaces only the fee. The bumped PSBT preserves the recipient; only fee increases via additional input + larger change. RBF signaling is enabled by default in BDK; `set_exact_sequence(n)` disables it.

**Locked decision:** Use BDK's built-in RBF as the safety net for fee estimation underestimates. This is why §5 ships `bdk_esplora` (historical, conservative) rather than `rustywallet-mempool` (mempool-projected, sharper but lower bus factor) — conservative estimates + RBF cover the safety case; sharp estimates alone risk getting stuck.

---

## Adding a new wallet — full pipeline

A "new wallet" in Bitcoin means **a fresh mnemonic** (or fresh key import) that derives to a brand-new address space, independent of any existing wallet. This is distinct from "new account" (§10), which shares a mnemonic.

`bcs-bitcoin` v1 is built on BDK, so "add wallet" maps to "create a new `bdk_wallet::Wallet` instance with a freshly-generated (or imported) descriptor, persist it, sync from genesis or from the user's known birthday".

### Lifecycle of adding a wallet

```text
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 1 — KEY GENERATION (or IMPORT)                                 │
│  New path:  Mnemonic::generate(12) + derive root xprv                 │
│  Import:    parse user-entered 12/24-word mnemonic OR import xprv     │
│  Crates:    bip0039 0.12, bip32 0.5                                  │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 2 — DESCRIPTOR BUILD                                           │
│  Pick script type (wpkh, wsh, tr). Pick BIP-44 purpose (44/49/84/86)│
│  Pick coin_type (0/1). Pick account_index = 0 for first account.     │
│  Build:  wpkh([fingerprint/84h/0h/0h]xprv.../0/*)                    │
│  Crates:    bitcoin 0.32, bdk_wallet 0.3.1 (Descriptor types)        │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 3 — WALLET INSTANCE                                            │
│  Wallet::create_single(descriptor)?                                  │
│  .network(Network::Signet)                                           │
│  .create_wallet(&mut connection)?                                    │
│  → returns Result<Wallet, CreateWithPersistError>                     │
│  Crates:    bdk_wallet 3.1, bdk-wallet::rusqlite (or bdk-sqlite)      │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 4 — FIRST SYNC                                                 │
│  Pick sync strategy: full_scan (new wallet, no history) OR            │
│  start_sync_with_revealed_spks (wallet has metadata).                 │
│  Apply via wallet.apply_update(update)?                              │
│  Persist with wallet.persist(&mut connection)?                       │
│  Crates:    bdk_esplora 0.22 (P0) — or bdk_electrum, bdk_kyoto     │
└──────────────────────────────────────────────────────────────────────┘
                                  ↓
┌──────────────────────────────────────────────────────────────────────┐
│  STAGE 5 — BACKUP PROMPT (host responsibility)                       │
│  Display mnemonic words (one at a time).                              │
│  Verify 2 random words (Cake pattern).                                │
│  Host encrypts + persists mnemonic separately if user opts in.       │
│  Crates:    aes-gcm, argon2, zeroize (see §8)                       │
└──────────────────────────────────────────────────────────────────────┘
```

### Crates per stage

| Stage                         | Crate                             | Version     | Role                                                                                          |
| ----------------------------- | --------------------------------- | ----------- | --------------------------------------------------------------------------------------------- |
| 1. Generate / import mnemonic | `bip0039` (rust-bitcoin)          | 0.12        | Mnemonic generation from CSPRNG; mnemonic parsing; PBKDF2 seed derivation; passphrase support |
| 1. Derive root xprv from seed | `bip32` (rust-bitcoin)            | 0.5         | BIP32 root key derivation; hardened/normal path derivation                                    |
| 2. Build descriptor           | `bitcoin` (rust-bitcoin)          | 0.32        | `Descriptor<PublicKey, SecretKey>` type + parse/format machinery                              |
| 2. Wrap descriptor for BDK    | `bdk_wallet::keys`                | 3.1         | `DescriptorSecretKey`, `DescriptorPublicKey`, `KeychainKind`                                  |
| 3. Create wallet instance     | `bdk_wallet`                      | 3.1         | `Wallet::create_single(descriptor)?.network(Network::Signet).create_wallet(&mut conn)?`       |
| 3. Persist wallet state       | `bdk-wallet::rusqlite` (built-in) | n/a         | SQLite persistence; works with multiple wallets in one DB                                     |
| 4. Chain sync                 | `bdk_esplora`                     | 0.22        | First-time full scan via `client.full_scan(req, STOP_GAP, PARALLEL_REQUESTS)?`                |
| 4. Apply sync                 | `bdk_wallet`                      | 3.1         | `wallet.apply_update(update)?`                                                                |
| 4. Persist sync result        | `bdk_wallet::rusqlite`            | n/a         | `wallet.persist(&mut conn)?`                                                                  |
| 5. Backup UX                  | `bip0039` + `zeroize`             | 0.12 + 1.7+ | `Zeroizing<Mnemonic>` for memory; `aes-gcm` + `argon2` for storage encryption (§8)            |

### Canonical `bcs-bitcoin` v1 add-wallet flow

```rust
use bdk_wallet::keys::bip39::Mnemonic;
use bdk_wallet::keys::{DescriptorSecretKey, ExtendedKey, DerivationPath};
use bdk_wallet::keys::Descriptor;
use bdk_wallet::{Wallet, KeychainKind};
use bdk_wallet::bitcoin::{Network, secp256k1::XPrv};
use bdk_wallet::rusqlite::Connection;
use bdk_esplora::EsploraExt;
use esplora_client::Builder;
use zeroize::Zeroizing;

const STOP_GAP: usize = 50;
const PARALLEL_REQUESTS: usize = 8;

fn add_wallet(
    conn: &mut Connection,
    network: Network,
    mnemonic_words: usize,    // 12 or 24
    passphrase: &str,         // "" if none
) -> Result<(Wallet, Zeroizing<Mnemonic>), anyhow::Error> {
    // 1. Mnemonic (new or imported — host decides)
    let mnemonic = if mnemonic_words == 0 {
        // Host-managed new wallet
        Zeroizing::new(Mnemonic::generate(12)?)  // 12 words = 128-bit entropy
    } else {
        // Imported — caller passes words as &[&str]
        Zeroizing::new(Mnemonic::parse_in(normalized, word_count, Language::English)?)
    };
    let seed = mnemonic.to_seed(passphrase);

    // 2. Root xprv → descriptor (account 0, BIP-86 single-key Taproot)
    let root: XPrv = XPrv::new(&seed)?;
    let account_path: DerivationPath = "m/86'/0'/0'".parse()?;
    let account_xprv = root.derive_path(&account_path)?;
    let descriptor: Descriptor<_, DescriptorSecretKey> =
        format!("tr({}/0/*)", account_xprv).parse()?;

    // 3. Create wallet instance + persist
    let wallet = Wallet::create_single(&descriptor)?
        .network(network)
        .create_wallet(conn)?;

    // 4. First-time full sync
    let client = Builder::new("https://mutinynet.com/api").build_blocking();
    let req = wallet.start_full_scan();
    let update = client.full_scan(req, STOP_GAP, PARALLEL_REQUESTS)?;
    wallet.apply_update(update)?;
    wallet.persist(conn)?;

    // 5. Return mnemonic for host-side backup UX (Zeroizing drops on scope exit)
    Ok((wallet, mnemonic))
}
```

### Why this stack for v1

- **`bdk_wallet 3.1`** is the canonical Bitcoin wallet core in Rust — 1.1M+ downloads, used by Bitkey, ProtonWallet, Bull Bitcoin. Documented in `bdk_wallet/examples/` and the Book of BDK.
- **`bip0039 0.12`** (rust-bitcoin umbrella) — same maintainers as `bitcoin` and `miniscript`. CC0-1.0. Universal interop.
- **`bip32 0.5`** (rust-bitcoin umbrella) — needed for xprv → account derivation.
- **`bdk-wallet::rusqlite`** (built-in feature) — no extra dep; supports multiple wallets in one DB.
- **`bdk_esplora 0.22`** — primary chain source per Locked Decision §B.3.
- **`aes-gcm` + `argon2` + `zeroize`** — see §8 for the seed-at-rest crypto.

### Multi-wallet persistence (multiple `Wallet` instances in one DB)

`bdk-wallet::rusqlite` supports multiple wallets per SQLite file via per-wallet table partitioning (see `bdk_wallet::rusqlite` source). For `bcs-bitcoin`, the recommended pattern is:

- Each user session = one SQLite file.
- Each wallet the user creates = one `bdk_wallet::Wallet` instance + a wallet-id row in a top-level `wallets` table.
- Descriptor set, tx history, labels, UTXO graph — all stored per-wallet.
- Mnemonic / xprv — stored SEPARATELY (in a different encrypted file) so SQLite never sees the plaintext.

```rust
// Schema sketch (host-managed)
CREATE TABLE wallets (
    id          TEXT PRIMARY KEY,    -- UUID
    name        TEXT NOT NULL,
    network     INTEGER NOT NULL,
    descriptor  TEXT NOT NULL,        -- public-only; private kept in separate keystore
    birthday    INTEGER NOT NULL,    -- first sync height hint
    created_at  INTEGER NOT NULL
);
```

### New wallet vs new account

| New wallet                              | New account                                           |
| --------------------------------------- | ----------------------------------------------------- |
| New mnemonic (or imported xprv)         | Existing mnemonic, new BIP-44 account index           |
| Independent address space               | Same mnemonic, disjoint derivation path               |
| `Wallet::create_single(new_descriptor)` | `Wallet::create_single(bumped_descriptor)`            |
| Persist as a separate wallet row        | Same wallet table, new descriptor row                 |
| Recovery = restore mnemonic             | Recovery = restore mnemonic + use right account index |

Both flows share the same crates; only the descriptor builder differs.

### What `bcs-bitcoin` v1 should adopt (locked)

1. **Mnemonic format:** 12-word BIP39 English (128-bit entropy), `Zeroizing<Mnemonic>` wrapper.
2. **First account index:** 0 (`m/86'/0'/0'` for Taproot, `m/84'/0'/0'` for Native SegWit — pick one and document).
3. **Persist:** mnemonic + xprv in a separate encrypted file; descriptor + UTXOs + labels in the BDK SQLite DB.
4. **First sync:** full scan with `STOP_GAP=50`, `PARALLEL_REQUESTS=8` (tune per environment).
5. **Backup UX:** display words one at a time, verify 2 random words (Cake pattern).
6. **No cloud backup.** User backs up manually or via BIP-139 export file.

---

## Crates & SDKs at a glance

Priority levels:

- **P0 (Core)** — required for any wallet; no fallback.
- **P1 (Production)** — stable, well-maintained, recommended default.
- **P2 (Optional)** — additive feature; pull in when needed.
- **P3 (Niche)** — experimental / low-traction / multi-coin; avoid unless specific need.

| Crate / SDK            | Version    | Purpose                            | Used for                   | Prio   | Repo / Stars / Maintained                                                                                                                                                         |
| ---------------------- | ---------- | ---------------------------------- | -------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bdk_wallet`           | 3.1.0      | Descriptor-based wallet core       | Wallet, sync, PSBT build   | **P0** | [bitcoindevkit/bdk_wallet](https://github.com/bitcoindevkit/bdk_wallet) · 1.1M+ downloads · **Active** (v3.1.0 Jun 2026, MSRV 1.85)                                               |
| `bdk_chain`            | 0.23.3     | Chain data structures (required)   | Wallet state core          | **P0** | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) · 1.06k ★ · **Active** (required dep of `bdk_wallet`)                                                                   |
| `bitcoin`              | 0.32.8     | Bitcoin types + PSBT (required)    | Workspace types            | **P0** | [rust-bitcoin/bitcoin](https://github.com/rust-bitcoin/rust-bitcoin) · 2.6k ★ · **Active** (CC0-1.0; required dep of `bdk_wallet`)                                                |
| `miniscript`           | 12.3       | Descriptor / policy compiler       | Spending policies          | **P0** | [rust-bitcoin/rust-miniscript](https://github.com/rust-bitcoin/rust-miniscript) · 420 ★ · **Active** (CC0-1.0, rust-bitcoin org; pinned to ^12.x per `bdk_wallet 3.1` constraint) |
| `secp256k1`            | 0.32       | ECDSA + Schnorr + MuSig2           | Signing crypto             | **P0** | [rust-bitcoin/rust-secp256k1](https://github.com/rust-bitcoin/rust-secp256k1) · 428 ★ · **Active** (CC0-1.0, Bitcoin Core team)                                                   |
| `bip32`                | 0.5        | BIP32 HD derivation (rust-bitcoin) | Key chains (secp256k1)     | **P0** | [rust-bitcoin/bitcoin](https://github.com/rust-bitcoin/rust-bitcoin) · 2.6k ★ · **Active**                                                                                        |
| `aes-gcm` (RustCrypto) | 0.11.0     | AES-256-GCM AEAD                   | Storage encryption         | **P0** | [RustCrypto/AEADs](https://github.com/RustCrypto/AEADs) · 130M+ downloads · **Active** (NCC-audited, MobileCoin-funded)                                                           |
| `argon2` (RustCrypto)  | latest     | Argon2id KDF                       | Password → cipher key      | **P0** | [RustCrypto/password-hashes](https://github.com/RustCrypto/password-hashes) · **Active** (OWASP-recommended; GPU/ASIC-resistant)                                                  |
| `zeroize`              | 1.7+       | Secure memory wiping               | Sensitive buffers          | **P0** | [RustCrypto/utils](https://github.com/RustCrypto/utils) · **Active** (Drop impls for mnemonic / key / password)                                                                   |
| `bdk_esplora`          | 0.22       | Esplora HTTP chain source          | Block fetch                | **P0** | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) · 1.06k ★ · **Active** (primary chain source for v1)                                                                    |
| `bdk-wallet::rusqlite` | built-in   | SQLite persistence                 | Wallet state               | **P0** | (bdk_wallet feature) · **Active** (built-in; v1 default)                                                                                                                          |
| `bip0039`              | 0.12       | BIP39 mnemonic + seed              | Mnemonic generation        | **P2** | (rust-bitcoin umbrella) · **Active** (downgraded from P0)                                                                                                                         |
| `chacha20poly1305`     | latest     | ChaCha20-Poly1305 AEAD             | Alt storage encryption     | **P2** | [RustCrypto/AEADs](https://github.com/RustCrypto/AEADs) · **Active** (mobile / no AES-NI)                                                                                         |
| `bdk-labels`           | 0.1.0      | BIP-329 wallet labels              | Tx / address labels        | **P1** | [Musab1258/bdk-labels](https://github.com/Musab1258/bdk-labels) · **Active**                                                                                                      |
| `bdk_electrum`         | 0.23       | Electrum chain source              | Block fetch (alt)          | **P2** | [bitcoindevkit/bdk](https://github.com/bitcoindevkit/bdk) · 1.06k ★ · **Active**                                                                                                  |
| `electrum-client`      | 0.25       | Electrum protocol client           | Block fetch (alt)          | **P2** | [bitcoindevkit/rust-electrum-client](https://github.com/bitcoindevkit/rust-electrum-client) · 87 ★ · **Active**                                                                   |
| `esplora-client`       | 0.13       | Esplora REST client                | Block fetch (alt)          | **P2** | [bitcoindevkit/rust-esplora-client](https://github.com/bitcoindevkit/rust-esplora-client) · 52 ★ · **Active** (1.4M downloads, 21 reverse deps)                                   |
| `bdk-sqlite`           | latest     | Async SQLite via sqlx              | Wallet state (async)       | **P2** | [bitcoindevkit/bdk-sqlite](https://github.com/bitcoindevkit/bdk-sqlite) · 3 ★ · **Active** (Oct 2025)                                                                             |
| `bip157` (Kyoto)       | 0.6.3      | BIP-157/158 compact block filters  | Private SPV sync           | **P2** | [2140-dev/kyoto](https://github.com/2140-dev/kyoto) · 87 ★ · **Active** (Jul 2026)                                                                                                |
| `bdk_kyoto`            | 0.16       | BDK-native Kyoto integration       | Private SPV sync (BDK)     | **P2** | [bitcoindevkit/bdk-kyoto](https://github.com/bitcoindevkit/bdk-kyoto) · 19 ★ · **Active** (0.16+ for BDK-3 compat)                                                                |
| `payjoin`              | 0.16       | BIP-77/78 PayJoin                  | Privacy-enhanced sends     | **P2** | [payjoin/rust-payjoin](https://github.com/payjoin/rust-payjoin) · 155 ★ · **Active** (OpenSats funded; Bull Bitcoin + Cake Wallet production)                                     |
| `ldk-node`             | 0.7.0      | Lightning node (LDK + BDK)         | Lightning channels         | **P2** | [lightningdevkit/ldk-node](https://github.com/lightningdevkit/ldk-node) · 207 ★ · **Active** (v0.7.0 Dec 2025)                                                                    |
| `hd-wallet`            | 0.7.0      | SLIP-10 + ed25519 + Stark          | Multi-curve key chains     | **P3** | [LFDT-Lockness/hd-wallet](https://github.com/LFDT-Lockness/hd-wallet) · **Active** (129K downloads; only if multi-curve)                                                          |
| `rustywallet-mempool`  | 0.2.0      | Mempool.space REST client          | Fee estimates, broadcast   | **P3** | [nirvagold/rustywallet](https://github.com/nirvagold/rustywallet) · 2 ★ · **Low traction** (41 downloads, 0 dependents, single maintainer)                                        |
| `bitcoin-fees`         | 0.1.21     | Core-compatible fee primitives     | Custom fee estimator       | **P3** | [klebs6/bitcoin-rs](https://github.com/klebs6/bitcoin-rs) · **Niche** (single-maintainer fork; only if Core-compat needed)                                                        |
| `niebla-158`           | 0.1.1      | CBF engine + SQLite store          | Private SPV (lean, no BDK) | **P3** | [docs.rs/niebla-158](https://docs.rs/niebla-158) · **Newer, low-traction** (alt to Kyoto)                                                                                         |
| `bdk_redb`             | exp.       | redb backend                       | Wallet state (alt, exp.)   | **P3** | [bitcoindevkit/bdk PR #1914](https://github.com/bitcoindevkit/bdk) · **Experimental** (Summer of Bitcoin 2025)                                                                    |
| `redb_wallet_storage`  | 0.1.1      | redb backend (async + sync)        | Wallet state (alt)         | **P3** | [pingu-73/redb_wallet_storage](https://github.com/pingu-73/redb_wallet_storage) · 2 ★ · **Prototype** (1.3K downloads, marked prototype)                                          |
| `bip0032`              | latest     | BIP32 + SLIP-0010 (RustCrypto)     | Key chains (alt)           | **P3** | [RustCrypto/elliptic-curves](https://github.com/RustCrypto/elliptic-curves) · **Active** (alt to bip32)                                                                           |
| `khodpay-bip39/32/44`  | latest     | Production-grade BIP39/32/44       | Key chains (validated)     | **P3** | [khodpay/rust-wallet](https://github.com/khodpay/rust-wallet) · 3 ★ · **Single maintainer** (abolfazlbeh)                                                                         |
| `rust-hdwallet`        | 0.1.0-beta | 200+ coin HD wallet (python port)  | Multi-coin wallets         | **P3** | [itsm3abena/rust-hdwallet](https://github.com/itsm3abena/rust-hdwallet) · 10 ★ · **Beta** (APIs unstable)                                                                         |

---

## Appendix

Material below is reference / decision-log. The Technical Reference above is the implementation guide.

---

## A. Recommended `bcs-bitcoin` Cargo.toml

```toml
[dependencies]
# ── Workspace ──
bcs-common       = { path = "../bcs-common" }
bcs-providers    = { path = "../bcs-providers" }

# ── Bitcoin core (P0: required deps of bdk_wallet) ──
bitcoin          = { version = "0.32", features = ["serde", "rand"] }
miniscript       = "12.3"     # capped at ^12.x by bdk_wallet 3.1; bump when BDK loosens
secp256k1        = { version = "0.32", features = ["global-context", "rand"] }
bdk_wallet       = { version = "3.1", features = ["rusqlite", "compiler", "keys-bip39"] }

# ── Network clients (P0: chain source + storage) ──
bdk_esplora      = "0.22"
bdk_electrum     = { version = "0.23", optional = true }
electrum-client  = { version = "0.25", optional = true }
esplora-client   = "0.13"

# ── SPV (P2: privacy-first sync, optional) ──
bdk_kyoto        = { version = "0.16", optional = true }

# ── BIP-329 labels (P1) ──
bdk-labels       = { version = "0.1", optional = true }

# ── HD wallet primitives ──
bip32            = { version = "0.5", features = ["secp256k1"] }
bip0039          = { version = "0.12", features = ["english"], optional = true }
                                                      # P2: only if host generates/imports mnemonics

# ── PSBT / Lightning / advanced ──
payjoin           = { version = "0.16", optional = true }
# ldk-node: deferred (see Open / future §C)

# ── Async + error ──
tokio             = { version = "1.40", features = ["full"] }
async-trait       = "0.1"
thiserror         = "1.0"
tracing           = "0.1"

[dev-dependencies]
wiremock          = "0.6"
proptest          = "1.4"
bitcoind          = "0.32"
bitcoin           = "0.32.8"   # align dev-dep with runtime bitcoin version
```

### Feature flags

```toml
[features]
default = ["labels", "esplora"]
labels = ["dep:bdk-labels"]
esplora = []                              # bdk_esplora + esplora-client are hard deps (P0 chain source)
electrum = ["dep:bdk_electrum", "dep:electrum-client"]
spv = ["dep:bdk_kyoto"]                   # compact block filter sync
payjoin = ["dep:payjoin"]
# lightning = ["dep:ldk-node"]             # deferred to Open / future
mnemonic = ["dep:bip0039"]                # enable only when host generates/imports mnemonics
```

---

## B. Locked decisions

These are the load-bearing v1 choices. Inline mentions in §1–§9 cross-reference back here.

1. **BDK version** — `bdk_wallet 3.1` (MSRV 1.85). See §1.0 metadata. **Reversal cost:** downgrade to BDK 2.4 requires rewriting the §1.1 skeleton (sync/PSBT/signer paths are 3.0-specific) — estimate ~2-3 days rework.
2. **Storage** — `rusqlite` via `bdk_wallet::rusqlite` for v1. Move to `bdk-sqlite` (P2 in Cargo.toml, feature-gated) when async surface is needed. Migration plan deferred to v2.
3. **Chain source** — `bdk_esplora` for v1 (HTTP REST, easiest ops). Electrum as second backend via the `electrum` feature flag. Compact block filters (`bdk_kyoto`) via `spv` feature only when user explicitly wants privacy. `bdk_bitcoind_rpc` for self-hosted Bitcoin Core (P2, deferred to v2 — see §C.5).
4. **PSBT signing flow** — Public descriptor in `Wallet`; signing via `Psbt::sign` from rust-bitcoin. See §1.1 PSBT pattern.
5. **Fee estimation** — `bdk_esplora` `GET /v1/fees/recommended` (no new dep). `bdk_electrum` `blockchain.estimatefee` as fallback via the `electrum` feature. `rustywallet-mempool` (P3) deferred to v2 with a `mempool-space` feature gate. `bitcoin-fees` (P3) only if implementing a Core-compatible estimator. See §5 for full rationale.
6. **HD derivation** — Use `bip32` (rust-bitcoin). `bip0039` is P2 / optional — only if host generates/imports mnemonics. Avoid mixing with `slip-10` (deprecated). See §1.0 metadata.
7. **Labels** — `bdk-labels` with BIP-329 import/export (`labels` feature flag). See §6.
8. **Multisig** — v1 supports **single-sig only**. Multi-key / threshold wallets via `miniscript` policies are out of scope; revisit when user demand materializes.

## C. Open / future

Deferred to v2. Each entry has an explicit trigger (what makes it worth re-evaluating).

1. **rustywallet-mempool** — currently P3 (2 ★, 41 downloads, 0 dependents, single maintainer). Re-evaluate at P2 if (a) crate exceeds 3 ★ or (b) maintainer publishes a 90-day-active release. Otherwise use BlockBook / Electrum for fee source.
2. **Multisig / multi-account** — see Locked decision #8 — out of scope for v1. Trigger: a paying customer requesting multi-key custody.
3. **Self-hosted `bdk_bitcoind_rpc`** — listed in §1 workspace tree but not pinned in Cargo.toml. Add as optional chain source when a self-hosted-node use case activates.
4. **Don't-build baseline** — alternative is to call BDK directly from the consumer app (Rust or via FFI), bypassing `bcs-bitcoin` entirely. If `bcs-bitcoin` scope stays Bitcoin-only, validate the wrapper earns its keep vs. direct BDK consumption.

---

## D. Sources

### BDK 3.x + Book of BDK

- https://docs.rs/bdk_wallet/latest/bdk_wallet/
- https://crates.io/crates/bdk_wallet (v3.1.0, 2026-06-14, 1.1M downloads)
- https://github.com/bitcoindevkit/bdk_wallet
- https://bitcoindevkit.github.io/book-of-bdk/cookbook/transactions/transaction-builder/
- https://bitcoindevkit.github.io/book-of-bdk/cookbook/persistence/sqlite/
- https://bitcoindevkit.github.io/book-of-bdk/release-guide/3.0/psbt-signing/
- https://github.com/bitcoindevkit/book-of-bdk/blob/main/examples/rust/full-wallet/src/main.rs
- https://github.com/bitcoindevkit/bdk_wallet/blob/master/examples/electrum.rs (RBF fee bump example)
- https://github.com/bitcoindevkit/bdk-sqlite/

### SPV / Compact Block Filters

- https://github.com/2140-dev/kyoto (formerly rustaceanrob/kyoto)
- https://crates.io/crates/bip157 (v0.6.3, 91K downloads)
- https://docs.rs/bdk_kyoto/latest/bdk_kyoto/
- https://bitcoindevkit.github.io/book-of-bdk/cookbook/syncing/kyoto/
- https://docs.rs/crate/niebla-158/latest (0.1.1, SQLite store built-in)

### Persistent storage

- https://docs.rs/bdk_redb/latest/bdk_redb/
- https://github.com/pingu-73/redb_wallet_storage
- https://docs.rs/redb_wallet_storage/latest/redb_wallet_storage/
- http://blog.summerofbitcoin.org/redb-for-bdk/

### HD key management

- https://docs.rs/bip32/latest/bip32/ (rust-bitcoin)
- https://docs.rs/bip0039/latest/bip0039/
- https://docs.rs/bip0032/latest/bip0032/ (RustCrypto)
- https://docs.rs/hd-wallet/latest/hd_wallet/ (v0.7.0, LFDT-Lockness)
- https://docs.rs/slip-10/latest/slip_10/ (deprecated)
- https://crates.io/crates/rust-hdwallet (0.1.0-beta, itsm3abena)
- https://github.com/khodpay/rust-wallet
- https://github.com/satoshilabs/slips/blob/master/slip-0039.md

### Mempool / Fee estimation

- https://crates.io/crates/rustywallet-mempool (v0.2.0, Jan 2026, 41 downloads)
- https://crates.io/crates/bitcoin-fees (v0.1.21)
- https://crates.io/crates/mempool_space (v0.0.60, RandyMcMillan)

### BIP-329 Labels

- https://crates.io/crates/bdk-labels (0.1.0)
- https://github.com/Musab1258/bdk-labels

### Production wallet comparisons

- https://www.spark.money/tools/bitcoin-dev-kit-comparison (BDK vs LDK vs bitcoinj, mid-2026)
- BDK production users list: bitcoindevkit.org

---

**End of research.** Pairs with `2026-08-04-bcs-bitcoin-unified-reference.md`. Apply revision dated 2026-08-04 (post-review): miniscript pinned to `^12.x` per `bdk_wallet 3.1` constraint; `bdk_kyoto 0.16+` for BDK-3 compat; `bdk` dead feature removed; `rustywallet-mempool` moved out of Locked Decisions (use BlockBook/Electrum primary); `bip0039` demoted to P2; P0 reframe now reflects runtime minimum (chain source + storage included); multisig declared out-of-scope for v1; hardware-wallet integration declared out-of-scope.
