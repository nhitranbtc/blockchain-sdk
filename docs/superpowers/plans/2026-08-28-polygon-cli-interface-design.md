# `polygon` CLI — Interface Design (T6 of #426 / #416)

**Date:** 2026-08-28
**Owner:** T6 / Phase 4 / Issue #426 (sub-task of #416 Q1 Option A)
**Status:** Interface design only — implementation lands in the next TDD phase per L13.
**Sibling docs:**
- Plan — `docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` §Phase 4 T6 (lines 262-298)
- User-stories — `docs/wallets/2026-08-27-polygon-wallet-user-stories.md` (31 stories, default = Amoy)
- ETH template — `docs/wallets/2026-08-23-eth-wallet-user-stories.md` (canonical mapping shape)
- Issue body — `gh issue view 426` (T6 = scaffold `polygon` CLI; T7 = Amoy smoke; T8 = mainnet + ETH regression; T9 = release cut)

---

## 1. Goal

Deliver a read-only **interface design** for the `polygon` CLI binary, scoped to T6 specifically. The next phase (L13 step 5 — TDD) will create the files referenced here. This doc records:

- The clap subcommand tree + every flag per command (covering 31 user-stories + 3 cross-cutting, per plan §T6 Step 2 line 272).
- The `Cargo.toml` deps and the module file tree (`main.rs` + `cli.rs` + `handlers/{mod,wallet,tx,erc20,fee,config,faucet,sign}.rs`).
- Per-file responsibilities (signatures + doc comments only; no `fn body`).
- The critical-tier L13 threat mitigations encoded in the type/shape of the interface (chain_id validation on EIP-712, Zeroizing wrap on mnemonic reads, env-var remove pattern, SPKI-pin placeholder).
- A failing-test-first test plan (L13 step 3) per command batch.
- The L12 review pre-flight findings + drift items the implementer should expect.

**Out of scope for this design doc:** implementation code bodies, PR body text, operator-runbook, the actual T7/T8/T9 task bodies (those have their own task issue entries).

---

## 2. Drift from plan

Plan §T6 (`docs/superpowers/plans/2026-08-27-polygon-wallet-core.md` lines 266-268) lists two files:

```text
- Create: rust-wallet-app/crates/polygon/Cargo.toml
- Create: rust-wallet-app/crates/polygon/src/main.rs
```

This interface design deviates in **five places**, each justified below.

### 2.1 Drift #1 — split `main.rs` into `main.rs` + `cli.rs` + `handlers/*`

- **Plan:** `main.rs` carries everything.
- **Design:** `main.rs` is a thin dispatcher (`Cli::parse` → match arms → handler call); `cli.rs` carries clap derive types; `handlers/*` carries per-command-batch logic.
- **Why:** T6 covers 31 user-stories across 10 command groups (wallet, tx, erc20, fee, config, faucet, sign-message, sign-typed). Plan §T6 Step 2 line 272 alone is ~25 flag-bearing subcommands. `eth/src/main.rs` (closest analog) is 890 lines and mixes clap derives + dispatch + password prompt + tracing init. Per L12 type-design findings (echoed in PRs #352, #384, #390 — `eth` CLI's `main.rs` was repeatedly flagged for carrying too many concerns), splitting improves review surface.
- **Precedent:** `rust-wallet-app/crates/btc/src/{main,cli,handlers}.rs` (the same split exists and is the canonical pattern when commands exceed ~5 subcommands). ETH CLI is the outlier (single-file) and is the source of the L12 findings we are avoiding.

### 2.2 Drift #2 — `parse_cli` is family-scoped, not Polygon-only at the CLI layer

- **Plan §T6 Step 2 line 273** lists `--network amoy|mainnet|anvil`. Amoy + mainnet are Polygon. Anvil (chain_id 31337) is Ethereum.
- **Design:** Default `--network amoy`. `--network mainnet` is ambiguous in a Polygon-flavoured CLI — interpret `mainnet` as Polygon mainnet (chain_id 137) to match the family context. `--network anvil` (31337) is **rejected** with `Error::InvalidInput` ("polygon-cli targets Polygon PoS; for Anvil regtest, run the `eth` CLI with `--network anvil`"). Rationale: the `eth` CLI already supports Anvil; duplicating it in `polygon` invites the cross-chain identity footgun (`polygon-wallet-core/src/disambig.rs` documents this concern at line 13: positive USDC identity is the caller's responsibility).
- **Implementation helper:** `polygon_wallet_core::network::PolygonChain::parse_cli` (defined at `evm-wallet-core/src/network.rs:228-238`). The CLI calls this directly; anvil is rejected by an explicit `Err(Error::InvalidInput(...))` arm added to a thin `cli.rs::parse_network(s: &str) -> Result<Network>` wrapper that delegates to `PolygonChain::parse_cli` and maps `Network::Polygon(...)`.

### 2.3 Drift #3 (acknowledged, not a deviation) — `pinned://` SPKI scheme

- **Plan §Q7 acceptance criterion:** T8 verifies `pinned://<spki>@polygon-rpc.com` succeeds.
- **Design:** Reserve `--rpc-url` parser to accept either `http(s)://` or `pinned://<spki>@<host>`. The `pinned://` branch carries the SPKI pin through to `provider::new_http_pinned` (per plan §Phase 2 line 219).
- **Reality check:** `evm-wallet-core/src/provider.rs:15-38` documents that the ETH-side SPKI pin verifier was REMOVED in commit `36ff115` due to F20 M-2 (signature-verify path returned `Ok` unconditionally). The T6 design must therefore **reserve the type** (`PinnedRpcUrl` newtype with parse + `into_parts() -> (Url, SpkiPin)`) without depending on a production-grade `new_http_pinned` impl. Implementation lands in T8 alongside the verifier composition (provider.rs:29-35 lists the three composition steps required). For T6 the design exposes the parser + `Error::SpkiPin` variant — production wiring is explicitly a T8 ack.

### 2.4 Drift #4 (acknowledged) — `polygon-wallet-core::Signer` is not 1:1 with ETH

- The ETH CLI handler `wallet_send_native` (`eth/src/handlers.rs:633-689`) takes a `&PrivateKeySigner` (alloy-signer-local) and builds the envelope via `eth_wallet_core::sign_native_eth_tx` (`evm-wallet-core/src/signer.rs:99`).
- The polygon CLI does **not** need its own sign helpers — it re-exports `polygon_wallet_core::*` (already includes `sign_native_eth_tx` via `polygon-wallet-core/src/lib.rs:24`) and uses the same alloy signer shape. The 31-story matrix in `docs/wallets/2026-08-27-polygon-wallet-user-stories.md` lines 31-66 shows 28/31 stories are inherited verbatim from ETH; the polygon CLI is a **wrapper that wires the same handlers with Polygon network constants**. This is the deliberate Q1 Option A refactor outcome (plan §Global Constraints line 18).

### 2.5 Drift #5 (TDD pickup gap) — `Error`/`Result` re-export from polygon-wallet-core

- **Surface gap discovered during TDD prep:** `polygon-wallet-core/src/lib.rs` only re-exports `Network`/`PolygonChain`/`EthereumChain`. The CLI design assumes `polygon_wallet_core::Result`, `polygon_wallet_core::Error` are importable (mirroring `eth_wallet_core::*` from the ETH CLI). Today they are not re-exported.
- **Resolution (TDD step 1):** Add `pub use evm_wallet_core::{Error, Result};` to `polygon-wallet-core/src/lib.rs`. One-line lib change. Lands in the same commit as T6 step 1 (Cargo.toml + workspace `members` update). Backward-compatible — additive re-export.
- **Alternative considered:** Add `evm-wallet-core` as direct dep to polygon CLI Cargo.toml. Rejected — breaks Q1 Option A's "single import surface" property (polygon CLI should depend on polygon-wallet-core alone for non-alloy types).

---

## 3. API surface

### 3.1 `Cargo.toml` — workspace member + deps

Workspace deps are declared in `rust-wallet-app/Cargo.toml:22-129` (alloy family, clap, tokio, tracing, etc.). The polygon crate adds:

```toml
# rust-wallet-app/crates/polygon/Cargo.toml
[package]
name = "polygon"
version = "0.1.0"
edition = "2021"
rust-version = "1.94"
license = "MIT"
description = "`polygon` CLI for the polygon-wallet-core v0.1 library (Issue #426 / Phase 4 of #416)."
publish = false

[[bin]]
name = "polygon"
path = "src/main.rs"

[dependencies]
clap                   = { workspace = true }
polygon-wallet-core    = { path = "../polygon-wallet-core" }
tracing                = { workspace = true }
tracing-subscriber     = { workspace = true }
hex                    = { workspace = true }
alloy-primitives       = { workspace = true }
alloy-provider         = { version = "^1.0" }
alloy-network          = { version = "^1.0" }
alloy-transport-http   = { version = "^1.0" }
alloy-rpc-types        = { version = "^1.0" }
alloy-consensus        = { version = "^1.0", features = ["k256"] }
alloy-signer-local     = { version = "^1.0", features = ["mnemonic"] }
uuid                   = { workspace = true }
tokio                  = { workspace = true }
dotenvy                = "0.15"      # mirrors eth/Cargo.toml:41
serde_json             = { workspace = true }
rpassword              = { workspace = true }
zeroize                = { workspace = true }

# Used for the `pinned://` URL parser + `--pin-spki` flag (T8 acceptance;
# T6 reserves the type only).
bitcoin-wallet-core    = { workspace = true }   # for chain::spki::SpkiPin
```

Mirrors `eth/Cargo.toml` line-for-line for the alloy + workspace deps. The `bitcoin-wallet-core` dep is new vs ETH — needed for the SPKI-pin type (T8 deferred, but the parser lands in T6).

### 3.2 Workspace `members` addition (plan §T6 Step 1 line 271)

Add `"crates/polygon",` to the `members = [...]` array at `rust-wallet-app/Cargo.toml:3-13`, alphabetically between `"crates/polygon-wallet-core"` and `"spikes/tron-v1"`.

### 3.3 Module file tree

```text
rust-wallet-app/crates/polygon/
├── Cargo.toml                  (file above)
└── src/
    ├── main.rs                 (Clap parse + dispatch + exit-code mapping + tracing init)
    ├── cli.rs                  (clap derive types: Cli, Command, WalletAction, TxAction, Erc20Action, FeeArgs, ConfigAction, SignMessageArgs, SignTypedArgs, FaucetArgs)
    └── handlers/
        ├── mod.rs              (re-exports + open_manager/open_provider/map_wallet_err + parse_network)
        ├── wallet.rs           (create / import / list / show / delete / balance / sync / send / send_speedup / export)
        ├── tx.rs               (list / get)
        ├── erc20.rs            (send / balance / list / register / approve)
        ├── fee.rs              (fee tier table)
        ├── config.rs           (config show — JSON + text)
        ├── faucet.rs           (print Amoy faucet URL + optional auto-claim marker)
        └── sign.rs             (sign-message EIP-191 + sign-typed EIP-712 with chain_id validation)
```

The 7-file handler split mirrors `btc/src/handlers.rs` (which carries all handlers in one file at 1900 lines); the polygon CLI splits because of the EVM/ERC-20 + EIP-712 surface, which BTC doesn't have. ETH CLI is single-file (`eth/src/handlers.rs`) — that's the outlier.

### 3.4 Clap subcommand tree (full, per plan §T6 Step 2 line 272)

Every subcommand includes the cross-cutting flags (last section): `--json` (output mode, when applicable), `--network amoy|mainnet`, `--rpc-url <URL>` (override default). Per the L13 hard rule, all wallet-network flags have `env = "POLYGON_NETWORK"` / `env = "POLYGON_RPC_URL"` so dotenvy-loaded env flows through clap.

```text
polygon
├── wallet
│   ├── create
│   │     --name <STR>                        # required, 1..=32 [A-Za-z0-9 _-]
│   │     [--password <STR>]                  # insecure (stderr warning); fallback POLYGON_PASSWORD → TTY prompt
│   │     [--network amoy|mainnet]            # default: amoy
│   │     [--derivation-path <PATH>]          # default: m/44'/60'/0'/0/0
│   │     [--account-index <N>]               # default: 0
│   │     [--legacy-token-symbol]             # displays "MATIC" instead of "POL" (Story 31)
│   │     [--rpc-url <URL>]
│   ├── import
│   │     --name <STR>
│   │     [--password <STR>]
│   │     [--network amoy|mainnet]
│   │     (--mnemonic <PHRASE> | --private-key <HEX>)   # exactly one required
│   │     [--account-index <N>]
│   │     [--legacy-token-symbol]
│   │     [--rpc-url <URL>]
│   ├── list
│   │     [--network amoy|mainnet]
│   │     [--all]                             # list across both networks
│   │     [--json]
│   ├── show
│   │     --name <STR> | --id <UUID>
│   │     [--network amoy|mainnet]
│   │     [--addresses | --export]            # Story 19 (xpub + first 5 receive addrs)
│   │     [--json]
│   ├── delete
│   │     --name <STR> | --id <UUID>
│   │     [--network amoy|mainnet]
│   ├── balance
│   │     --address <ADDR>
│   │     [--network amoy|mainnet]
│   │     [--unit pol|wei|matic(deprecated)]  # default: pol; matic warns + aliases pol (Story 31)
│   │     [--legacy-token-symbol]
│   │     [--rpc-url <URL>]
│   ├── sync
│   │     --address <ADDR>
│   │     [--network amoy|mainnet]
│   │     [--rpc-url <URL>]
│   └── send
│         --name <STR>
│         [--password <STR>]
│         --to <ADDR>
│         --amount <DECIMAL>
│         [--network amoy|mainnet]
│         [--unit pol|wei]
│         [--batch <FILE> | --drain]          # Story 13 (batch) / Story 14 (drain)
│         [--nonce <N>]                       # Story 15
│         [--gas-limit <N>]                   # Story 16
│         [--fee fastest|half_hour|hour|economy]            # default: half_hour
│         [--max-fee-gwei <N>] [--priority-fee-gwei <N>]   # EIP-1559 override (Story 6)
│         [--dry-run]
│         [--wait]
│         [--rpc-url <URL>]
│         send speed-up                       # Story 17
│           --tx-hash <HASH>
│           --max-fee-gwei <N> --priority-fee-gwei <N>
│           --name <STR>
│           [--password <STR>]
│           [--network amoy|mainnet]
│           [--rpc-url <URL>]
├── tx
│   ├── list
│   │     --address <ADDR>
│   │     [--network amoy|mainnet]
│   │     [--since-block <N>] [--limit <N>]
│   │     [--json]
│   └── get
│         --tx-hash <HASH>
│         [--network amoy|mainnet]
│         [--json]
│         [--rpc-url <URL>]
├── erc20
│   ├── send                                  # Story 21
│   │     --name <STR>
│   │     [--password <STR>]
│   │     --token USDC|USDT|DAI
│   │     [--token-address <ADDR>]
│   │     --to <ADDR>
│   │     --amount <DECIMAL>
│   │     [--network amoy|mainnet]
│   │     [--gas-limit <N>]
│   │     [--max-fee-gwei <N>] [--priority-fee-gwei <N>]
│   │     [--dry-run]
│   │     [--rpc-url <URL>]
│   ├── balance                               # Story 22
│   │     --address <ADDR>
│   │     --token USDC|USDT|DAI
│   │     [--token-address <ADDR>]
│   │     [--network amoy|mainnet]
│   │     [--all]
│   │     [--decimals <N>]
│   │     [--json]
│   │     [--rpc-url <URL>]
│   ├── list                                  # Story 23
│   │     [--network amoy|mainnet]
│   │     [--json]
│   ├── register                              # Story 24
│   │     --address <ADDR>
│   │     [--network amoy|mainnet]
│   │     [--list | --remove --symbol <SYM>]
│   └── approve                               # Story 25
│         --name <STR>
│         --token USDC|USDT|DAI
│         --spender <ADDR>
│         --amount <DECIMAL>
│         [--amount unlimited|max]
│         [--network amoy|mainnet]
│         [--gas-limit <N>]
│         [--max-fee-gwei <N>] [--priority-fee-gwei <N>]
│         [--dry-run]
│         [--rpc-url <URL>]
├── fee                                      # Story 8
│   [--network amoy|mainnet]
│   [--json]
│   [--rpc-url <URL>]
├── config                                   # Story 11
│   └── show
│     [--json]
├── faucet                                    # Story 30
│   --address <ADDR>
│   [--network amoy]
│   [--faucet-token POL]
│   [--auto]                                  # reserved for T7
├── sign-message                              # Story 18
│   --name <STR>
│   --password <STR>
│   --message <STR>
│   [--address <ADDR>]
│   [--verify <ADDR>]
│   [--rpc-url <URL>]
└── sign-typed                                # Story 27 (EIP-712)
    --name <STR>
    --password <STR>
    (--typed-data <JSON> | --typed-data-file <PATH>)
    --chain-id 137|80002                      # REQUIRED; Q7 + C1 enforcement
    [--verify <ADDR>]
    [--rpc-url <URL>]
```

**Top-level global flags** (on `Cli`, `global = true`):

```text
--rpc-url <URL>          # --rpc-url > POLYGON_RPC_URL env > per-network default
--data-dir <PATH>        # --data-dir > POLYGON_DATA_DIR env > XDG default
--json                   # output mode
--network <NET>          # default for commands that take --network at the action level
--legacy-token-symbol    # global MATIC alias (Story 31)
--pin-spki <HEX>         # T8 reserved; T6 reserves parser only
```

Global flags mirror `eth/src/main.rs:51-74` (`rpc_url`, `data_dir` globals with `env = "..."` + `global = true`).

### 3.5 Cross-cutting (per plan §T6 Step 2 + issue #426 spec)

- **`--json`** on every command that produces output (text-mode default; JSON overrides). Mirrors `eth` wallet balance (lines 184-185) + ERC-20 list (lines 345-347).
- **`std::process::ExitCode`**: `main()` returns `ExitCode` (typed) rather than `std::process::exit(u8)`. This is a stability improvement vs the ETH CLI's `std::process::exit(u8)` (`eth/src/main.rs:399`) and matches plan §"Cross-cutting" line 293. Exit-code table per `evm_wallet_core::Error::exit_code()` (existing in v0.2 — see `eth/src/main.rs:18-22`).
- **No daemons**: no background threads, no long-running loops, no signal handlers. Each invocation is request/response.
- **`zeroize::Zeroizing<Mnemonic>`**: mnemonic from `WalletManager::unlock()` (`evm-wallet-core/src/wallet.rs:534`) already wraps in `Zeroizing<Mnemonic>`; the CLI must not `.clone()` or `.into()` into a plain `String`. Construct alloy's `PrivateKeySigner` at the use site, scoped to the command (mirrors ETH's H-2 fix at `eth/src/main.rs:608-614`).
- **EIP-55 display**: `alloy_primitives::Address::to_checksum_buffer(None)` for every address printed to stdout. Direct-format into `String`/stdout via `format!("{}", addr)` (Display impl is EIP-55 — see `eth/src/handlers.rs:440` rationale comment).
- **`--legacy-token-symbol`** flag enables MATIC alias display (Story 31 + plan §"Q8"). Implementation: pass `use_legacy` to `polygon_wallet_core::disambig::gas_token_label(use_legacy)` (defined at `polygon-wallet-core/src/disambig.rs:53-59`).
- **USDC footgun**: `polygon_wallet_core::disambig::reject_bridged_usdc_e(addr)` (defined at `polygon-wallet-core/src/disambig.rs:75-84`) is called automatically on every `erc20 send` whose `--token USDC` resolves to a bridged address. The CLI emits `Error::InvalidInput("BRIDGED_USDC_REJECTED ...")` with exit code 2.

---

## 4. Threat-model coverage

The interface shape encodes mitigations for these L13 critical-tier threats (from the original brief + plan §Q7 + L12 lessons).

| Threat | Mitigation in interface | Source ref |
|---|---|---|
| EIP-712 cross-chain replay | `sign-typed` requires explicit `--chain-id 137\|80002`. CLI validates flag matches `network.chain_id()` BEFORE calling `sign_typed_data`. Mismatch → `Error::InvalidInput` (exit 2). | plan §Q7 line 24 + §Critical-tier L13 implications in brief |
| Mnemonic leak via logger / `Debug` derive | `Cli` derives `Debug` manually, redacting `password` (mirrors `btc/src/cli.rs:5-6` L12 CRITICAL #2). Mnemonic never read into plain `String`; `WalletManager::unlock` returns `Zeroizing<Mnemonic>` which `Debug`s as `Mnemonic(***REDACTED***)`. | `evm-wallet-core/src/wallet.rs:534` + `btc/src/cli.rs:5-6` |
| Password env var inheritance to subprocesses | `resolve_password()` (CLI-internal, mirrors `eth/src/main.rs:421-429`) reads `POLYGON_PASSWORD` then `std::env::remove_var("POLYGON_PASSWORD")` BEFORE doing anything else. | `eth/src/main.rs:421-429` pattern + L13 brief |
| SPKI downgrade on RPC | `--rpc-url` parser accepts `http(s)://` (default rustls system CAs) OR `pinned://<spki>@<host>` (SPKI-pinned). Production `new_http_pinned` impl **deferred to T8** (per provider.rs:15-38 removal note). Interface **reserves the parser + `SpkiPin` type** so T6 wires the parser and T8 only adds the production verifier. | `evm-wallet-core/src/provider.rs:15-38` + `bitcoin-wallet-core/src/chain/spki.rs:44` |
| USDC.e vs native USDC footgun | Global `--legacy-token-symbol` flag does NOT suppress the USDC.e guard. `erc20 send --token USDC` auto-calls `reject_bridged_usdc_e(token_addr)`; bridged address → `Error::InvalidInput("BRIDGED_USDC_REJECTED ...")`, exit 2. | `polygon-wallet-core/src/disambig.rs:75-84` + plan §"Q8" |
| `/proc/<pid>/environ` inheritance | After read+remove, no `Debug` print, no `Display::fmt` print, no `eprintln!("password = ...")`. | `eth/src/main.rs:421-458` |
| Wrong-chain RPC gas estimation | All `send` handlers MUST chain-id-check the provider against `wallet_network.chain_id()` BEFORE signing (mirrors `eth/src/handlers.rs:651-660`). Returns `Error::InvalidInput` on mismatch, exit 2. | `eth/src/handlers.rs:651-660` (L12 security L-1 + L-4) |
| Pending-tx forgery on `send speed-up` | Reuses the eth `wallet_speedup` cryptographic-recovery pattern (`eth/src/handlers.rs:847-874`). Recovery via `EthereumTxEnvelope::recover_signer`; recovered address must match RPC-reported `from` AND wallet address. | `eth/src/handlers.rs:847-874` |
| Default-RPC log poisoning | `redact_rpc_url(e)` wrapper on every `Error::Rpc` formatter — reuses `evm_wallet_core::redact_rpc_url` (introduced per Issue #356 / H-6). | `eth/src/handlers.rs:595-597` + `evm-wallet-core/src/redact.rs` |
| Mnemonic trace in `wallet create` output | Mnemonic echoed to STDERR once, immediately cleared via `Zeroizing::drop()`; never to STDOUT (which carries the `wallet_id` for scripting). Mirrors btc L28 / F49. | `btc/src/main.rs:7` |
| dotenvy malformed .env hides behind defaults | `main.rs` returns exit 2 on malformed `.env` (not silent-default). Mirrors `eth/src/main.rs:380-387`. | `eth/src/main.rs:380-387` |

---

## 5. Implementation — per-file responsibilities

All signatures + doc comments only. NO bodies.

### 5.1 `polygon/src/main.rs`

```text
mod cli;
mod handlers;

fn main() -> std::process::ExitCode;

/// Resolve wallet password with priority chain:
///   --password argv (stderr warning) → POLYGON_PASSWORD env (read+remove) →
///   TTY prompt → Error::InvalidInput. Matches eth/src/main.rs:421-458.
fn resolve_password(cli_pw: Option<&str>) -> polygon_wallet_core::Result<String>;

/// Resolution kernel: same chain, with TTY prompt injected.
/// Mirrors eth/src/main.rs:439-458.
fn resolve_password_with(
    cli_pw: Option<&str>,
    env_pw: Option<String>,
    prompt_fn: impl FnOnce() -> polygon_wallet_core::Result<String>,
) -> polygon_wallet_core::Result<String>;

/// Cross-platform TTY prompt. Maps io::Error to Error::InvalidInput
/// (never panics on missing /dev/tty). Mirrors eth/src/main.rs:470-477.
fn prompt_password(prompt: &str) -> polygon_wallet_core::Result<String>;

/// Drive dispatch in a tokio current-thread runtime.
fn run(cli: cli::Cli) -> polygon_wallet_core::Result<()>;

#[cfg(test)]
mod password_resolution_tests;  // mirrors eth/src/main.rs:769-889 verbatim
```

### 5.2 `polygon/src/cli.rs`

```text
/// clap value_parser for Address-typed flags. Mirrors eth/src/main.rs:44-46.
fn parse_address(s: &str) -> std::result::Result<Address, String>;

#[derive(Parser, Debug)]
#[command(name = "polygon", version, about = "Polygon PoS wallet CLI (alloy v1.8.x)")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    #[arg(long, global = true, env = "POLYGON_RPC_URL")]
    rpc_url: Option<String>,

    #[arg(long, global = true, env = "POLYGON_DATA_DIR")]
    data_dir: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Wallet { #[command(subcommand)] action: WalletAction },
    Tx { #[command(subcommand)] action: TxAction },
    Erc20 { #[command(subcommand)] action: Erc20Action },
    Fee(FeeArgs),
    Config { #[command(subcommand)] action: ConfigAction },
    Faucet(FaucetArgs),
    SignMessage(SignMessageArgs),
    SignTyped(SignTypedArgs),
    Version,
}

#[derive(Subcommand, Debug)]
enum WalletAction { Create { ... }, Import { ... }, List { ... }, Show { ... },
                     Delete { ... }, Balance { ... }, Sync { ... },
                     Send(SendArgs), SendSpeedup(SendSpeedupArgs) }

#[derive(clap::Args, Debug)]
struct SendArgs { /* 13 fields per §3.4 */ }

#[derive(clap::Args, Debug)]
struct SendSpeedupArgs { /* per §3.4 */ }

#[derive(Subcommand, Debug)]
enum TxAction { List { ... }, Get { ... } }

#[derive(clap::Args, Debug)]
struct FeeArgs { ... }

#[derive(Subcommand, Debug)]
enum ConfigAction { Show { ... } }

#[derive(clap::Args, Debug)]
struct FaucetArgs { ... }

#[derive(clap::Args, Debug)]
struct SignMessageArgs { ... }

#[derive(clap::Args, Debug)]
struct SignTypedArgs {
    /// REQUIRED. Rejects any chain_id not in {137, 80002}.
    #[arg(long)]
    chain_id: u64,

    /// Required: --typed-data JSON literal OR --typed-data-file path.
    #[arg(long, conflicts_with = "typed_data_file")]
    typed_data: Option<String>,
    #[arg(long, conflicts_with = "typed_data")]
    typed_data_file: Option<PathBuf>,

    #[arg(long)]
    name: String,
    #[arg(long)]
    password: Option<String>,
    #[arg(long)]
    address: Option<String>,
    #[arg(long)]
    verify: Option<String>,
    #[arg(long, global = true, env = "POLYGON_RPC_URL")]
    rpc_url: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Erc20Action { Send { ... }, Balance { ... }, List { ... },
                    Register { ... }, Approve { ... } }
```

Notes:
- All `--network` flags carry `env = "POLYGON_NETWORK"` (mirrors `eth/src/main.rs:135` env-attr pattern).
- Wallet name flag carries no env attr; name is identity-bound, not config-bound (same as ETH).
- Sign-typed's `--chain-id` is `u64`, validated against `{137, 80002}` in `main.rs::run` before calling the handler. This is the **type-level enforcement** of Q7.

### 5.3 `polygon/src/handlers/mod.rs`

```text
pub use self::wallet::*;
pub use self::tx::*;
pub use self::erc20::*;
pub use self::fee::*;
pub use self::config::*;
pub use self::faucet::*;
pub use self::sign::*;

/// Open a WalletManager honoring --data-dir. Mirrors eth/src/handlers.rs:43-48.
pub fn open_manager(data_dir: Option<&PathBuf>) -> polygon_wallet_core::Result<WalletManager>;

/// Open an RPC RootProvider<Ethereum> against --rpc-url. Handles
/// http(s):// (default rustls system CAs) and pinned://<spki>@<host>
/// (T8 reserved). Returns Error::InvalidInput on URL parse failure.
pub fn open_provider(rpc_url: &str) -> polygon_wallet_core::Result<RootProvider<Ethereum>>;

/// Map WalletError → canonical Error so the CLI's exit_code() table applies.
/// Mirrors eth/src/handlers.rs:68-88 verbatim.
pub(crate) fn map_wallet_err(e: WalletError) -> Error;

/// Parse --network at the polygon CLI boundary. Default = amoy.
/// Rejects "anvil" with Error::InvalidInput (drift #2 above).
pub fn parse_network(s: &str) -> polygon_wallet_core::Result<Network>;

/// Validate wallet name (1..=32 chars, charset [A-Za-z0-9 _-]).
/// Mirrors eth/src/handlers.rs:95-113.
pub(crate) fn validate_wallet_name(name: &str) -> Result<()>;
```

### 5.4 `polygon/src/handlers/wallet.rs`

```text
pub fn wallet_create(...) -> Result<WalletCreated>;            // Story 1
pub fn wallet_import(...) -> Result<WalletCreated>;            // Story 2
pub fn wallet_list(mgr: &WalletManager, all: bool, json: bool) -> Result<()>;  // Story 9
pub fn wallet_show(mgr, name, id, network, addresses, export, json) -> Result<()>;  // Stories 9, 19
pub fn wallet_delete(mgr, name, id, network) -> Result<()>;    // Story 9
pub async fn wallet_balance(provider, address, unit, network, legacy, rpc) -> Result<()>;  // Stories 3, 31
pub async fn wallet_sync(provider, address, network) -> Result<()>;  // Story 4
pub async fn wallet_send_native(...) -> Result<B256>;          // Stories 5, 6, 13-16
pub async fn wallet_send_speedup(...) -> Result<B256>;         // Story 17
```

All signatures re-use `evm-wallet-core::sign_native_eth_tx` (via `polygon-wallet-core` re-export) for envelope construction, plus the chain_id trust-boundary gate from `eth/src/handlers.rs:651-660`.

### 5.5 `polygon/src/handlers/tx.rs`

```text
pub async fn tx_list(provider, address, since_block, limit, json) -> Result<()>;
pub async fn tx_get(provider, tx_hash, json) -> Result<()>;     // Story 7
```

Mirrors `eth/src/handlers.rs:566-579` (tx_get) and `eth/src/handlers.rs:945-952` (tx_list); tx_list adds the `--address` filter + `--since-block` (no ETH equivalent — polygon needs the address to scope the get_logs scan).

### 5.6 `polygon/src/handlers/erc20.rs`

```text
pub async fn erc20_send(...) -> Result<B256>;                    // Story 21
pub async fn erc20_balance(...) -> Result<()>;                  // Story 22
pub fn erc20_list(network, json) -> Result<()>;                 // Story 23
pub fn erc20_register(address, list, remove, network) -> Result<()>;  // Story 24
pub async fn erc20_approve(...) -> Result<B256>;                 // Story 25

/// Resolve --token USDC|USDT|DAI to (Address, decimals) via
/// polygon-wallet-core::tokens::load_mainnet/load_amoy.
fn resolve_token(symbol: &str, network: Network) -> Result<(Address, u8)>;

/// Reject bridged USDC.e addresses (always-on, not behind --legacy).
fn guard_usdc_e(token: Address) -> Result<()>;
```

`resolve_token` and `guard_usdc_e` are file-private; the public handlers call them automatically (USDC.e guard runs on every `erc20 send --token USDC`).

### 5.7 `polygon/src/handlers/fee.rs`

```text
pub async fn fee(provider: &RootProvider<Ethereum>, network: Network, json: bool) -> Result<()>;
```

Calls `provider.estimate_eip1559_fees()` **per invocation** — no in-process cache (per plan §Q5: 2-second blocks mean cached values go stale in <3s). Maps alloy's estimate to a fee-tier table (fastest/half_hour/hour/economy) by taking the estimate as the half_hour tier and applying multipliers (per `docs/wallets/2026-08-27-polygon-wallet-user-stories.md` Story 5 AC).

### 5.8 `polygon/src/handlers/config.rs`

```text
pub fn config_show(rpc_url: &str, data_dir: Option<&PathBuf>, json: bool) -> Result<()>;
```

Mirrors `eth/src/handlers.rs:983-1030` (config_show) with polygon-specific env names: `POLYGON_NETWORK`, `POLYGON_RPC_URL`, `POLYGON_DATA_DIR`.

### 5.9 `polygon/src/handlers/faucet.rs`

```text
pub fn faucet_print_url(address: Address, network: Network, faucet_token: Option<String>) -> Result<()>;
```

Prints the canonical Amoy faucet URL (`https://faucet.polygon.technology/`) plus the address to drip. `--auto` is reserved for T7 (operator-driven per L29); in T6 it prints a stderr marker `auto-claim: deferred to T7 (L29)` and exits 0.

### 5.10 `polygon/src/handlers/sign.rs`

```text
/// EIP-191 personal_sign. Mirrors evm-wallet-core/src/signer.rs:149-153.
/// Optional --verify round-trips the signature.
pub async fn sign_message(
    provider: &RootProvider<Ethereum>,
    signer: &PrivateKeySigner,
    message: &str,
    verify_address: Option<Address>,
) -> Result<()>;                                                     // Story 18

/// EIP-712 typed-data sign. Chain-id validation enforced BEFORE signing.
pub async fn sign_typed_data(
    provider: &RootProvider<Ethereum>,
    signer: &PrivateKeySigner,
    typed_data_json: &str,
    chain_id: u64,                                                    // must be 137 or 80002
    verify_address: Option<Address>,
) -> Result<()>;                                                     // Story 27

fn assert_polygon_chain_id(chain_id: u64) -> Result<()> {
    // Q7 + C1 enforcement. Returns Err(InvalidInput) for any value
    // not in {137, 80002}.
}
```

`assert_polygon_chain_id` is the single chokepoint for Q7 enforcement; both `sign_typed_data` (explicit arg) and any future EIP-712 path (route handlers, Permit2, etc.) call it before signing.

### 5.11 Tests placement

- Per-file `#[cfg(test)] mod tests` for unit tests (e.g., `handlers::fee::tests::fee_tier_multipliers_apply`).
- `tests/cli_smoke.rs` (workspace-level) for binary integration tests driven via `assert_cmd` or `escargot`. Mirrors the structure of `rust-wallet-app/crates/eth/tests/`.
- `tests/amoy_smoke.rs` lives at the workspace level too (T7 scope; reserved file in T6).

---

## 6. Tests — failing-test-first plan (L13 step 3)

Each batch lists the test module + first failing test name. The first test is the failing seed; subsequent tests land in subsequent TDD cycles.

### 6.1 Batch A — main.rs + cli.rs (parsing + dispatch)

Module: `polygon/src/main.rs::password_resolution_tests` (mirrors `eth/src/main.rs:769-889`).

1. `argv_wins_over_env_and_prompt` (failing seed — copied from `eth/src/main.rs:806`).
2. `env_used_when_no_argv`.
3. `prompt_used_when_no_argv_no_env`.
4. `empty_argv_falls_through_to_env`.
5. `resolve_password_reads_and_removes_polygon_password_env` (L13-brief critical-tier: env-var remove pattern).
6. `parse_network_rejects_anvil_with_invalid_input`.

### 6.2 Batch B — handlers/mod.rs (open_manager, open_provider, parse_network)

Module: `polygon/src/handlers/mod.rs::tests`.

1. `open_provider_accepts_https_url` (failing seed).
2. `open_provider_accepts_pinned_url_scheme_without_invoking_impl` (T8 reserved; T6 just checks parser + dispatches to stub returning `Error::InvalidInput("SPKI pinning deferred to T8")`).
3. `parse_network_amoy_default_returns_polygon_amoy`.
4. `parse_network_mainnet_returns_polygon_mainnet`.
5. `parse_network_anvil_returns_invalid_input` (drift #2 explicit test).
6. `map_wallet_err_crypto_to_decryption_failed`.

### 6.3 Batch C — handlers/wallet.rs

Module: `polygon/src/handlers/wallet.rs::tests`.

1. `wallet_create_writes_encrypted_blob_to_polygon_data_dir` (failing seed).
2. `wallet_create_zeroizing_mnemonic_not_in_stdout` (L13 brief — capture stdout, assert no 12-word BIP-39 substring).
3. `wallet_balance_chain_id_trust_boundary_check` (mock provider returning chain_id=1 for an amoy wallet → `Error::InvalidInput`).
4. `wallet_send_speedup_recovers_signer_and_rejects_rpc_forgery` (mirrors ETH `wallet_speedup` Gate 5 cryptographic recovery at `eth/src/handlers.rs:847-874`).
5. `legacy_token_symbol_displays_matic_not_pol` (Story 31).

### 6.4 Batch D — handlers/sign.rs

Module: `polygon/src/handlers/sign.rs::tests`.

1. `sign_typed_data_rejects_chain_id_1_with_invalid_input` (failing seed — Q7 mitigation; cross-chain replay blocked at the type level).
2. `sign_typed_data_rejects_chain_id_11155111_with_invalid_input`.
3. `sign_typed_data_accepts_chain_id_137_with_valid_signature`.
4. `sign_typed_data_accepts_chain_id_80002_with_valid_signature`.
5. `sign_typed_data_signed_message_does_not_verify_on_other_chain` (spike V10 mirror; chains 137 vs 1).

### 6.5 Batch E — handlers/erc20.rs (USDC.e footgun)

Module: `polygon/src/handlers/erc20.rs::tests`.

1. `erc20_send_usdc_rejects_bridged_usdce_address_with_invalid_input` (failing seed — Story 31 + disambig.rs).
2. `erc20_send_usdc_accepts_native_usdc_address`.
3. `erc20_balance_uses_registry_decimals_without_rpc_call`.
4. `erc20_balance_override_decimals_via_flag`.

### 6.6 Batch F — handlers/fee.rs + handlers/config.rs + handlers/faucet.rs

Module: split per file.

1. `fee_reestimates_per_call_does_not_cache` (failing seed — Q5 cadence).
2. `fee_returns_invalid_input_for_mainnet_fee_query_on_anvil_chain` (chain_id trust boundary).
3. `config_show_json_includes_polygon_chain_id`.
4. `faucet_prints_amoy_url_with_address`.
5. `faucet_auto_flag_defers_to_t7_with_stderr_marker`.

### 6.7 Batch G — binary integration (workspace-level)

File: `polygon/tests/cli_smoke.rs`.

1. `cli_help_exits_zero_with_polygon_subcommands_listed` (failing seed — proves binary builds + clap tree is correct).
2. `cli_version_exits_zero_with_polygon_v0_1_0`.
3. `cli_wallet_create_then_list_round_trip` (uses tempdir + `POLYGON_DATA_DIR`).
4. `cli_unknown_subcommand_exits_2` (clap default; verifies stable exit codes per plan §Cross-cutting line 293).

---

## 7. L12 review pre-flight — likely findings to expect

L12 (lessons-learned L12 in `tasks/lessons.md`) plus the in-flight L13 brief call out review patterns. The implementer should expect:

| Finding category | Likely L12 finding | Mitigation baked into this design |
|---|---|---|
| Type design | "`Cli` derives `Debug` and leaks `password` via `tracing::debug!(?cli)`." | `Cli` (and all password-bearing subcommand args) derive `Debug` manually, redacting `password` to `Some(***REDACTED***)`. Mirrors `btc/src/cli.rs:5-6`. |
| Type design | "`send-typed`'s `--chain-id` is `Option<u64>` so it can be silently omitted." | `--chain-id` is **required** (`u64`, no `Option`). Mismatch with wallet network → `Error::InvalidInput` BEFORE any signing. See `cli.rs::SignTypedArgs::chain_id`. |
| Type design | "`Secret<Mnemonic>` wrapper not used; mnemonic flows through plain `String`." | CLI never owns a `String` mnemonic. `WalletManager::unlock()` returns `Zeroizing<Mnemonic>`; alloy's `MnemonicBuilder` consumes it directly. |
| Type design | "`SpkiPin` exposed as raw `String`; loses the strong type." | `--pin-spki` and the `pinned://<spki>@<host>` parser construct `bitcoin_wallet_core::chain::spki::SpkiPin` (`bitcoin-wallet-core/src/chain/spki.rs:44`) — strong-typed from parse time. |
| Code review | "`unwrap()` on mnemonic parsing or chain-id parse." | All fallible parses return `Result<_, Error::InvalidInput>`; no `unwrap()`/`expect()` outside `#[cfg(test)]`. |
| Code review | "`eprintln!` of `password` or `mnemonic` in any error path." | All loggers redact via `tracing` field redaction (default `RUST_LOG=info` excludes `debug!`); no `eprintln!` of any sensitive material. |
| Code review | "`fetch(&self.network_str)` doesn't validate against the polygon family." | `parse_network` rejects "anvil" (drift #2 above); also rejects "mumbai" / "80001" (forwarded from `PolygonChain::parse_cli` — see `evm-wallet-core/src/network.rs:230-237`). |
| Code review | "`wallet create` mnemonic echo to STDOUT pollutes scriptable output." | Mnemonic echoes to **STDERR only**, cleared on `Zeroizing::drop()` (mirrors `btc/src/main.rs:7` L28/F49). `wallet_id` → STDOUT. |
| Code review | "`erc20 send` doesn't reject bridged USDC.e." | `guard_usdc_e(token)` runs unconditionally in `erc20_send` (auto, not behind `--legacy-token-symbol`). |
| Security audit | "POLYGON_PASSWORD env var persists in `/proc/<pid>/environ` for child processes." | `resolve_password` reads then `std::env::remove_var("POLYGON_PASSWORD")` (mirrors `eth/src/main.rs:425-427`). |
| Security audit | "RPC URL embeds in `Error::Rpc` log poisoning vector." | `redact_rpc_url(e)` wrapper on every `Error::Rpc` formatter (mirrors `eth/src/handlers.rs:595-597`). |
| Security audit | "`sign-typed` envelope uses `wallet_network.chain_id()` from CLI flag, but provider's actual `chain_id` may differ." | Optional pre-sign `provider.get_chain_id()` check (mirrors `eth/src/handlers.rs:651-660`); mismatch → `Error::InvalidInput`. Defense-in-depth on top of the explicit `--chain-id` flag. |
| Test review | "Network-rejection test missing for `parse_network`." | `parse_network_rejects_anvil_with_invalid_input` + `parse_network_rejects_mumbai_with_invalid_input` (both in Batch B). |
| Test review | "EIP-712 chain-id mismatch test missing." | `sign_typed_data_rejects_chain_id_1_with_invalid_input` (Batch D test #1). |

---

## 8. Lessons captured

None yet. T6 hasn't started; this design is the deliverable per L13 step 1 (interface design before code). Per `tasks/lessons.md` L18, lessons are harvested **after** the commit lands.

---

## 9. Backlog (deferred from T6)

Items intentionally NOT in T6 scope. Each lands in a later phase or issue.

| Item | Phase / Issue | Why deferred |
|---|---|---|
| Production `new_http_pinned` verifier | T8 (#426 sibling) | ETH-side verifier was removed (`evm-wallet-core/src/provider.rs:15-38`); must compose with `WebPkiServerVerifier` + `webpki` signature verifier + `x509-parser` for RSA-2048 (provider.rs:29-35). T6 reserves the parser + `SpkiPin` type only. |
| `--auto` claim for `faucet` | T7 (operator-driven per L29) | Live RPC + real faucet claim is L29-deferred to operator session. T6 prints the URL + a stderr marker. |
| `tx list --address` historical scan | T8 | Indexes Polygon's 2-second block history (~43,200 blocks/day). Live Amoy RPC will time out at scale. T6 wires the RPC path; T8 wires the cache/index strategy. |
| MATIC rebrand display in `wallet create` output | T6 (in scope) vs deeper token-list overhaul | T6 implements `--legacy-token-symbol` for the *gas token* display only. Pre-rebrand token symbol lists (legacy USDC.e) remain hidden behind the disambig guard. |
| `wallet sync` block-explorer link | post-v0.1 | Polygon's official block-explorer URL pattern (polygonscan.com) is not exposed in v0.1; deferred to v0.2. |
| Hardware wallet (`--ledger`, `--trezor`) | v0.2 per Q6 | Plan §Q6 line 23: deferred to v0.2 (`alloy-signer-ledger` / `alloy-signer-trezor`). |
| EIP-712 nested struct support | v0.3 per plan §"Out of scope" line 403 | v0.1 ships single-domain EIP-712; nested structs land in v0.3. |
| Polygon zkEVM (`Network::PolygonZkEvm` variant) | v0.2 per plan line 395 | Different chain-id (1101) + RPC + token registry. Add via new enum variant at v0.2. |
| ENS resolution | v1.x per plan line 404 | `alice.eth` → address lookup; not in v0.1. |
| EIP-4337 account abstraction | v1.x per plan line 405 | Smart-account wallets; defer. |
| MEV protection / private RPCs | v1.x per plan line 402 | Flashbots-style Polygon MEV auction; not in scope. |
| WebSocket subscriptions | post-v1.x per plan line 410 | CLI is request/response; WS for streaming is out of v0.1 scope. |

---

## 10. Migration notes — `polygon` vs `eth` CLI

### 10.1 Shared handlers vs duplicated handlers

- **Plan §C2 (per the brief):** not explicitly stated; the original `polygon-wallet-core` plan §Global Constraints line 18 + §Q1 Option A ("Refactor `eth-wallet-core` → `evm-wallet-core` + thin `eth` + `polygon` wrappers. Single source of truth for EVM primitives") implies that handlers SHOULD be shared.
- **Design decision:** handlers are **duplicated** in `polygon/src/handlers/` rather than imported from `eth/src/handlers.rs`. Rationale:
  1. `eth/src/handlers.rs` imports `eth_wallet_core::*` (specific `Network::Ethereum(...)` callsite shape per `eth/src/handlers.rs:130`, line 200, etc.). Forcing polygon to depend on `eth` (which depends on `eth-wallet-core`) would create a circular dependency (`polygon → eth → eth-wallet-core → evm-wallet-core ← polygon-wallet-core`).
  2. The handlers themselves are small (~50-200 lines each) and the network constants they read are at the `Network::Polygon(...)` vs `Network::Ethereum(...)` discriminant — branch points, not deep types. A small amount of duplication is cheaper than a circular dep.
  3. The critical security gates (chain-id check, Zeroizing wrap, env-var remove) are identical to ETH's, and the duplication *preserves* the security shape (no shortcuts taken because "ETH already validated").
- **Concrete plan for refactoring:** post-T9, an L25 follow-up PR can extract `eth_wallet_core::wallet::send_native` + `sign_message` + `sign_typed_data` as a generic function that takes `Network` (the two-level family enum), so both CLIs share the handler. This is **not** a v0.1 requirement — it lands in v0.2 if the duplication becomes painful.

### 10.2 Divergences from `eth` CLI

| Surface | ETH CLI | Polygon CLI | Why |
|---|---|---|---|
| Default network | sepolia | amoy | T6 brief + user-stories Story 1 default |
| `--network anvil` | accepted | rejected | Drift #2 (cross-chain identity footgun) |
| `--network mumbai` | rejected (N/A) | rejected (deprecation 2024-Q2) | Forwarded from `PolygonChain::parse_cli` |
| `--legacy-token-symbol` | absent | present (Story 31) | MATIC rebrand 2024-09-04 |
| `--chain-id` on `sign-typed` | absent | required (Q7) | Cross-chain replay protection |
| `erc20 send --token USDC` | accepts bridged USDC | rejects bridged USDC.e | `disambig.rs` guard |
| `fee` cache | none | none (per plan §Q5) | Both re-fetch per call |
| `--pin-spki` global | absent | reserved (T8) | Plan §Q7 + T8 acceptance |
| Global exit-code path | `std::process::exit(u8)` | `std::process::ExitCode` | Stability improvement |
| Env var prefix | `ETH_*` | `POLYGON_*` | Family-scoped env naming |

### 10.3 Shared subsystems

Both CLIs re-use (via `polygon-wallet-core` re-exports):

- `Network` enum (`evm-wallet-core/src/network.rs:16`) — two-level.
- `PolygonChain::parse_cli` (`evm-wallet-core/src/network.rs:228`).
- `sign_native_eth_tx` (`evm-wallet-core/src/signer.rs:99`).
- `sign_message` (`evm-wallet-core/src/signer.rs:149`) — EIP-191.
- `sign_typed_data` (`evm-wallet-core/src/signer.rs:164`) — currently returns `SignError::Unsupported` (deferred per signer.rs:155-172). T6 must wire a polygon-side sign-typed path that doesn't depend on the deferred fn.
- `WalletManager::unlock` / `unlock_signer` / `create_wallet_for_network` / `import_wallet_for_network` (wallet.rs:534, :600, :197, :281).
- `Error::exit_code()` (table at `evm-wallet-core/src/error.rs` — 0..=5 stable exit codes per #297 M11).
- `redact_rpc_url` (from `evm-wallet-core/src/redact.rs`).
- `dotenvy::dotenv()` at `main()` startup (pattern from `eth/src/main.rs:380-387`).
- `rpassword::prompt_password` for TTY password input.
- `tracing_subscriber::fmt` for log routing (logs → STDERR).

### 10.4 Test migration

The `polygon/tests/amoy_smoke.rs` integration test (T7 scope, file referenced at plan §T7 line 304) and `polygon/tests/mainnet_smoke.rs` (T8 scope, plan §T8 line 318) are placeholders for the operator-driven smoke runs. T6 ships `polygon/tests/cli_smoke.rs` (Batch G in §6.7).
