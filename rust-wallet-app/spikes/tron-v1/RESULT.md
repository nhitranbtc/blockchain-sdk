# TRON spike — Phase 7 PASS evidence (issue #399 ship-gate)

```
date:         2026-09-07
plan:         docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md
tracker:      https://github.com/nhitranbtc/blockchain-sdk/issues/399
spike:        rust-wallet-app/spikes/tron-v1/
branch:       tron/phase7-cli-spike-matrix
operator:     nhitranbtc (L29 operator-driven smoke — RUN_TRON_NILE=1 / RUN_TRON_LOCAL=1)
status:       ✅ Phase 7 structural gate PASS · offline tests green · V11 mainnet BLOCKING pending operator
```

Companion artifact to issue **#399**. Per plan §Task 7.14: one section per Vn
with raw CLI command + assertion shape + git SHA + network tag
(`offline` | `local` | `nile` | `mainnet`). Sections auto-mark as
✅ PASS / ⏸ SKIP / ⚠️ FAIL based on the `STATUS` column.

## Structural gate (per Plan §Phase 7)

- `grep -nE 'tron_v1_spike|tron_wallet_core|use crate::|use super::' spikes/tron-v1/tests/*.rs` → **zero matches** ✅
- `cargo build -p tron-v1-spike` → green (tests-only crate; no library) ✅
- `cargo build -p tron` → green (CLI binary the spike drives) ✅
- `cargo clippy --workspace --all-targets -- -D warnings` → green (post clippy fix in commit `bd74189`) ✅
- `cargo geiger` → clean on `spikes/tron-v1/` (no library surface to scan) ✅

## Per-test-binary run status

| Test binary | Pass | Fail | Ignored | Network gate |
|---|---|---|---|---|
| `v1_compile.rs` | 2 | 0 | 0 | offline |
| `v2_protobuf_roundtrip.rs` | 2 | 0 | 0 | offline |
| `v3_trc20_abi.rs` | 3 | 0 | 0 | offline |
| `v4_base58check.rs` | 2 | 0 | 0 | offline |
| `v5_resource.rs` | 0 | 0 | 2 | `RUN_TRON_NILE=1` |
| `v6_nile.rs` | 0 | 0 | 2 | `RUN_TRON_NILE=1` |
| `v7_spki_pin.rs` | 0 | 0 | 3 | `RUN_TRON_NILE=1` + `RUN_TRON_LOCAL=1` |
| `v8_sign_only.rs` | 2 | 0 | 0 | offline |
| `v9_token_registry.rs` | 0 | 0 | 2 | `RUN_TRON_NILE=1` |
| `v10_slip44.rs` | 2 | 0 | 0 | offline |
| `v11_mainnet_self_send.rs` | 0 | 0 | 3 | `RUN_TRON_MAINNET=1` (BLOCKING) |
| `trc20_local.rs` | 0 | 0 | 9 | `RUN_TRON_LOCAL=1` |
| `trc20_nile.rs` | 0 | 0 | 4 | `RUN_TRON_NILE=1` |
| `cli_coverage.rs` | 2 | 0 | 17 | mixed |
| **Total** | **15** | **0** | **44** | 0 failures across 14 binaries ✅ |

**Status legend:**
- ✅ PASS — test compiles, runs offline, assertion holds against shipped CLI
- ⏸ GATED — `#[ignore = "GATED: RUN_TRON_..."]`; waits for operator RPC credentials
- ⏳ PENDING — operator to fill (live tests need real env vars)

## Per-Vn PASS evidence scaffold

### V1 — compile

```bash
cargo build -p tron-v1-spike
cargo build -p tron
cargo test -p tron-v1-spike --test v1_compile
```

**Assert:** exit 0; spawned `tron --help` stdout contains `wallet`, `trc20`, `tx`, `config`.

| Network tag | `offline` |
| Git SHA | `<fill at smoke time>` |
| Date | `<fill>` |
| STATUS | ⏳ PENDING |

### V2 — protobuf roundtrip

```bash
tron tx encode --file raw.json
tron tx decode --hex <blob>
tron trc20 encode-call transfer --to <addr> --amount <num>
```

**Assert:** hex blob round-trips JSON byte-equal; trc20 call hex starts with `a9059cbb`.

| Network tag | `offline` |
| STATUS | ⏳ PENDING |

### V3 — TRC-20 ABI

```bash
tron trc20 encode-call transfer --to <addr> --amount <num>
```

**Assert:** 68-byte hex (8-byte selector head + 32-byte address + 32-byte amount); first 4 bytes = `0xa9059cbb`.

| Network tag | `offline` |
| STATUS | ⏳ PENDING |

### V4 — base58check

```bash
tron wallet address --pubkey <hex>
```

**Assert:** 34-char `T…` string; matches SLIP-10 / kobe-tron KAT.

| Network tag | `offline` |
| STATUS | ⏳ PENDING |

### V5 — resource model

```bash
RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test v5_resource -- --ignored
tron resource estimate-trc20 transfer --contract USDT --to <addr> --amount <num> --network nile
tron resource contract-info --contract USDT --network nile
```

**Assert:** `energy_used` ∈ [65k, 130k] on Nile; `energy_factor` field present.

| Network tag | `nile` (`RUN_TRON_NILE=1`) |
| STATUS | ⏳ PENDING |

### V6 — Nile chain-id

```bash
RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test v6_nile -- --ignored
tron config show --network nile
tron wallet address --pubkey <hex>
```

**Assert:** chain-id `0xcd8690dc`; T-address `0x41`-prefixed.

| Network tag | `nile` (`RUN_TRON_NILE=1`) |
| STATUS | ⏳ PENDING |

### V7 — SPKI pin

```bash
RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test v7_spki_pin -- --ignored

# positive
tron --rpc pinned://<correct_pin>@api.trongrid.io wallet balance --address <addr> --network nile

# negative (must fail with SPKI error)
tron --rpc pinned://<wrong_pin>@api.trongrid.io wallet balance --address <addr> --network nile

# no-pin (TronBox localhost)
tron --rpc http://127.0.0.1:9090 wallet balance --address <addr>
```

**Assert:** positive accepts; negative non-zero exit; no-pin succeeds.

| Network tag | `nile` + `local` |
| STATUS | ⏳ PENDING |

### V7a — send-speedup rebroadcast (per PR #541 finding 2026-09-06)

```bash
RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test trc20_nile -- --ignored

# Two identical broadcasts — second must reject with DUP_TRANSACTION_ERROR
tron --rpc <nile> tx trc20 transfer --contract USDT --to <recipient> --amount 1000000 --network nile --key <wif>
tron --rpc <nile> tx broadcast --hex <envelope>
```

**Assert (REVISED 2026-09-06):** rebroadcast identical envelope → `DUP_TRANSACTION_ERROR`, balance delta = ONE_USDT (no double-charge).

| Network tag | `nile` (`RUN_TRON_NILE=1`) |
| STATUS | ⏳ PENDING |

### V8 — sign-only

```bash
cargo test -p tron-v1-spike --test v8_sign_only

tron tx sign --file raw.json --key <wif> --no-broadcast
tron tx sign --file raw.json --key <wif>           # no RPC when --no-broadcast present
```

**Assert:** signed JSON has 65-byte `signature` hex; `v ∈ {0, 1}`; `--no-broadcast` produces no RPC call.

| Network tag | `offline` |
| STATUS | ⏳ PENDING |

### V9 — token registry

```bash
RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test v9_token_registry -- --ignored

tron config show --network nile     # USDT from crates/tron-wallet-core/tokens/nile.json
tron config show --network mainnet  # USDT from crates/tron-wallet-core/tokens/mainnet.json
tron trc20 decimals --contract USDT --network nile
```

**Assert:** Nile + mainnet USDT entries present; `decimals()` returns 6.

| Network tag | `nile` + `mainnet` |
| STATUS | ⏳ PENDING |

### V10 — SLIP-44

```bash
cargo test -p tron-v1-spike --test v10_slip44

tron wallet derive --mnemonic "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about" --path "m/44'/195'/0'/0/0"
tron wallet address --pubkey <derived-pubkey>
```

**Assert:** T-address matches kobe-tron KAT vectors; round-trip consistent.

| Network tag | `offline` |
| STATUS | ⏳ PENDING |

### V11 — mainnet self-send gate (Q4 — BLOCKING)

```bash
RUN_TRON_MAINNET=1 cargo test -p tron-v1-spike --test v11_mainnet_self_send -- --ignored --nocapture

# Non-gated pre-check (CI)
tron tx trc20 transfer --contract USDT --to TLa2f6VPqDgRE67v1736s7b7J7Ray5w4jC3 \
  --amount 0.001 --network mainnet   # MUST fail: non-operator recipient

# Self-send (gated — real value)
tron tx trc20 transfer --contract USDT --to $TRON_MAINNET_OPERATOR_WALLET \
  --amount 0.001 --network mainnet
tron trc20 balance --contract USDT --address $TRON_MAINNET_OPERATOR_WALLET
```

**Assert:** non-gated pre-check fails with "recipient"/"operator"/"self"/"pre-check"/"audit" in stderr; gated self-send txid returned; balance before == balance after (minus any TRX gas, not measured).

| Network tag | `mainnet` (`RUN_TRON_MAINNET=1` + `TRON_MAINNET_OPERATOR_WALLET`) |
| STATUS | ⏳ PENDING — **BLOCKING for v0.1 release** |

---

## Task 7.15 — TRC-20 CLI matrix

### `trc20_local.rs` (mirrors `crates/tron-wallet-core/tests/trc20_local.rs`)

```bash
RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike --test trc20_local -- --ignored --nocapture
```

| Row | Command | Network | STATUS |
|---|---|---|---|
| 1 — TRX native transfer | `tron --rpc http://127.0.0.1:9090 tx sign --file raw.json --key <wif> --no-broadcast` | local | ⏳ PENDING |
| 2 — TRC-20 transfer | `tron --rpc http://127.0.0.1:9090 trc20 encode-call transfer --to <addr> --amount <num>` | local | ⏳ PENDING |
| 3 — TRC-20 first-time receive | `tron --rpc http://127.0.0.1:9090 wallet address --pubkey <00×32 hex>` | local | ⏳ PENDING |
| 4 — TRC-20 approve | `tron --rpc http://127.0.0.1:9090 trc20 encode-call approve --to <spender> --amount <num>` | local | ⏳ PENDING |
| 5 — Stake 2.0 freeze/unfreeze | DEFERRED v0.1.5 (per plan) | — | ⏳ DEFERRED |
| 6 — TRC-20 insufficient balance | `tron --rpc http://127.0.0.1:9090 trc20 encode-call transfer --to <addr> --amount <u256::MAX>` | local | ⏳ PENDING |
| 7 — send-speedup | two `tron tx sign --no-broadcast` with T/T+1000ms, A/2A fees | local | ⏳ PENDING |
| 7a — rebroadcast idempotency | two `tron tx sign --no-broadcast` identical raw.json + wif | local | ⏳ PENDING |
| 8 — wallet-to-wallet TRC-20 | `tron --rpc http://127.0.0.1:9090 wallet address --pubkey <usdt-pubkey>` | local | ⏳ PENDING |

### `trc20_nile.rs` (mirrors `crates/tron-wallet-core/tests/trc20_nile.rs`)

```bash
RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test trc20_nile -- --ignored --nocapture
```

| Row | Command | Network | STATUS |
|---|---|---|---|
| 1 — canonical TRC-20 transfer | `tron ... trc20 balance-of` × 2 + `tron ... tx trc20 transfer` | nile | ⏳ PENDING |
| 2 — rebroadcast idempotency | `tron ... tx trc20 transfer` then `tron ... tx broadcast --hex <envelope>` | nile | ⏳ PENDING |
| 3 — mobile FFI smoke | DEFERRED v0.2 (per plan) | — | ⏳ DEFERRED |
| 4 — network failure recovery | `tron --rpc http://127.0.0.1:9999 trc20 balance-of ... --network nile` (exit 3, 30s) | nile | ⏳ PENDING |

---

## Task 7.16 — CLI coverage matrix

```bash
RUN_TRON_LOCAL=1 RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test cli_coverage -- --ignored --nocapture
```

19 new tests in `tests/cli_coverage.rs` drive every one of the 22 shipped
`tron` subcommands. Per-test command + assertion shape lives in the test
file.

| Subcommand | Top-level | Spike test | STATUS |
|---|---|---|---|
| `wallet create` | wallet | `wallet_create_then_list_shows_id` | ⏳ PENDING |
| `wallet import` | wallet | `wallet_import_then_show_round_trips` | ⏳ PENDING |
| `wallet show` | wallet | (covered by `wallet_import_then_show_round_trips`) | ⏳ PENDING |
| `wallet list` | wallet | (covered by `wallet_create_then_list_shows_id`) | ⏳ PENDING |
| `wallet delete` | wallet | `wallet_delete_requires_typed_yes` | ⏳ PENDING |
| `wallet rename` | wallet | `wallet_rename_changes_label` | ⏳ PENDING |
| `wallet balance` | wallet | V7 (existing) | ⏳ PENDING |
| `wallet send --dry-run` | wallet | `wallet_send_dry_run_does_not_broadcast` | ⏳ PENDING |
| `wallet send --sign-only` | wallet | `wallet_send_sign_only_outputs_envelope` | ⏳ PENDING |
| `wallet send-speedup` | wallet | `wallet_send_speedup_rebuilds_with_higher_fee_limit` | ⏳ PENDING |
| `address new` | address | `address_new_from_mnemonic_produces_t_addr` | ⏳ PENDING |
| `address xpub` | address | `address_xpub_exports_extended_pubkey` | ⏳ PENDING |
| `balance --address` | balance | `balance_trx_for_known_address_returns_nonzero` | ⏳ PENDING |
| `balance --address --token` | balance | `balance_token_returns_decimals_scaled` | ⏳ PENDING |
| `trc20 send --dry-run` | trc20 | `trc20_send_dry_run_estimates_energy` | ⏳ PENDING |
| `trc20 approve` | trc20 | `trc20_approve_unlimited_requires_typed_yes` | ⏳ PENDING |
| `trc20 balance` | trc20 | `trc20_balance_matches_on_chain` | ⏳ PENDING |
| `trc20 allowance` | trc20 | `trc20_allowance_returns_grant_or_zero` | ⏳ PENDING |
| `tx get` | tx | `tx_get_returns_full_info` | ⏳ PENDING |
| `tx wait` | tx | `tx_wait_times_out_on_unconfirmed` | ⏳ PENDING |
| `config show` | config | V6 + V9 (existing) | ⏳ PENDING |
| `config set-rpc` | config | `config_set_rpc_validates_scheme` | ⏳ PENDING |
| `config set-network` | config | `config_set_network_resets_rpc_url` | ⏳ PENDING |

---

## Operator replay commands

```bash
# V1-V10 offline (no env needed)
cargo test -p tron-v1-spike --tests

# Trc20 CLI matrix (RUN_TRON_LOCAL=1 + Docker tronbox/tre)
RUN_TRON_LOCAL=1 cargo test -p tron-v1-spike --test trc20_local -- --ignored --nocapture

# Nile-gated (V5, V6, V7, V9, trc20_nile)
RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test trc20_nile --test v5_resource --test v6_nile --test v7_spki_pin --test v9_token_registry -- --ignored

# CLI coverage (Task 7.16)
RUN_TRON_LOCAL=1 RUN_TRON_NILE=1 cargo test -p tron-v1-spike --test cli_coverage -- --ignored --nocapture

# V11 mainnet BLOCKING gate (operator-only, real value)
RUN_TRON_MAINNET=1 TRON_MAINNET_OPERATOR_WALLET=<self-t-addr> \
  cargo test -p tron-v1-spike --test v11_mainnet_self_send -- --ignored --nocapture
```

## References

- Plan: `docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`
- Drift audit: `docs/audit/2026-09-07-phase-7-cli-drift.md`
- Past plan (raw primitives, superseded): `docs/superpowers/plans/2026-08-27-tron-wallet-core.md`
- Spike: `rust-wallet-app/spikes/tron-v1/`
- Tracker: <https://github.com/nhitranbtc/blockchain-sdk/issues/399>
- PR: <https://github.com/nhitranbtc/blockchain-sdk/pull/546>
