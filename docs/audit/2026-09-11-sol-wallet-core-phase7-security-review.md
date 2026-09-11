---
title: sol-wallet-core Phase 7 — security review (ship-gate)
tracker: #559 — filed 2026-09-11 via `gh issue create` (base inferred from cwd; labels rust-sol-core + backlog + security + task; milestone sol-wallet-core v0.1)
plan: docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md (Phase 7 starts line 1359)
deep-dive: docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md
companion-audit: docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md (P5-1..P5-5 🟠🟡 plan-level CLI drift + clap compile bugs)
phase6-audit: docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md (P6-3/P6-4 plaintext-key lifecycle carries into CLI handlers)
date: 2026-09-11
status: open
verdict: BLOCK
severity_legend: 🔴 critical · 🟠 high · 🟡 medium · 🔵 low/hardening · ✅ passed (flagged for completeness)
guide: .local/plugins-docs/2026-09-06-mattpocock-vs-superpowers-audit-guide.md
---

# sol-wallet-core Phase 7 — security review (ship-gate)

Pre-implementation review of `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md`
**Phase 7** (lines 1359–1540) — `sol` CLI (22 commands across wallet/address/balance/spl/tx/config) —
issued before any Phase 7 code lands on `sol/phase7-cli`.

**Scope.** Four implementation seams inside Phase 7:

1. `cli.rs` — clap `Cli` struct + `Cluster`/`Commitment` enums + 22-variant `Commands` enum (Task 7.1 Steps 1–4).
2. `handlers/{wallet,address,balance,spl,tx,config}.rs` — 22 command dispatch arms (Task 7.1 Steps 5–10 + Task 7.2 Steps 1–6).
3. `handlers/error.rs::classify` — 0/1/2/3/4/5 exit-code mapping (Task 7.1 Step 11).
4. `main.rs` + global flag plumbing — `data_dir`, `rpc`, `spki_pin`, `cluster`, `commitment`, `priority_fee`, `cu_limit`, `allow_insecure_tls` (Task 7.1 Steps 1 + 18; Task 7.2 Step 16 verify gate).

The companion audit (`2026-09-09-solana-rust-sdks-deep-dive-security-audit.md`) covered
**plan-level** CLI drift and the broken `clap` sample (P5-1 🟠 exit-code inversion,
P5-2 🟡 handler-inventory contradiction, P5-3 🟡 two clap samples won't compile, P5-4 🟡
public-API count mismatch, P5-5 🟡 coverage-audit letter references off-by-one). The
**Phase 6 audit** flagged plaintext-key lifecycle (P6-3 🔴) and zeroize gap (P6-4 🟠) at
the library level — those propagate directly into every Phase 7 handler that calls
`WalletManager::unlock`. None of the 22 findings below are visible from plan-level
inspection alone — they require implementation-level reasoning about argv secret exposure,
Debug-format STDERR leakage, URL/path validation, mainnet-safety UX, and dry-run matrix.

**Overall verdict: BLOCK.** Three 🔴 HIGH findings (P7-1 argv secret exposure, P7-2
plaintext-key lifecycle in send/send-speedup handlers, P7-3 exit-code inversion will ship
in `classify()`) must clear before Phase 7.1 implementation begins. Six 🟠 HIGH/MEDIUM,
nine 🟡 MEDIUM, and three 🔵 LOW findings listed in priority order below; one ✅ passed
confirm for completeness.

---

## Drift scan (L13 step 4a)

Phase 7 not yet implemented on the live tree — no drift to surface. Plan claims checked against
current state on 2026-09-11:

| Plan claim                                                    | Source     | Verified                                 |
| ------------------------------------------------------------- | ---------- | ---------------------------------------- |
| `crates/sol/` CLI binary skeleton exists (Cargo.toml stub)   | plan L1365 | ✅ `rust-wallet-app/crates/sol/Cargo.toml` present (no `src/` yet) |
| `sol-wallet-core` already depends on Anza SDK + RPC + send_with_retry + wait_for_confirm | plan L1373 | ✅ Phase 5 PR #556 landed commit `2934d040` |
| `WalletManager` + Argon2id + AES-GCM persistence exist        | plan L1373 | ✅ Phase 6 PR #558 landed commit `56a5bfe3` |
| Phase 6 audit findings P6-3/P6-4 (plaintext-key lifecycle) NOT YET RESOLVED in library | Phase 6 audit | 🔴 open — Phase 7 inherits F47 leak |
| Deep-dive `### Solana CLI` architecture has P5-1 (exit-code inversion), P5-2 (handler inventory), P5-3 (clap sample won't compile) | companion audit | 🟠🟡 open — Phase 7 Step 1 + Step 11 reference broken samples verbatim |
| `Cluster` enum has NO `Testnet` variant                        | plan L1386, deep-dive L2662 | ✅ confirmed (testnet DEPRECATED 2022-23) |
| `--cluster devnet` env gate `RUN_SOL_DEVNET=1`                | plan L1402, L1444 | ✅ explicit loud-RED gate in plan |
| Phase 9.1 owns mainnet smoke (`tests/mainnet_smoke.rs` + `cli_mainnet_smoke.rs`) | plan L1455 | ✅ row 22 explicitly deferred per Q4 Q-gate |

No drift. Phase 7 plan is internally consistent with the locked Phase 5 + Phase 6 state
but inherits **3 unresolved Phase 5 audit findings** (P5-1, P5-3, plus implicit Phase 6
carryover of P6-3/P6-4) that will land in `crates/sol/src/` if executed as written.

---

## Findings (ranked most-severe first)

### 🔴 P7-1 — `--mnemonic` argv-exposure L12 H-1 still ships [HIGH]

Task 7.1 Step 6: `wallet import` accepts `--mnemonic` **as a clap arg**, alongside
`--mnemonic-file <mode-0600>`. Plan text L1390:
> "Implement `wallet import` — `--mnemonic` / `--mnemonic-file <mode-0600>` /
> `--private-key-file` (close argv-exposure L12 H-1)"

Mnemonic on the command line is visible in `ps aux`, `ps -ef`, `/proc/<pid>/cmdline`,
shell history (`~/.bash_history`, `~/.zsh_history`), CI logs, and any process accounting
syscall (`lastcomm`, `auditd`). The L12 H-1 fix is the **alternative** `--mnemonic-file`;
keeping `--mnemonic` as a peer defeats the mitigation. Also: `address new` (Task 7.2 Step 1)
likely accepts `--mnemonic` for derivation — same exposure.

**Fix.** Two-step mitigation, all 4 sites (`wallet import`, `address new`, plus any
`wallet send --mnemonic` / `wallet create --mnemonic` future paths):
1. Remove `--mnemonic` from `clap` args; require `--mnemonic-file` (or `--private-key-file`).
   `SecretSeed` wrapper type with custom clap `value_parser` that rejects `String` inputs
   at compile time so a future contributor cannot re-add `--mnemonic` without
   `#[forbid(unsafe_code)]`-style lint trip.
2. **OR** — keep `--mnemonic` but warn loudly to STDERR (red) on first use + emit
   `Warning: mnemonic in argv visible in process list. Use --mnemonic-file for safety.`
   and require `--i-understand-argv-exposure` env var as second factor. Less robust.

Test in `crates/sol/tests/cli_wallet.rs`: `wallet import --mnemonic "word1 word2 ..."` →
assert either hard-reject (option 1) or warning + success (option 2). Run `ps aux` in
the test harness during invocation, assert mnemonic string NOT present in `cmdline`
bytes — this is the actual user-visible attack surface.

---

### 🔴 P7-2 — Plaintext `Keypair` held across RPC in `wallet send` + `send-speedup` handlers [HIGH]

Task 7.1 Steps 9 + 10. Phase 6 audit **P6-3** flagged that `WalletManager::unlock` returns
`solana_sdk::Keypair` (no `Zeroize`) and `lock()` RAII not specified. Phase 7 ships the
handler layer that **holds the unlocked keypair for the full RPC round-trip**:

- `wallet send` (Step 9) — keypair held from `unlock` → sign → `submit_sol`/`submit_spl` →
  optional `wait` + `wait_finalized` (1–12 slot polling).
- `wallet send-speedup` (Step 10) — keypair held from `unlock` → sign → `submit_send_speedup`
  → optional confirm. **Two RPCs** = longer lifetime.
- `wallet balance` (Step 8) — held briefly but still heap-allocated.

`solana_sdk::Keypair` is 64 bytes (32-byte secret seed + 32-byte public key). It does NOT
implement `Zeroize` (Anza gap F47). Plaintext bytes survive `lock()`, survive scope exit
on the heap (until natural drop), survive panic unwind (no `Drop` guard). CI test logs
that capture heap snapshots (e.g. via `valgrind --tool=memcheck`) would surface key bytes.
Memory-dump attacker on a co-tenant host or a core-dump post-mortem exfiltrates the key.

**Fix.** Carry P6-3's library-side fix into the CLI layer:
```rust
// handlers/wallet.rs
let zk = wallet_manager.unlock(id, &password)?;  // returns Zeroizing<Keypair>
let sk_bytes: Zeroizing<[u8; 64]> = Zeroizing::new(zk.to_bytes());
let keypair = Keypair::try_from(&sk_bytes[..])?;  // reconstruct local copy
// pass `&keypair` (NOT owned) to tx::submit_sol — inner fn drops on return
// sk_bytes RAII-zeroizes on handler return
```
Plus `#![warn(clippy::large_types_passed_by_value)]` in `crates/sol/src/` to discourage
`Keypair` by-value. Add CI grep check: `grep -rn "Keypair" crates/sol/src/handlers/`
audited for any `unwrap()` / `expect()` on `Keypair` paths.

Test in `crates/sol/tests/cli_wallet.rs`: run `wallet send` against surfpool; immediately
after exit, parse heap via `memmap2` of `/proc/self/maps` for prior PID (after fork-double);
assert zeroized key bytes NOT present. Alternative without `/proc`: assert handler scope
end returns within tight stack-frames + lock-on-drop property via custom `OwnedLock`
wrapper.

---

### 🔴 P7-3 — Exit-code table inversion will ship in `classify()` [HIGH]

Task 7.1 Step 11 (plan L1395):
> "Implement `error::classify(err) -> exit_code` per deep-dive `### Exit code mapping`
> (0/1/2/3/4/5) + full 21-variant error table per deep-dive §J line 1825"

The companion audit (`P5-1` 🟠) found that deep-dive §J table (L1825) and §L `classify()`
code body (L2664–2690) disagree:
- **Spec table (§J L1825):** exit **4** = signing (`SignFailed`); exit **5** = persistence/config
  (`WalletNotFound`, `WalletDecryptFailed`, `FileIo`, `ConfigInvalid`, `PalError`).
- **Code body (§L L2738–2744):** exit **4** = `WalletNotFound | WalletDecryptFailed`;
  exit **5** = `SignFailed | FileIo | ConfigInvalid | PalError`.

Phase 7 Step 11 instructs the implementer to "use the 21-variant error table per
deep-dive §J line 1825" — i.e. the **inverted** table. If executed literally,
`handlers/error.rs::classify()` will return the wrong exit codes for `SignFailed` and
the wallet/config variants. Downstream shell scripts that key on exit codes will mishandle
errors:
- `wallet send` after key corruption → emits exit 5 instead of 4 → monitoring flags it as
  "internal" not "wallet" → on-call pager misroutes.
- `wallet import` with wrong passphrase → emits exit 5 instead of 4 → same misroute.

The **code body** in §L (and the matching output-conventions list at L2843) is the
correct arrangement — these two agree. The §J table is the bug.

**Fix.** Update Step 11 to specify "the table at §L L2738–2744 + output-conventions list
at §L L2843, NOT §J L1825" (two-against-one wins). Add explicit row mapping in plan:
```
Error::InvalidMnemonic | InvalidAddress | InvalidDerivationPath
  | InvalidCluster | InvalidTokenMint | InvalidTokenProgram => exit 2
Error::BroadcastFailed | ConfirmTimeout => exit 3
Error::WalletNotFound | WalletDecryptFailed => exit 4
Error::SignFailed | FileIo | ConfigInvalid | PalError => exit 5
```
Test `crates/sol/tests/cli_integration_surfpool.rs` row 9 of the 22-row matrix:
force `SignFailed` via tampered keypair → assert exit code 4; force `WalletNotFound` →
assert exit code 5. Both tests would FAIL on the inverted table.

---

### 🟠 P7-4 — Phase 7 Step 1 copies broken clap sample (P5-3) → build fails [HIGH]

Task 7.1 Step 1 (plan L1385):
> "Implement `Cli` struct with global flags (data_dir, rpc, spki_pin, cluster,
> commitment, priority_fee, cu_limit, allow_insecure_tls) per deep-dive `### Solana CLI`
> architecture"

The deep-dive architecture (L2765) contains a clap syntax error:
```rust
#[arg(long, conflicts_with = "wait", conflicts_with_all = [wait])]
pub wait_finalized: bool,
```
`conflicts_with_all` takes `&[&str]`, not `[ident]`; `[wait]` parses as a bare ident array
which clap rejects. Also: the constraint is duplicated (`conflicts_with = "wait"` AND
`conflicts_with_all = [wait]`). This sample will not compile.

Second break (L2631–2634):
```rust
let cfg = SolanaConfig::load(&data_dir)?.with_overrides(&cli.into());  // moves cli
match cli.command { ... }                                            // use-after-move
```
`cli.into()` consumes `cli`; the next line `match cli.command` fails to compile.

If Phase 7.1 implements the `Cli` struct verbatim from deep-dive L2619–2656, `cargo build
-p sol --tests` fails before any test runs. CI loud-RED.

**Fix.** Correct the clap signature:
```rust
#[arg(long, conflicts_with = "wait")]
pub wait_finalized: bool,
```
And the dispatch:
```rust
let overrides = SolanaConfig::from_cli(&cli);  // borrow, don't move
let cfg = SolanaConfig::load(&data_dir)?.with_overrides(&overrides)?;
match cli.command { ... }  // cli still owned
```
Test in `crates/sol/tests/cli_config.rs`: `--wait --wait-finalized` simultaneously →
assert `Error::InvalidArgument` + exit 2 (not a clap panic).

---

### 🟠 P7-5 — `eprintln!("{e:?}")` prints anyhow Debug chain → secret leak in STDERR [HIGH]

Task 7.1 main.rs (deep-dive L2709–2712):
```rust
if let Err(e) = dispatch_result {
    let exit_code = handlers::error::classify(&e);
    eprintln!("{e:?}");  // Debug of full anyhow chain
    std::process::exit(exit_code);
}
```

Anyhow `e.chain()` walks the wrapped error sources. If any handler returns
`anyhow::Error::msg(format!("keypair: {:?}", sk))` (likely in debug builds via
panic-on-unwrap), the `Keypair` Debug impl prints **base58 of the 64-byte secret** —
the full secret seed in STDERR. STDERR is captured by CI logs, systemd journal,
`docker logs`, terminal scrollback. Once logged, secret exfiltration is trivial.

Even without explicit `{:?}` on the Keypair, the F47 zeroize gap (P6-3) means a
panic message that includes key-derived state (e.g. derived child pubkey from a
compromised seed) leaks through the chain.

**Fix.** Two layers:
1. In `handlers/error.rs`, define a wrapper `pub struct Redact<T>(pub T)` with `impl<T:
   fmt::Debug> fmt::Debug for Redact<T>` that prints `"<redacted>"`. Wrap every secret
   before passing into `anyhow::Context::context()`. Mandatory via `#[must_use]` lint.
2. Add a Phase 8 panic-scrubber equivalent in main.rs: regex-match STDERR for base58
   64-byte / 32-byte hex / `xprv...` prefix patterns BEFORE exit; if matched, replace
   with `<redacted>` and emit exit 99 (matches Phase 8 plan line 1546 panic_scrubber.rs).

Test in `crates/sol/tests/cli_wallet.rs` row 9: deliberately trigger a keypair-leaking
error (e.g. invalid signature path that wraps key bytes) → assert STDERR captured by
`assert_cmd` does NOT contain raw base58 64-byte secret.

---

### 🟠 P7-6 — `wallet delete` has no confirmation gate — accidental irreversible loss [HIGH]

Task 7.1 Step 7 (plan L1391): `wallet delete` — invokes `WalletManager::delete(id)`. No
mention of `--yes` flag or interactive confirmation. The encrypted blob is removed
unconditionally. Mis-typed UUID (e.g. copy-paste truncation) destroys wallet.

Similarly `wallet import --private-key-file` has no preview: user does not see the
derived address before import — if the private-key-file is wrong (corrupted, attacker-
substituted), the user has imported an attacker-controlled key and given it a friendly
name. P6-6 also applies here: source file mode must be checked.

**Fix.**
1. `wallet delete --id <uuid>`: require `--yes` flag. Without `--yes`: print summary
   (id, name, address, cluster, last-modified) + prompt `"Type 'delete <uuid>' to
   confirm:"` + read from STDIN. Non-interactive (`!isatty(stdin)`): hard-fail with
   exit 2 + error message naming `--yes`.
2. `wallet import --private-key-file`: after deriving address, print
   `"Will import key with derived address: <base58>. Continue? [y/N]"` + read STDIN.
   With `--yes`: skip prompt.

Test `crates/sol/tests/cli_wallet.rs` row 9: `wallet delete --id <uuid>` without
`--yes` in non-tty → assert exit 2 + encrypted blob still present on disk after the
command.

---

### 🟠 P7-7 — `--confirm-mainnet` semantics undefined for non-interactive contexts [MEDIUM]

Task 7.1 Step 9: `wallet send --confirm-mainnet` default true for mainnet-beta. Plan
text describes this as "extra prompt" but provides no mechanics. Three execution
modes are possible:
1. Interactive: read STDIN for `y/N` confirmation.
2. Non-interactive (`!isatty(stdin)`): hard-fail exit 2 with "use --confirm-mainnet yes"
   env or explicit `--confirm-mainnet yes` flag.
3. Always-pass: flag is a no-op — silently lets through. **DO NOT ship this.**

Current plan text is ambiguous and likely defaults to option 3 (silently no-op), which
defeats the entire purpose of the safety flag. A scripted CI pipeline that forgets
`--confirm-mainnet yes` would burn real SOL.

**Fix.** Two-tier, default-deny:
```rust
if cfg.cluster == Cluster::MainnetBeta && !self.confirm_mainnet {
    return Err(anyhow!("mainnet send requires --confirm-mainnet yes (or env SOL_CONFIRM_MAINNET=yes)"));
}
```
Plus `--confirm-mainnet yes` (parses "yes" string) — bare `--confirm-mainnet` (no arg)
becomes an error. Audit log to STDERR: `"WARN: mainnet send confirmed at <UTC-ISO8601>
by operator=<$USER> amount=<lamports> to=<addr> sig=<pending>"`.

Test in `crates/sol/tests/cli_mainnet_smoke.rs` (Phase 9.1 owns this file): against
surfpool configured as mainnet cluster, omit `--confirm-mainnet` → assert exit 2.

---

### 🟠 P7-8 — `config set-rpc <url>` accepts arbitrary URL — no scheme/host validation [MEDIUM]

Task 7.2 Step 6 (plan L1436): `config set-rpc <url>` persists URL to `config.json`.
Plan text and deep-dive L2949 provide no validation. Risks:

1. `http://` (cleartext) — RPC traffic sniffable on the wire; auth tokens + tx sigs leak.
2. URL with `userinfo` (`https://user:pass@evil.com/rpc`) — credentials persist in
   `config.json` in plaintext, exfiltrated by any process with read on the wallet dir.
3. `file://` / `javascript:` / `data:` — clap `value_parser` doesn't enforce scheme.
4. IP literals + RFC-1918 (e.g. `http://192.168.1.1:8899`) — fine for localnet but
   unclear if intentional on devnet/mainnet.

**Fix.** In `handlers/config.rs::set_rpc`:
```rust
let url = url::Url::parse(&raw_url)?;
if url.scheme() != "https" && !cli.allow_insecure_tls {
    return Err(Error::ConfigInvalid { reason: "rpc URL must be https (or pass --allow-insecure-tls for http)".into() });
}
if !url.username().is_empty() || url.password().is_some() {
    return Err(Error::ConfigInvalid { reason: "rpc URL must not contain userinfo".into() });
}
match url.host_str() {
    Some(h) if h == "localhost" || h == "127.0.0.1" || h == "::1" => {}  // ok for localnet
    Some(_) => {}
    None => return Err(Error::ConfigInvalid { reason: "rpc URL must have host".into() }),
}
```
Test `crates/sol/tests/cli_config.rs`: `set-rpc http://api.devnet.solana.com` → exit 2;
`set-rpc https://user:pass@evil.com` → exit 2; `set-rpc https://api.devnet.solana.com`
→ exit 0.

---

### 🟠 P7-9 — `requestAirdrop` not cluster-gated pre-RPC [MEDIUM]

Task 5.2 implementation (Phase 5.2 PR #556) added `requestAirdrop` (deep-dive §5.2).
Phase 7 Step 8 (`wallet balance --wallet-id`) and any future `wallet airdrop` (Phase 7.1
does not list one but `wallet` 9-subcommand list at L1531 + L2169 may grow) call into
`requestAirdrop`. `requestAirdrop` on mainnet returns RPC error (`airdrop request
rejected`) after a network round-trip — wasted RTT + log noise + false alarm on
monitoring.

More serious: if `wallet airdrop` (any future V0.1.5 command per `faucet.rs` at L2605)
ever ships without cluster check, calling it on mainnet says "Airdrop not available
on mainnet" — confusing UX and a missed exit-2 opportunity.

**Fix.** Cluster guard in the handler (not in the library — library stays neutral for
FFI callers that may have legitimate non-airdrop paths):
```rust
// handlers/wallet.rs (any airdrop-touching command)
if cfg.cluster == Cluster::MainnetBeta && command_touches_airdrop {
    return Err(anyhow!("airdrop unavailable on mainnet-beta; use --cluster devnet|localnet"));
}
```
Test in `crates/sol/tests/cli_config.rs`: `--cluster mainnet-beta wallet airdrop 5 <addr>`
→ exit 2 + no RPC request emitted (assert via mock RPC counter).

---

### 🟠 P7-10 — `wallet rename --to <name>` no name validation → path traversal / unsafe names [MEDIUM]

Task 7.1 Step 7 (plan L1391). Wallet names are stored on disk as part of `data_dir/<cluster>/<wallet_id>/<name>.json`
(inferred from `WalletManager::rename` semantics — deep-dive L2435). If `--to` accepts
arbitrary strings:

1. Path traversal: `--to "../../../etc/passwd"` → rename writes outside `data_dir` →
   attacker-controlled write location if wallet dir is shared (multi-user host,
   container volume mount).
2. Filesystem-unsafe chars: `NUL`, `/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`, control
   chars (0x00–0x1F) — most filesystems reject; Windows reserved names (`CON`, `PRN`,
   `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`).
3. Empty string + whitespace-only — degenerate.

**Fix.** In `WalletManager::rename(id, name)` (library-side; called by CLI handler):
```rust
const NAME_MAX_LEN: usize = 64;
if name.is_empty() || name.len() > NAME_MAX_LEN {
    return Err(Error::ConfigInvalid { reason: format!("wallet name must be 1..={} chars", NAME_MAX_LEN) });
}
if name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0')) {
    return Err(Error::ConfigInvalid { reason: "wallet name contains forbidden char".into() });
}
if matches!(name.to_uppercase().as_str(), "CON" | "PRN" | "AUX" | "NUL" | "COM1".."COM9" | "LPT1".."LPT9") {
    return Err(Error::ConfigInvalid { reason: "wallet name is reserved".into() });
}
```
Test `crates/sol/tests/cli_wallet.rs` row 9: `wallet rename --to "../../etc/passwd"` →
exit 2; `--to ""` → exit 2; `--to $'foo\nbar'` → exit 2; `--to "valid-name_1"` → exit 0.

---

### 🟡 P7-11 — `tx wait --timeout` no upper bound → DoS via long-lived CLI process [MEDIUM]

Task 7.2 Step 5 (plan L1435): `tx wait --sig --timeout --poll-interval` calls
`tx::wait_for_confirm`. Plan text does not bound `--timeout`. A user (or attacker
with control of argv in a CI step) can pass `--timeout 999999999` and the CLI
process lives for years, holding wallet dir lock, polling RPC, accumulating log
volume.

**Fix.** Clamp in clap:
```rust
#[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=600))]
pub timeout: u64,
```
Max 10 minutes — covers the 12-slot `finalized` path with margin. Test `cli_tx.rs` row 16:
`--timeout 999999999` → exit 2 + error msg naming `--timeout` range.

---

### 🟡 P7-12 — `tracing_subscriber` at INFO may log signing params / RPC request bodies [MEDIUM]

Task 7.1 main.rs (deep-dive L2680–2684):
```rust
tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info")))
    .with_writer(std::io::stderr)
    .init();
```

If any handler emits `tracing::info!(?keypair)` (plausible for debug visibility) or
`tracing::info!(rpc_request = ?body)` (plausible for surfpool debugging), secret bytes
hit STDERR. The Phase 8 panic-scrubber catches `panic!` messages but not `tracing`
emissions — those flow through a separate path.

**Fix.**
1. Add `tracing::instrument` skip on every function parameter that holds a secret:
   `#[tracing::instrument(skip(mnemonic, keypair), fields(wallet_id = %id))]`.
2. Add CI grep audit: `grep -rn "tracing::info!\|tracing::debug!" crates/sol/src/
   | grep -iE "keypair|mnemonic|secret|seed|pk"` should return ZERO matches.
3. Document in `crates/sol/src/main.rs` top comment: "Tracing emissions MUST NOT
   include secret material. Use `skip(...)` on instrumented fns + never `{:?}` format
   on key types."

Test `crates/sol/tests/cli_wallet.rs`: run `wallet send` with `SOL_LOG=trace` →
assert captured STDERR does NOT contain base58 64-char substring matching a known
private-key pattern.

---

### 🟡 P7-13 — `--dry-run` vs `--sign-only` semantics conflated [MEDIUM]

Task 7.1 Step 9 (plan L1393): `wallet send --dry-run --sign-only --wait ...`. The
deep-dive handler (L2892–2918) treats them as orthogonal flags but does not enforce
mutual exclusion:

- `--dry-run` = `simulate_transaction` (RPC call, no sig emitted, no balance change).
- `--sign-only` = sign + return base64 (no broadcast).
- Both = sign-then-simulate? Or simulate without sign? Ambiguous.

If `--dry-run` AND `--sign-only` both present without mutual exclusion, the handler
behaves unexpectedly depending on order in the if/else. Plan does not specify.

**Fix.** Add `#[arg(long, conflicts_with = "sign_only", conflicts_with = "dry_run")]`
on both flags (via separate constraint), OR explicit `match` precedence:
```rust
match (sign_only, dry_run) {
    (true, false) => sign_only_sol(...).await?,
    (false, true) => simulate_sol(...).await?,
    (true, true) => return Err(anyhow!("--dry-run and --sign-only are mutually exclusive")),
    (false, false) => submit_sol(...).await?,
}
```
Test `crates/sol/tests/cli_tx.rs` row 13: `--dry-run --sign-only` together → exit 2 +
no sig emitted + no simulation result. `cli_tx.rs` row 14: `--sign-only` alone → exit 0
+ sig present in STDOUT JSON + no broadcast (assert via mock RPC no `send_transaction`
call).

---

### 🟡 P7-14 — `spl send --skip-memo-required` is unconditional opt-out [MEDIUM]

Task 7.1 Step 9 + Task 7.2 Step 3 reference `--skip-memo-required` (deep-dive L2814).
Token-2022 mints can enforce memo on every transfer; skipping this check causes the
SPL transfer to fail RPC-side with a less-actionable error and may violate the mint's
policy. The flag is currently opt-out (default: enforce) but the enforcement is silent
in the UX — a user who doesn't know Token-2022 memo exists will pass `--skip-memo-required`
to make the error go away, losing the protection permanently for that mint.

**Fix.** Require an explicit `--i-understand-no-memo-enforcement` second flag when
`--skip-memo-required` is set:
```rust
if skip_memo_required && !i_understand_no_memo_enforcement {
    return Err(anyhow!("--skip-memo-required requires --i-understand-no-memo-enforcement"));
}
```
Audit log: `WARN: Token-2022 memo enforcement bypassed for mint=<addr> at <UTC-ISO8601>`.

Test `crates/sol/tests/cli_spl.rs`: Token-2022 mint with memo extension → `--skip-memo-required`
alone → exit 2; both flags → exit 0 + warn STDERR.

---

### 🟡 P7-15 — Coverage gate misses all 22 handler paths [MEDIUM]

Phase 6.2 audit extended coverage gate to Phase 6 files (`crypto/`, `persist.rs`,
`wallet_manager.rs`, `platform/`). Phase 7 adds `crates/sol/src/{main.rs, cli.rs,
handlers/{wallet,address,balance,spl,tx,config,error}.rs}` — **none in the gate**. The
`classify()` function alone has 5 arms; 4 of them (`Invalid*`, `BroadcastFailed/ConfirmTimeout`,
`WalletNotFound/DecryptFailed`, `SignFailed/FileIo/ConfigInvalid/PalError`) need explicit
tests for each exit code path.

P7-3 (exit-code inversion) would have been caught at PR time with proper coverage.

**Fix.** Extend gate:
```toml
# .cargo-tarpaulin.toml or CLI flags
sol::main
sol::cli
sol::handlers::wallet
sol::handlers::address
sol::handlers::balance
sol::handlers::spl
sol::handlers::tx
sol::handlers::config
sol::handlers::error
```
Plus `cli_wallet.rs` / `cli_spl.rs` / etc. integration tests count toward handler
coverage when they exercise error paths. Loud-RED if any module drops below 95% line
coverage (lower threshold than 100% because some main.rs branches are env-conditional).

---

### 🟡 P7-16 — `address new` accepts `--mnemonic` (L1431) — inline secret for derivation [MEDIUM]

Task 7.2 Step 1 (plan L1431): `address new --mnemonic [--mnemonic-file] --account <N>
--address-index <N>`. Same argv-exposure as P7-1. Additionally: `address new` is
arguably a pure derivation operation — it could read the mnemonic from a wallet_id
(`WalletManager::unlock(id)` then derive) without accepting the raw mnemonic at all.

The current shape forces users to type/paste their full 12/24-word mnemonic every
time they want a non-default address index — high friction + high secret exposure.

**Fix.** Default to `--wallet-id <uuid> --password <pw>`; accept `--mnemonic-file` as
alternative; REJECT `--mnemonic` (no inline) per P7-1 fix. New shape:
```text
sol address new --wallet-id <uuid> --account <N> --address-index <N>     # preferred
sol address new --mnemonic-file <path> --account <N> --address-index <N> # cold
```
Test `crates/sol/tests/cli_address.rs`: `--wallet-id` path returns pubkey + no
secret-arg leak in `ps`.

---

### 🟡 P7-17 — `address pubkey --wallet-id` triggers full `unlock()` when only pubkey needed [MEDIUM]

Task 7.2 Step 1: `address pubkey --wallet-id`. Ed25519 HD derivation (SLIP-0010) IS
deterministic and DOES require the seed to derive a child pubkey — but the parent
pubkey (account 0, address-index 0) can be obtained from the seed via
`Keypair::from_seed(seed).pubkey()` without exposing the full keypair to the caller.

Plan Step 1 doesn't specify whether the handler calls `WalletManager::unlock` (returns
Keypair) or a `WalletManager::derive_pubkey(wallet_id, password, account, address_index)`
that returns only `Pubkey`. The former leaks the entire secret to handler scope.

**Fix.** Library-side: add `WalletManager::derive_pubkey(&self, id, password, account,
address_index) -> Result<Pubkey, Error>` that:
1. Reads encrypted blob from disk.
2. Derives key + child pubkey inside a scoped block.
3. Calls `Zeroizing::new(...)` for the intermediate keypair; drops at block exit.
4. Returns only `Pubkey`.

CLI handler calls `derive_pubkey` (not `unlock`). Test `crates/sol/tests/cli_address.rs`:
assert Pubkey returned correctly + heap snapshot (per P7-2) does NOT contain keypair bytes.

---

### 🟡 P7-18 — P5-4 / P5-5 deep-dive drift will propagate to handler docs [MEDIUM]

P5-4 🟡: public-API count stated three ways (~140 / ~152 / ~190) — actual sum 179.
P5-5 🟡: coverage-audit letter references off-by-one. These are doc-quality, not
exploitable, but Phase 7 generates handler docstrings referencing `tests/R*` row
numbers and per-command API counts. If the deep-dive is the source of truth, the
handler docs will inherit the wrong numbers.

**Fix.** When implementing handler docstrings, reference the **resolved** values
from the Phase 5 / Phase 6 audits (P5-4 resolved: 179; P5-5 resolved: see companion
audit row table). Do NOT copy-paste from deep-dive §Coverage / §Library surface
sections. Add `// doc-source: companion-audit` comment at the top of `cli.rs` to
discourage future contributors from reverting.

---

### 🟡 P7-19 — `wallet import --private-key-file` ignores P6-6 source-file mode check [MEDIUM]

Phase 6 audit P6-6 🟠 flagged `WalletManager::import_from_pk_file` must refuse source
file mode > 0o600 on Unix (Windows ACL deferred). Phase 7 Step 6 ships this CLI flag
but does NOT specify whether the CLI handler validates source mode BEFORE calling
`import_from_pk_file`. If the CLI relies on the library to refuse: ✅ — covered by P6-6.
If the CLI does its own check: also fine. If the CLI bypasses the library check
(e.g. reads PK + constructs `Keypair` directly + bypasses import): **P6-6 fix bypassed**.

**Fix.** Add explicit note to plan Step 6 + test in `cli_wallet.rs` row 9: pre-create
file mode 0644 → `wallet import --private-key-file <path>` → assert exit 2 +
`Error::InsecureSourceFile` (or equivalent) — NOT a successful import. Reference P6-6
checklist explicitly.

---

### 🟡 P7-20 — `config set-cluster mainnet-beta` no confirmation [MEDIUM]

Task 7.2 Step 6 (plan L1436): `config set-cluster mainnet-beta|devnet|localnet`.
Switching to mainnet-beta silently routes ALL subsequent `wallet send` / `balance` /
etc. to real SOL. A typo in cluster name (or attacker shell access) → wallet now talks
to mainnet, even if user intended devnet.

**Fix.** Add confirmation gate ONLY when transitioning FROM a non-mainnet cluster TO
mainnet-beta:
```rust
if target == Cluster::MainnetBeta && current != Cluster::MainnetBeta {
    eprintln!("WARN: switching cluster to mainnet-beta. Subsequent operations will use real SOL.");
    eprintln!("Set SOL_CONFIRM_MAINNET=yes to skip this prompt in scripts.");
    if !confirm_interactive()? {
        return Err(anyhow!("cluster switch aborted"));
    }
}
```
Test `cli_config.rs`: `set-cluster devnet` then `set-cluster mainnet-beta` without
`SOL_CONFIRM_MAINNET=yes` → exit 2 (or interactive prompt) + cluster NOT changed on disk.

---

### 🔵 P7-21 — STDERR mnemonic in CI logs (cross-cutting) [LOW]

P7-1 mitigates argv exposure but STDERR mnemonic output (plan Step 5 — "wallet create →
mnemonic → STDERR (red highlight)") persists in CI logs forever. A leaked CI artifact
with the mnemonic = full compromise. Plan's "red highlight" implies terminal escapes
(`\x1b[31m...`) which don't render in non-TTY logs but the raw text is still there.

**Fix.** Add explicit `SECRET:` prefix to the STDERR mnemonic line so log-grep + secret-
scanning (e.g. trufflehog, gitleaks) catches it:
```
SECRET: mnemonic=<word1 word2 ... word12>  # do not commit; do not log
```
Plus `tracing::warn!` with the same prefix for runtime re-emission. Test `cli_wallet.rs`
row 1: capture STDERR, assert line starts with `SECRET:`.

---

### 🔵 P7-22 — `sol --version` exposes build fingerprint [LOW]

Deep-dive L2624: `#[command(name = "sol", version, about = ...)]`. `version` reads
from `Cargo.toml` — exposes sol-wallet-core + Anza SDK version + git SHA. Minor
fingerprinting surface for known-vuln correlation. Acceptable; flag for awareness.

**Fix.** No fix. Document as accepted info-disclosure. CI artifact builds already
expose similar info via `cargo metadata`.

---

### 🔵 P7-23 — `spl send --memo <text>` no length / charset bound [LOW]

Task 7.2 Step 3: `spl send --memo <text>`. SPL Memo program accepts arbitrary bytes.
Excessive length burns CU + bloats tx; charset not validated. If `memo` contains
control chars, downstream log readers may render incorrectly.

**Fix.** Clamp memo length to 566 bytes (SPL Memo program max) + reject NUL bytes:
```rust
if memo.len() > 566 { return Err(Error::InvalidInput { reason: "memo max 566 bytes".into() }); }
if memo.contains('\0') { return Err(Error::InvalidInput { reason: "memo may not contain NUL".into() }); }
```
Test `cli_spl.rs` row 7: memo of 1024 bytes → exit 2.

---

### ✅ P7-24 — `Cluster` enum has NO `Testnet` variant + rejects `testnet` via clap [PASS, info]

Plan L1386 + deep-dive L2662. `Cluster::MainnetBeta | Devnet | Localnet` only. clap
`value_enum` rejects unknown variant "testnet" at parse time → exit 2 + clap error
message. Cross-referenced from companion audit `Risk #12` L2557 (MITIGATED).
Testnet deprecation documented at L378 / L426 / L436. ✅ confirmed.

---

## Pre-Phase-7 ship checklist

A Phase 7.1 implementation PR is not mergeable until every box below flips:

- [ ] **P7-1** `--mnemonic` removed from all clap args (or `--i-understand-argv-exposure` second-factor required); `tests/cli_wallet.rs` row 2 has argv-leak assertion.
- [ ] **P7-2** CLI handlers hold `Zeroizing<Keypair>` (not raw Keypair) across RPC; `tests/cli_wallet.rs` row 9 has heap-zeroize probe; clippy large-types-passed-by-value lint enabled.
- [ ] **P7-3** Plan Step 11 + `handlers/error.rs::classify` use exit-code mapping from §L L2738–2744 (NOT §J L1825); `cli_integration_surfpool.rs` row 9 has 4 exit-code assert tests.
- [ ] **P7-4** `Cli` struct clap sample corrected (no `conflicts_with_all = [wait]`; no `cli.into()` move-before-match); `cli_config.rs` has `--wait --wait-finalized` mutual-exclusion test.
- [ ] **P7-5** `Redact<T>` wrapper + panic-scrubber regex (Phase 8 reuse) applied to `eprintln!("{e:?}")`; `cli_wallet.rs` has secret-not-in-STDERR assertion.
- [ ] **P7-6** `wallet delete` requires `--yes` or interactive confirmation (non-TTY → exit 2); `wallet import --private-key-file` prints derived address preview + requires `--yes`.
- [ ] **P7-7** `--confirm-mainnet yes` env+flag two-tier; bare `--confirm-mainnet` → exit 2; audit-log to STDERR; `cli_mainnet_smoke.rs` (Phase 9.1) gates on this.
- [ ] **P7-8** `set-rpc <url>` validates scheme (https-only unless `--allow-insecure-tls`), rejects userinfo, requires host; `cli_config.rs` has 3 negative tests.
- [ ] **P7-9** `requestAirdrop` cluster guard (mainnet → exit 2 before RPC); `cli_config.rs` has mock-RPC no-call assertion.
- [ ] **P7-10** `wallet rename --to <name>` enforces 1..=64 chars + forbidden-charset + Windows-reserved; `cli_wallet.rs` row 9 has 4 negative tests.
- [ ] **P7-11** `tx wait --timeout` clamped to 1..=600 seconds; `cli_tx.rs` row 16 has out-of-range test.
- [ ] **P7-12** `tracing::instrument(skip(...))` on all secret-bearing fns; CI grep check for `tracing::*!(?keypair|?mnemonic|?secret)` returns ZERO matches; `cli_wallet.rs` row 9 has `SOL_LOG=trace` STDERR-capture test.
- [ ] **P7-13** `--dry-run` and `--sign-only` mutually exclusive (clap `conflicts_with`); `cli_tx.rs` row 13 has together-reject test + alone-assert test.
- [ ] **P7-14** `--skip-memo-required` requires `--i-understand-no-memo-enforcement` second flag; `cli_spl.rs` has both required assertion + audit-log warn.
- [ ] **P7-15** Coverage gate extended to `sol::main`, `sol::cli`, `sol::handlers::{wallet,address,balance,spl,tx,config,error}`; Phase 7.2 verification (lines 1451–1540) gains 5 new loud-RED gate assertions for each `classify()` arm.
- [ ] **P7-16** `address new --mnemonic` removed; CLI uses `--wallet-id --password` or `--mnemonic-file` only.
- [ ] **P7-17** Library adds `WalletManager::derive_pubkey(id, password, account, address_index) -> Result<Pubkey, Error>` returning only Pubkey (no Keypair exposure); `cli_address.rs` row 13 has heap-zeroize probe.
- [ ] **P7-18** Handler docstrings reference companion-audit resolved values (179 APIs; corrected row letters), not deep-dive raw text.
- [ ] **P7-19** Plan Step 6 explicitly references P6-6 fix; `cli_wallet.rs` row 2 has 0644-refuse test.
- [ ] **P7-20** `config set-cluster mainnet-beta` requires `SOL_CONFIRM_MAINNET=yes` or interactive confirmation when transitioning from non-mainnet; `cli_config.rs` has transition-reject test.
- [ ] **P7-21** STDERR mnemonic prefixed with `SECRET:`; log scanners (trufflehog / gitleaks) tuned for pattern; `cli_wallet.rs` row 1 has prefix assertion.
- [ ] **P7-22** No fix — accepted info-disclosure; documented.
- [ ] **P7-23** `spl send --memo <text>` ≤ 566 bytes + no NUL; `cli_spl.rs` row 7 has over-limit reject test.
- [ ] **P7-24** `Cluster` enum NO `Testnet` variant — confirmed (PASS).

All 23 boxes must be ✅ before the Phase 7.1 PR merges into `rust-sol-core`. Phase 7.2
verification (lines 1451–1540 of plan) gains 7 new loud-RED gate assertions: argv-leak
probe (P7-1), heap-zeroize probe (P7-2), 4-arm `classify()` exit-code assertion (P7-3),
clap-mutual-exclusion (P7-4), STDERR-secret-scrub assertion (P7-5), `--confirm-mainnet`
gate (P7-7), mainnet-cluster-switch confirmation (P7-20). A Phase 7.2 verification PR
that runs without these 7 new assertions is not mergeable.

---

## Filing the review issue

A companion GitHub issue must be opened so this audit lands in the v0.1 tracker. The
issue lives on `rust-sol-core` (per Phase Set Up Task S.2 rule), label set
`rust-sol-core` + `backlog` + `security` + `task`, milestone `sol-wallet-core v0.1`,
priority `priority/p0`.

**Body template** (issue body lives in `/tmp/<slug>-body.md` per memory
`gate-guard-gh-pr-classifier.md` — inline body with `rm`/`rmdir` prose trips the
Fact-Force gate):

```text
## Goal

Pre-implementation security review of Phase 7 (`sol` CLI, 22 commands) in
`docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md` (lines 1359–1540).

Closes BEFORE Phase 7.1 implementation begins on `sol/phase7-cli`.

## Verdict

**BLOCK.** 3 🔴 HIGH + 6 🟠 + 9 🟡 + 3 🔵 + 1 ✅ findings — see audit doc for full
catalog.

## Companion

Audit doc: `docs/audit/2026-09-11-sol-wallet-core-phase7-security-review.md`

Companion audit (plan-level CLI drift): `docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md`
(P5-1 🟠, P5-2 🟡, P5-3 🟡 carry into Phase 7 Step 1 + Step 11)

Phase 6 audit (library-side plaintext-key lifecycle): `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md`
(P6-3 🔴, P6-4 🟠 propagate into CLI handlers per P7-2)

## Findings (must clear before Phase 7.1 PR merges)

- [ ] **P7-1** 🔴 `--mnemonic` argv-exposure L12 H-1 still ships
- [ ] **P7-2** 🔴 Plaintext `Keypair` held across RPC in wallet send + send-speedup handlers
- [ ] **P7-3** 🔴 Exit-code table inversion will ship in `classify()` per plan Step 11
- [ ] **P7-4** 🟠 Phase 7 Step 1 copies broken clap sample (P5-3) → build fails
- [ ] **P7-5** 🟠 `eprintln!("{e:?}")` prints anyhow Debug chain → secret leak in STDERR
- [ ] **P7-6** 🟠 `wallet delete` has no confirmation gate — accidental irreversible loss
- [ ] **P7-7** 🟠 `--confirm-mainnet` semantics undefined for non-interactive contexts
- [ ] **P7-8** 🟠 `config set-rpc <url>` accepts arbitrary URL — no scheme/host validation
- [ ] **P7-9** 🟠 `requestAirdrop` not cluster-gated pre-RPC
- [ ] **P7-10** 🟠 `wallet rename --to <name>` no name validation → path traversal / unsafe names
- [ ] **P7-11** 🟡 `tx wait --timeout` no upper bound → DoS via long-lived CLI process
- [ ] **P7-12** 🟡 `tracing_subscriber` at INFO may log signing params / RPC request bodies
- [ ] **P7-13** 🟡 `--dry-run` vs `--sign-only` semantics conflated
- [ ] **P7-14** 🟡 `spl send --skip-memo-required` is unconditional opt-out
- [ ] **P7-15** 🟡 Coverage gate misses all 22 handler paths
- [ ] **P7-16** 🟡 `address new` accepts `--mnemonic` (L1431) — inline secret for derivation
- [ ] **P7-17** 🟡 `address pubkey --wallet-id` triggers full `unlock()` when only pubkey needed
- [ ] **P7-18** 🟡 P5-4 / P5-5 deep-dive drift will propagate to handler docs
- [ ] **P7-19** 🟡 `wallet import --private-key-file` ignores P6-6 source-file mode check
- [ ] **P7-20** 🟡 `config set-cluster mainnet-beta` no confirmation
- [ ] **P7-21** 🔵 STDERR mnemonic in CI logs (cross-cutting)
- [ ] **P7-22** 🔵 `sol --version` exposes build fingerprint (accepted info-disclosure)
- [ ] **P7-23** 🔵 `spl send --memo <text>` no length / charset bound
- [ ] **P7-24** ✅ `Cluster` enum has NO `Testnet` variant — confirmed (PASS)

## Resolution contract

A Phase 7.1 PR is not mergeable until all 23 boxes flip ✅ in this issue body
(update-issues-before-merge rule per memory). Each fix lands in the Phase 7.1 PR that
closes this issue; the checklist-flip + squash-merge happen together.

## Branch

This audit doc lives on `sol/phase7-cli-audit` branched from `rust-sol-core`.
The Phase 7.1 implementation PR will branch from the same `rust-sol-core` and close
this issue.

## Cross-references

- Plan §Phase 7: `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md` L1359–1540
- Plan §Phase 7 verification: `docs/superpowers/plans/2026-09-09-sol-wallet-core-v0.1.md` L1451–1540
- Plan §Phase 8 (FFI panic-scrubber reuse for P7-5): L1543+
- Plan §Phase 9.1 (mainnet smoke gates P7-7): §V0.1 mainnet gate
- Companion audit: `docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md` P5-1..P5-5
- Phase 6 audit: `docs/audit/2026-09-11-sol-wallet-core-phase6-security-review.md` P6-1..P6-14 (P6-3/P6-4 amplified by Phase 7)
- Deep-dive CLI architecture: `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` L2585–3055
- Testnet DEPRECATED 2022-23 (L378, L426, L436, L2557): Solana Foundation unified testing on devnet; P7-24 confirms V0.1 has no testnet variant
```

Open via:

```bash
gh issue create \
  --base rust-sol-core \
  --title "security(sol): Phase 7 sol CLI — BLOCK on 23 findings (3 🔴 HIGH)" \
  --body-file /tmp/sol-phase7-audit-body.md \
  --label "rust-sol-core" --label "backlog" --label "security" --label "task" \
  --milestone "sol-wallet-core v0.1"
```

---

## Resolution

Each `- [ ]` flips to `- [x]` only when the corresponding test or code change lands in
the Phase 7.1 PR and the test passes locally + in CI. The audit issue closes when the
Phase 7.1 PR merges into `rust-sol-core` per `update-issues-before-merge` rule.

Phase 7.2 verification (plan lines 1451–1540) MUST be updated to assert seven new
loud-RED gates: argv-leak probe (P7-1), heap-zeroize probe (P7-2), 4-arm `classify()`
exit-code assertion (P7-3), clap-mutual-exclusion (P7-4), STDERR-secret-scrub assertion
(P7-5), `--confirm-mainnet` gate (P7-7), mainnet-cluster-switch confirmation (P7-20).
A Phase 7.2 verification PR that runs without these seven new assertions is not mergeable.
