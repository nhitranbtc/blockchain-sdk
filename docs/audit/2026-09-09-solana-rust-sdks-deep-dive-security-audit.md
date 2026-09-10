---
title: solana-rust-sdks-deep-dive — security audit (ship-gate)
tracker: TBD — file per `## Filing the audit issue` below
plan: none — no `docs/superpowers/plans/*-sol-*.md` exists as of 2026-09-09
deep-dive: docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md
date: 2026-09-09
status: open
verdict: BLOCK
severity_legend: 🔴 critical · 🟠 high · 🟡 medium · 🔵 low/hardening
guide: .local/plugins-docs/2026-09-06-mattpocock-vs-superpowers-audit-guide.md
---

# solana-rust-sdks-deep-dive — security audit (ship-gate)

Audit of `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` (4,932 lines, 20 top-level
sections) against the eight-axis procedure in
`.local/plugins-docs/2026-09-06-mattpocock-vs-superpowers-audit-guide.md`.

**Scope adaptation.** The guide targets plan documents under `docs/superpowers/plans/`. The
audited artefact is a research deep-dive; the guide's §Pre-audit step 1 permits a user-provided
path. Two consequences, stated up front so the score is not misread:

- **A5 (tracker) and A8 (worktree/branch) score low by construction**, not by defect. No Solana
  plan, issue, label, or branch exists yet. Those axes measure artefacts a research doc is not
  expected to carry.
- Everything else is scored normally. The document makes concrete, implementable, load-bearing
  claims — pinned crate versions, derivation paths, CI workflows, exit codes, fee arithmetic — and
  those are audited as specifications, because that is how an implementer will read them.

**Overall verdict: BLOCK.** Three axes score 0 (A4, A5, A8). Independent of the rubric, two
finding classes must clear before any `sol-wallet-core` code is written:

1. **C1 / P2-1** — the chosen HD-derivation crate implements a different algorithm than the one
   the document names throughout. Shipping as written produces addresses no other Solana wallet
   can reproduce.
2. **P6-1 / P6-2** — both proposed CI workflows run a real-value mainnet transaction on every
   push and pull request, using a mainnet-funded mnemonic stored as a repository secret.

The research is otherwise strong. All 15 pinned Anza/SPL versions verified live and current; the
Token-2022 vs classic ATA footgun analysis, the Metaplex licence exclusion, and the PAL layering
are all sound and worth keeping. The findings below are corrections, not a rejection of the work.

---

## Drift scan (L13 step 4a)

Every version pin, install path, artefact path, and vendor attribution cited by the document,
checked against live state on 2026-09-09.

### Crate version pins — clean

Queried `crates.io/api/v1/crates/<name>` for each pin.

| Crate | Pinned | Exists | Yanked | Latest stable | Verdict |
|---|---|---|---|---|---|
| `solana-sdk` | 4.1.0 | yes | no | 4.1.0 | ✅ at latest |
| `solana-program` | 4.1.0 | yes | no | 4.1.0 | ✅ at latest |
| `solana-client` | 4.2.2 | yes | no | 4.2.2 | ✅ at latest |
| `solana-rpc-client` | 4.2.2 | yes | no | 4.2.2 | ✅ at latest |
| `solana-compute-budget-program` | 4.2.2 | yes | no | 4.2.2 | ✅ at latest |
| `solana-keypair` | 3.1.2 | yes | no | 3.1.2 | ✅ at latest |
| `solana-signer` | 3.0.1 | yes | no | 3.0.1 | ✅ at latest |
| `solana-message` | 4.6.0 | yes | no | 4.6.0 | ✅ at latest |
| `solana-transaction` | 4.3.0 | yes | no | 4.3.0 | ✅ at latest |
| `solana-instruction` | 3.5.0 | yes | no | 3.5.0 | ✅ at latest |
| `solana-zk-sdk` | 7.0.1 | yes | no | 7.0.1 | ✅ at latest |
| `spl-token` | 9.0.0 | yes | no | 9.0.0 | ✅ at latest |
| `spl-token-2022` | 11.0.0 | yes | no | 11.0.0 | ✅ at latest |
| `spl-associated-token-account` | 8.0.0 | yes | no | 8.0.0 | ✅ at latest |
| `spl-memo` | 7.0.0 | yes | no | 7.0.0 | ✅ at latest |
| `ed25519-dalek` | 3.0.0 | yes | no | 3.0.0 | ✅ at latest |
| `ed25519-bip32` | 0.4.3 | yes | no | 0.4.3 | ✅ version live — **but see D-2** |

The Anza subcrate-desync analysis (deep-dive §"Anza subcrate version pinning") is correct and
independently confirmed: the four version families (3.x signer/keypair/instruction, 4.1 sdk/program,
4.2 client/rpc, 4.3–4.6 transaction/message) really are desynced, and exact `=x.y.z` pinning really
is required. Good catch, keep it.

### Drift rows

| # | Claim (line) | Live state | Severity |
|---|---|---|---|
| **D-1** | `cargo install surfpool --locked` (L43, L807, L824, L941) is the install path for the txtx/solana-foundation local validator | crates.io `surfpool` = **0.1.0 only**, published 2026-01-18, description literally `"surf pool"`, 11 LOC, no bin, no real lib — a **squat** (see D-15). The real tool ships as workspace crates `surfpool-core` / `surfpool-types` / `surfpool-db` / `surfpool-sdk` at **1.5.0** (trustpub SHA `86493c4bf716b01d4bedbc531aa9288cb14d5f36`, Apache-2.0, edition 2024, 24k total / 12k recent downloads). The official install path is `curl -sL https://run.surfpool.run/ \| bash` (or source `cargo surfpool-install`, or Docker `surfpool/surfpool`). The deep-dive's tool choice is correct — the install command is wrong. | 🟡 |
| **D-2** | `ed25519-bip32` 0.4.3 implements **SLIP-0010** (L11, L37, L96, L1016, L1041, L1196–1206, L2205, L2516, L3098, L4891, +5 more) | Upstream README: *"implementation of BIP32 for the edwards 25519 curve, based on a **BIP32-ed25519** paper from Dmitry Khovratovich and Jason Law"*, *"compatible with **cardano** key derivation"*. crates.io keywords: `bip32`, `ed25519`, `ed25519-bip32`. No SLIP-0010 anywhere. **Different algorithm.** | 🔴 |
| **D-3** | `ed25519-bip32` is the "**sole Rust option**" for Solana HD (L2516) | `slip10_ed25519` 0.1.3 (*"Private key derivation for SLIP-0010 Ed25519"*) and `slip10` 0.4.3 (*"SLIP-0010: Universal private key derivation"*) both live on crates.io. Anza also ships `solana-seed-derivable` + `solana-seed-phrase` + `Keypair::from_seed_phrase_and_passphrase` — acknowledged by the doc itself at L4490 but deferred to V0.1.5. | 🟠 |
| **D-4** | surfpool GitHub = `github.com/txtx/surfpool`; *"Maintenance: active (txtx, 2025+)"* (L801, L818, L894, L4894) | crates.io `repository` for the 1.5.0 family = **`github.com/solana-foundation/surfpool`**. Project moved orgs. Only `surfpool-subgraph` still points at txtx. | 🟡 |
| **D-5** | `surfpool --version  # any 0.6.x+` (L4171) | No 0.6.x exists in either the `surfpool` (0.1.0) or `surfpool-*` (1.5.0) line. | 🟡 |
| **D-6** | MSRV = **1.89.0** ("mandatory — every Anza crate pins it", L11, L2166, L4787) | Repo-wide standard is **1.98.1** (`.github/workflows/rust-tron-core-ci.yml:127` — *"pinned to 1.98.1 to match `rust-toolchain.toml`"*). The deep-dive's own CI blocks also use `toolchain: "1.98.1"` (L4034, L4067, L4109, L4409) and its Tron-delta table says `MSRV \| 1.98.1 \| 1.98.1` (L4142). A third value, **1.85**, appears at L2171 / L4787 as the "advertised" MSRV. | 🟠 |
| **D-7** | `docs/api/2026-09-08-sol-wallet-core-v0.1-api.md` holds the full ~190-API enumeration (L2351) | **MISSING** | 🟡 |
| **D-8** | `wallets/2026-09-08-sol-wallet-user-stories.md` supplies every story id (L1248, L1468, L1495) | **MISSING** — every story-id column in the doc is therefore unresolvable | 🟡 |
| **D-9** | *(retracted — research doc correctly does not yet have a Solana crate path)* | — | — |
| **D-10** | *(retracted — research doc correctly does not yet have Solana spike tests)* | — | — |
| **D-11** | `.github/workflows/rust-sol-core-ci.yml` + `rust-sol-cli-ci.yml` | **MISSING** — both correctly labelled "planned, not yet landed" | ✅ honest |
| **D-12** | `.local/solana-sdk/` at 117 workspace members | **EXISTS** — member count not independently re-counted this pass | 🔵 |
| **D-13** | *(retracted)* | — | — |
| **D-14** | *(retracted)* | — | — |

---

## Cross-cutting controls (apply to every section)

| ID | Control | Finding | Severity |
|---|---|---|---|
| **C1** | HD derivation must produce the same address as `solana-keygen`, Phantom, and Solflare for a given BIP-39 phrase. | Not met. See P2-1. This is the single control whose failure is unrecoverable for a user: wrong derivation means an imported seed shows an empty wallet, and funds sent to a doc-derived address are reachable only from this tool. | 🔴 |
| **C2** | One authoritative value per fact; no fact stated two ways. | Not met. Contradiction is the dominant defect class: default cluster (3 ways), MSRV (3), derivation path (6), exit-code map (2), config path + format (4), crate count (2), retry backoff (3), test-dir location (3), API count (3), mainnet smoke amount (2). Each is a place where an implementer picks the wrong branch and the doc still "supports" them. | 🟠 |
| **C3** | Secret material never crosses a trust boundary without a single, unambiguous owner. | Not met. FFI raw-secret contract is self-contradictory (P4-1); `sol_wallet_sign_transaction` is an undomain-separated blind-sign surface (P4-2). | 🟠 |
| **C4** | No CI job spends real funds or reads a production-funded secret on an untrusted trigger. | Not met — both proposed workflows violate this. See P6-1, P6-2. | 🔴 |
| **C5** | Every "ready" / "verified" claim carries a runnable verification command. | Not met. 22 CLI commands, 14 spike rows, and 5 compile targets are marked `ready` / `verified (Phase 4)` for a crate that does not exist. The doc flags this itself at L1483 and then contradicts the flag two lines later. | 🟠 |
| **C6** | Arithmetic in summary tables reconciles with the rows it summarises. | Not met in 6 tables. Individually cosmetic; collectively they mean no summary figure in the document can be trusted without re-derivation. | 🟡 |
| **C7** | Rejected dependencies do not reappear as requirements. | Not met — `wiremock` (P6-4). | 🟠 |

---

## P0 — Crate selection, pins, licences, size budget

| ID | Finding | Evidence | Severity |
|---|---|---|---|
| **P0-1** | **`cargo install surfpool --locked` installs a 0.1.0 squat, not the validator.** A developer or CI runner following the doc gets 11 lines of unrelated crate on `PATH`; `SurfpoolGuard::spawn()` then fails, or worse succeeds against something unexpected. Tool choice is correct — surfpool is the canonical `solana-test-validator` drop-in built on LiteSVM and is the right pick. Only the install command is wrong. | D-1, D-15; L43, L807, L824, L941 | 🟡 |
| **P0-2** | Dependency count stated as **~25** (L49: "22 mobile-safe + 2 desktop-only + 1 build-time") and **46** (L319, L2173). Both are presented as totals for the same crate. | L49 vs L319 | 🟠 |
| **P0-3** | The §Summary table (L308–319) does not reconcile with its own rows. Mobile-safe column sums to **45**, stated 44. Desktop-only sums to **1**, stated 2. The "Async + HTTP" row says 6 while the table it summarises (L111–119) lists 7. Root cause: `directories` is counted mobile-safe in the summary but marked `❌ desktop-only` at L107 and listed under "Mobile-unsafe dependencies" at L195. | L107 / L195 / L313 / L319 | 🟡 |
| **P0-4** | `solana-rpc` 4.2.2 appears in the chosen-crates table (L30) and is counted as one of the "11 used directly" (L4462), but is absent from the projected dependency tables (L53–66, ten crates) and from the `Cargo.toml` block (L213–222). It is also an **agave validator-side** crate; pulling it into a wallet drags in a large dependency subtree for no client-side benefit. Decide: drop it, or justify it. | L30 vs L4462 vs L2175 | 🟠 |
| **P0-5** | MSRV drift — three values. See D-6. Under `rust-version = "1.98.1"` in the workspace, "advertise 1.85" is not a split-MSRV pattern, it is a claim `cargo` will reject at build time. | D-6 | 🟠 |
| **P0-6** | Licence summary lists `ed25519-bip32` under **three** licence rows simultaneously — Apache-2.0 (L284), MIT (L285/L2452), and `MIT OR Apache-2.0` (L286/L2453). `argon2` / `aes-gcm` are Apache-2.0 in the summary but `MIT OR Apache-2.0` in the crate table (L42). | L284–287, L2451–2454 | 🟡 |
| **P0-7** | `Cargo.toml` block omits `rustls-native-certs` (a declared dependency at L116) and `tungstenite` (L119), and pins `once_cell` / `regex` as bare `"1"` while every other shared crate uses `{ workspace = true }`. | L210–278 | 🔵 |
| **P0-8** | Binary-size figures are asserted with no measurement method (`~6-7 MB`, `~3 MB`, `~12-15 MB total`). For a mobile budget these are the load-bearing numbers; they need a `cargo bloat` / `cargo size` run behind them or an explicit "estimate, unmeasured" tag. | L292–304, L2335–2347 | 🔵 |

---

## P1 — Networks, clusters, RPC providers

| ID | Finding | Evidence | Severity |
|---|---|---|---|
| **P1-1** | **Default cluster is stated three ways, and the code sample picks the dangerous one.** Section header and table: *"Devnet (V0.1 **default test cluster**)"* (L325, L332, L513). Endpoint rules: *"Default CLI: **Devnet**"* (L340). Precedence rule: *"default **`mainnet-beta`** (production-first; user must explicitly switch to devnet)"* (L346). The clap sample hard-codes it: `default_value_t = Cluster::MainnetBeta` (L2579), and the global-flag table repeats `mainnet-beta` (L2703). An implementer who copies the sample ships a wallet whose unqualified `sol wallet send` targets mainnet. | L325/L332/L340 vs L346/L2579/L2703 | 🔴 |
| **P1-2** | Devnet ATA rent observation of **1,488,440 lamports** (L664) is not a valid rent-exempt minimum. Solana's formula is `(128 + size) × 3480 × 2`; a 165-byte SPL token account gives exactly **2,039,280** (matching L428/L432/L438), an 82-byte mint gives 1,461,600. 1,488,440 ÷ 6960 = 213.86 — not an integer, so it corresponds to no account size. Either the observation is mis-transcribed or it is not a rent figure. The pre-flight check at L1377/L3428 budgets 2,039,280, so a wrong observation here would over-reserve rather than under-reserve — but the number should not stand unexplained in a reference doc. | L664 vs L428 | 🟡 |
| **P1-3** | Config file location and format stated four ways: `~/.local/share/sol/config.json` (L346, L976), `~/.local/share/sol/` (L2701), `~/.local/share/sol-wallet` (L1834), `~/.config/sol/config.toml` (L4202). Format is JSON at L976 and **TOML** at L3575/L3721 ("`SolanaConfig` TOML save → load"). | as cited | 🟡 |
| **P1-4** | Testnet exclusion is well argued and internally consistent (L325, L369–392, L2500). No finding — flagged here as a section that passed. | — | ✅ |

---

## P2 — HD derivation and key material (highest-severity section)

| ID | Finding | Evidence | Severity |
|---|---|---|---|
| **P2-1** | **`ed25519-bip32` implements BIP32-Ed25519 (Khovratovich–Law, Cardano), not SLIP-0010.** The document asserts SLIP-0010 for this crate in 15+ places and lists it as the sole HD dependency. The two schemes differ at master-key generation (SLIP-0010: `HMAC-SHA512("ed25519 seed", S)`; BIP32-Ed25519: bit-clamped seed expansion) and therefore produce **different keys from the same mnemonic**. Consequence: `sol address new` on a phrase imported from Phantom/Solflare/`solana-keygen` yields a different address; the user sees an empty wallet, and any funds sent to the doc-derived address are recoverable only from this tool. This is a fund-loss-class defect, not a documentation nit. | Upstream README (verbatim in D-2); L11, L37, L96, L1016, L1041, L1196, L2205, L3098, L4891 | 🔴 |
| **P2-2** | **The document refutes itself on P2-1 and does not notice.** (a) Its own feature table lists `XPrv::derive_child(CKDPriv::Normal(idx))` — non-hardened derivation — as `ready` (L1202, L3101). SLIP-0010 Ed25519 supports **hardened derivation only**; non-hardened is undefined. (b) It states *"Ed25519 SLIP-0010 does NOT expose a parent public key (`xpub`)"* (L1305, L1648, L1661, L2398, L2492) — true of SLIP-0010, and false of `ed25519-bip32`, whose `XPub` type is the scheme's defining feature. Both facts are only consistent if the crate is BIP32-Ed25519. | L1202/L3101 vs L1305/L1661 | 🔴 |
| **P2-3** | **The derivation path is stated six mutually-incompatible ways.** `m/44'/501'/0'/0'` (L1017) · `m/44'/501'/0'/0'/0'` (L1039, L1206, L4765) · `m/44'/501'/0'/0'/0` (L1302, L4703) · `m/44'/501'/0'/0'/N'` with the code call `XPrv::derive(".../N")` in the same row (L1645) · `m/44'/501'/0'/0/0` claimed as "Phantom/Solflare convention" (L1657, L2951) · `m/44'/501'/0'/0'` via typed `DerivationPath` (L4489). L1039 additionally writes `0'` and then calls it non-hardened in prose: *"Hardened only at 44', 501', 0' — non-hardened at 0' and final index."* Every variant containing a non-hardened component is unimplementable under SLIP-0010 (P2-2a). | as cited | 🔴 |
| **P2-4** | `bip39` API examples mix major versions. `Mnemonic::from_phrase(...)` + `Seed::new(&m, "")` (L1014, L1215, L2464, L3096) are **bip39 1.x**; `Seed` was removed in 2.0. `Mnemonic::generate_in(Language::English, 12)` (L1212, L3094) is **2.x**. The crate is pinned only as `{ workspace = true }` / "latest" (L45, L2204), so neither set is guaranteed to compile. | L1215 vs L1212 | 🟠 |
| **P2-5** | Spike **V10** — *"SLIP-0010 vector (`bip39 "abandon x11 about"` → Phantom/Solflare base58 match)"* — is marked **`ready`** (L2432). Under P2-1 this test cannot pass. It is the one test in the document that would have caught the defect, and it is pre-declared green. | L2432 | 🟠 |
| **P2-6** | Remediation is already in the document, mislabelled as optional: Anza ships `solana-seed-phrase`, `solana-seed-derivable`, `solana-derivation-path`, and `Keypair::from_seed_phrase_and_passphrase` (L4489–L4490, L4643–L4645), deferred to "V0.1.5 (switch to native)". Given P2-1 this is a V0.1 blocker, not a V0.1.5 refactor. | L4490 | — |

**P2 remediation (required before any `keys` module is written):**

1. Pick one derivation implementation and name it once: Anza-native (`Keypair::from_seed_phrase_and_passphrase` + `solana-derivation-path`) **or** `slip10_ed25519`. Do not use `ed25519-bip32`.
2. Fix the path to a single value. `m/44'/501'/0'/0'` is the `solana-keygen` / Phantom default; `m/44'/501'/0'` is the Solflare account path. Whichever is chosen, state it once and derive every other mention from it.
3. Add a **failing-first** cross-wallet vector test before implementation: fixed BIP-39 phrase → expected base58 address captured from `solana-keygen pubkey` (not from our own code). That test is the definition of done for the module.
4. Delete the `xpub`-impossibility prose or re-anchor it to the chosen scheme — it is correct for SLIP-0010 and wrong for BIP32-Ed25519.

---

## P3 — Transaction build, sign, broadcast, fees

| ID | Finding | Evidence | Severity |
|---|---|---|---|
| **P3-1** | Compute-unit figures are internally inconsistent by three orders of magnitude, and none is sourced to a measurement. Classic SPL `transfer_checked` is given as "~150 CU legacy → **~3-5 CU** post-p-token" (L454, L1566, L3411); as a "**~5,000 CU** safe default" (L466, L2499); and the tests assert "CU consumed **≈ 5_000**" as a pass criterion (L3591, L4184). A test asserting ≈5,000 against a stated estimate of ~5 fails on its own terms. (For reference, real classic SPL transfers run in the low thousands of CU, so the "~150 CU legacy" figure looks like the *system-program* transfer cost, not the SPL one.) Re-measure on surfpool and publish one number. | L454 / L466 / L3591 | 🟠 |
| **P3-2** | **Two canonical `send_with_retry` implementations, one of which is signature-invalid.** §D (L1596–1621) re-signs after refreshing the blockhash — correct. The `solana-client` feature map (L4832–4853) clones `tx.signatures` onto the new message and never re-signs; the comment beside it admits *"caller must re-sign with the original keypair, NOT reuse old sigs"* while the code does exactly that. The second version produces a transaction the cluster rejects. | L1596 vs L4832 | 🟠 |
| **P3-3** | *"Each attempt burns 5000 lamports if it lands on stale blockhash before cluster rejects"* (L3444) is false. A transaction rejected for `BlockhashNotFound` is never included in a block and pays nothing; fees are charged only on inclusion. As written it tells the implementer retries cost money and invites a wrong retry budget. | L3444 | 🟠 |
| **P3-4** | Blockhash cache TTL of **60 s** (L3576, L3722, L4710) contradicts §D's rule that blockhash is fetched *"Once per `send_transaction` invocation (right before `sign`)"* (L1586). Blockhash validity is ~60–90 s, so a 60 s-cached hash may be at end-of-life when signed — producing `BlockhashNotFound` on a large fraction of sends under load, exactly the failure the retry loop exists to avoid. | L1586 vs L3576 | 🟠 |
| **P3-5** | Retry backoff stated three ways: `0ms / 100ms / 400ms` (L1590); `100→200→400ms` (L1395, L2046, L2402, L3443); `100→200→400→800ms` with `max_attempts = 3` (L4823 — four delays, three attempts). | as cited | 🟡 |
| **P3-6** | §D retry pseudocode will not compile: `max_attempts: u32 = 3` is not valid Rust (no default arguments), and `&msg.instructions()` yields `&[CompiledInstruction]` where `Transaction::new_signed_with_payer` requires `&[Instruction]`. Labelled pseudocode, but it is the only retry specification an implementer has. | L1597–1610 | 🟡 |
| **P3-7** | Unverified API claims, each presented as a concrete call. `spl_token::instruction::create_mint` (L1159) — spl-token exposes `initialize_mint` / `initialize_mint2`, not `create_mint`. `Pubkey::short()` (L1095, L1670, L3116) — no such method. `tx.to_base64()` (L1032, L1108, L4741) and `VersionedTransaction::try_from(base64)` (L1109, L4742) — serialisation goes through `bincode` + `base64`, there is no such inherent method or `TryFrom` impl. Each needs a `cargo check` against the pinned version or removal. | as cited | 🟠 |
| **P3-8** | *"Ed25519 sign is non-deterministic over blockhash"* (L3442) is wrong — Ed25519 signing is deterministic (RFC 8032). The signature changes because the **message** changed. L3446 states the correct mechanism two lines later. Fix the wording; the operational conclusion (always re-sign) is right either way. | L3442 vs L3446 | 🟡 |

---

## P4 — Wallet persistence, memory hygiene, FFI

| ID | Finding | Evidence | Severity |
|---|---|---|---|
| **P4-1** | **Raw-secret ownership across the FFI boundary is specified two contradictory ways.** `sol_wallet_unlock` writes a 32-byte secret into a **caller-allocated** buffer (L1804, L1822). L1820 says *"FFI NEVER owns the secret key — caller must Zeroize after use."* L2029 says *"Caller MUST call `sol_wallet_lock` (which calls the underlying `Zeroizing::drop`)"*. `sol_wallet_lock` can only zero the Rust-side copy; it cannot reach the caller's buffer. Under the L2029 reading the Dart/Swift/Kotlin copy is never wiped. Pick one contract and make the other an explicit no-op. | L1804/L1820 vs L2029 | 🟠 |
| **P4-2** | `sol_wallet_sign_transaction` is documented as *"sign arbitrary 32-byte hash"* while its parameter is `in_message` (L1807). An FFI export that signs caller-supplied bytes with no domain separation lets any caller obtain a valid signature over a serialised transaction it constructed — the classic blind-signing hole. Require a domain-separation prefix, or restrict the export to signing a `Message` the core itself built. | L1807 | 🟠 |
| **P4-3** | §K FFI "Signature" column is not valid C — the cells describe function pointers (`int (*out_id)(char*, size_t); ...`) rather than the exported symbol signature. `cbindgen` output cannot be reviewed against this. | L1802–1813 | 🟡 |
| **P4-4** | Argon2id parameters (m=64 MB, t=3, p=1), 16-byte salt, 12-byte nonce, `nonce ‖ ciphertext ‖ tag` layout, atomic write + mode 0600, and the JSON envelope are all specified concretely and consistently. No finding — flagged as a section that passed. | L1686–1722 | ✅ |

---

## P5 — CLI surface

| ID | Finding | Evidence | Severity |
|---|---|---|---|
| **P5-1** | **Exit-code mapping is inverted between the specification and the code.** §J table: exit **4** = signing (`SignFailed`), exit **5** = persistence/config/internal (`Wallet*`, `FileIo`, `ConfigInvalid`, `PalError`) (L1785–1794). `classify()`: exit **4** = `WalletNotFound | WalletDecryptFailed`, exit **5** = `SignFailed | FileIo | ConfigInvalid | PalError` (L2664–2690). The output-conventions list (L2781–2787) agrees with the code. Two-against-one, but a spec that ships with a wrong table produces wrong scripts downstream. | L1785 vs L2678 | 🟠 |
| **P5-2** | Handler inventory contradicts the dispatch match. L2544 lists `spl.rs # send, approve, allowance` (omits `balance`, which §spl declares as one of 4). L2558 says the 6 V0.1 handlers are *"wallet, address, spl, stake skipped→V0.2 placeholder, tx, config"* — counting `stake` and omitting `balance`, while the actual match arms (L2634–2640) are Wallet/Address/Balance/Spl/Tx/Config. | L2544/L2558 vs L2634 | 🟡 |
| **P5-3** | Two clap samples will not compile. `#[arg(long, conflicts_with = "wait", conflicts_with_all = [wait])]` (L2765) — `conflicts_with_all` takes string literals, `[wait]` is a bare ident, and the constraint is duplicated. `SolanaConfig::load(&data_dir)?.with_overrides(&cli.into())` moves `cli`, which is then used in `match cli.command` on the next line (L2631–2634). | L2765, L2631 | 🟡 |
| **P5-4** | Public-API count stated three ways for the same crate: `~140` (L2351, L2999), `~152` (L2093, L2095), `~190` (L2374). The per-module table at L2353–2372 actually sums to **179**. | as cited | 🟡 |
| **P5-5** | Coverage-audit letter references are systematically off-by-one against §Coverage B and §Coverage F. `build_spl_approve → B2` (L3808) but approve is **B.3**; revoke `→B3` but is **B.4**; close `→B4` but is **B.5**. `XPrv::sign → F7` (L3841) but F.7 is "derive child hardened"; `xpub → F8` but xpub is **F.13**. | L3807–3842 vs L3028–3049, L3094–3107 | 🟡 |

---

## P6 — Tests and CI (second-highest-severity section)

| ID | Finding | Evidence | Severity |
|---|---|---|---|
| **P6-1** | **Both proposed workflows spend real money on every push and pull request.** `rust-sol-core-ci.yml` sets `RUN_SOL_MAINNET: "1"` in **both** matrix legs (L3995, L4011) under `on: push [main] / pull_request [main]`. `rust-sol-cli-ci.yml` includes a `mainnet` leg under the same triggers, and L4436 states it outright: *"all 3 legs run on push + PR to `main` with matrix fan-out."* This contradicts the gate spec in four places — *"manual pre-release"* (L3557), *"`workflow_dispatch` only"* (L4334), *"Manual trigger only via workflow_dispatch"* (L4365), *"once per V0.1 release cut"* (L493). | L3995/L4011/L4396/L4436 | 🔴 |
| **P6-2** | **A mainnet-funded mnemonic is read by a `pull_request`-triggered job.** `SOLANA_MAINNET_TEST_MNEMONIC` (L4399) and `SOLANA_DEVNET_TEST_MNEMONIC` (L4084, L4392) are consumed by workflows with a `pull_request` trigger. Any change that reaches a run with secrets in scope can exfiltrate the phrase (echo, network call, altered test). Fork PRs get no secrets, so the loud-RED contract (L2408–2413: *panic when env vars absent*) additionally turns every external contribution red. Move all secret-bearing legs to `workflow_dispatch` and use a dedicated throwaway wallet holding only smoke-test funds. | L4084, L4392, L4399 | 🔴 |
| **P6-3** | The `devnet` matrix leg runs the **mainnet** test binary: `RUN_SOL_DEVNET: "1"` + devnet secret, command `cargo test -p sol-wallet-core --test mainnet_smoke` (L4389–4395) — byte-identical to the `mainnet` leg's command. Either it panics (mainnet gate unset) or it runs the mainnet path with devnet credentials. | L4389–4402 | 🔴 |
| **P6-4** | `wiremock` is required by test row 22 (*"12 RPC methods against mock HTTP server"*, L3586, L3680, L3732) but was **explicitly REJECTED** at L1007 — *"REJECTED 2026-09-08 — prefer real local node (`surfpool` subprocess) over trait mocks"* — and appears in no dependency table. Row 22 as written cannot be implemented under the stated policy. | L1007 vs L3586 | 🟠 |
| **P6-5** | CI copy-paste from the Tron workflow leaked through unreviewed: `RUN_SOL_NILE: "1"` (L4010) — **Nile is a TRON testnet**, annotated *"cluster alias — handled by aliases/alias.rs"* as if it were a Solana cluster. | L4010 | 🟠 |
| **P6-6** | The `wallet-core` leg passes `--ignored` (L4027), which runs **only** ignored tests, while its own comment claims it covers *"unit + wiremock RPC + crypto + SPL instruction encode + Token-2022 disambig"*. All non-ignored unit tests are silently skipped. The other legs correctly use `--include-ignored`. | L4027 vs L4007 | 🟠 |
| **P6-7** | The Android compile gate can never execute: `mobile-check` is `runs-on: macos-latest` (L4098) and the Android step is guarded `if: runner.os == 'Linux'` (L4123). It reports green having compiled nothing for Android. | L4098 vs L4123 | 🟠 |
| **P6-8** | The `rust-test-devnet-smoke` job is documented as exercising *"the CLI binary end-to-end … in `spikes/sol-v1/tests/cli_sol_devnet.rs`"* (L4048) but runs `cargo test -p sol-wallet-core --tests` (L4086), which cannot reach that path. The job does not do what its name and comment claim. | L4048 vs L4086 | 🟠 |
| **P6-9** | Mainnet smoke amount contradicts itself by 100×. Gate spec: **$0.001 USDC** (L492, L500, L509, L4282). CLI test row 22: `--amount 100000` annotated *"$0.10 USDC self-send"* (L4204) — 100,000 µUSDC at 6 decimals **is** 0.1 USDC; $0.001 would be `--amount 1000`. | L492 vs L4204 | 🟠 |
| **P6-10** | *"**Audit passes.** No unaccounted gap."* (L3913) — self-certification with no command run, for a test suite whose crate does not exist. Same pattern as C5. | L3913 | 🟠 |
| **P6-11** | CLI test location stated three ways: `crates/sol/tests/` "10 files, ~1400 LOC" (L3652); `spikes/sol-v1/tests/` "~32 files, ~3380 LOC" (L3705, L3792, L3915); `crates/sol-wallet-core/tests/` "(renamed; lives in wallet-core crate)" (L4427). | as cited | 🟡 |
| **P6-12** | Row-number references collide across three numbering schemes (matrix rows 1–38, user-story ids, spike V1–V12) with no disambiguation. Concrete errors: `submit_send_speedup → row 30` (L3607) when row 30 is `send_with_retry` and speedup is row 35; the CLI file map cites "row 21 / row 25, 30" (L4229–4232) which are story ids, not CLI matrix rows. Header says "Entry-point coverage (**16** functions)" over an 18-row table (L3600); "Module × test scenario matrix (**20** modules)" over 34 rows (L3561). | as cited | 🟡 |
| **P6-13** | §Coverage audit totals do not reconcile: *"Total V0.1 features audited: **173**"* (L3012, L3352), but the coverage tables A–T contain **206** rows. Wallet-local modules given as **20** (L3356) and **17** (L2483). | L3012 vs tables | 🟡 |
| **P6-14** | In-document cross-references use absolute line numbers, several already stale — L3541 points at "line ~2480" for Cross-cutting R (actually L2031); L3544 points at "line ~1960" for §C Compute Budget (actually L1550). L3578 uses a GitHub `#L1032-L1128` anchor that does not resolve in rendered markdown and targets the wrong content. Replace with heading anchors. | L3540–3544, L3578 | 🔵 |
| **P6-15** | The loud-RED gated-test contract (L2406–2415) is well specified and worth keeping. No finding — flagged as a section that passed. | — | ✅ |

---

## P7 — Structural / editorial

| ID | Finding | Evidence | Severity |
|---|---|---|---|
| **P7-1** | Garbled sentences that carry technical weight: *"Blockchain never auto-rolls Ed25519 keys (no derivation chain code via SLIP-0010 parent pubkey)"* (Risk #4, L2492); *"Drop `tour-de-force` for Anza `solana-program` modules"* (L2445 — not a Cargo concept). | L2492, L2445 | 🟡 |
| **P7-2** | `directories` called a *"dev-dep"* at L2442; it is an optional regular dependency (L268). | L2442 vs L268 | 🔵 |
| **P7-3** | Stake entry points listed as **6** including `split` (L3623) versus **5** everywhere else (L1432, L2906–2910). | as cited | 🔵 |
| **P7-4** | Group-3 subcrate sub-tables contain duplicate rows across sections (`solana_curve25519`, `solana_bn254`, `solana_big_mod_exp`, `solana_instructions-sysvar`, `solana_program-entrypoint`, `solana-fee-calculator`) and one duplicate **within** a single table (`solana_rent` twice, L4575–4576, the second mislabelled as the "modern fee calc types" crate — that is `solana-fee-structure`, listed separately at L4578). Section row counts (~100) do not match the stated ~70. | L4524–4652 | 🔵 |
| **P7-5** | Section §"Solana Wallet Core — design notes (Solana-only)" (L2508) is an empty heading immediately followed by another `###`. | L2508 | 🔵 |

---

## Ship-gate checklist

Minimum bar before any `sol-wallet-core` code is written or a Solana plan document is opened.

**Blocking — must be resolved, not deferred:**

- [ ] **P2-1/P2-2/P2-3** — one derivation scheme chosen (Anza-native or `slip10_ed25519`; **not** `ed25519-bip32`), one path fixed, every one of the 15+ SLIP-0010 mentions corrected.
- [ ] **P2 remediation step 3** — cross-wallet vector test written and **observed failing** against a `solana-keygen pubkey` ground truth before any `keys` implementation exists.
- [ ] **P6-1/P6-2/P6-3** — every mainnet-gated CI leg moved to `workflow_dispatch`; smoke wallet is a dedicated throwaway funded only with smoke-test amounts; no production-funded secret readable from a `pull_request` trigger.
- [ ] **P1-1** — one default cluster stated once. If the answer is `mainnet-beta`, `--confirm-mainnet` must default to on and be non-bypassable in non-interactive mode.
- [ ] **P0-1** — surfpool install path corrected. Use `curl -sL https://run.surfpool.run/ | bash` (or source `cargo surfpool-install` from `github.com/solana-foundation/surfpool`, or Docker `surfpool/surfpool`). Do **not** use `cargo install surfpool` — the crates.io `surfpool` 0.1.0 crate is an unrelated 11-LOC squat (D-15). Verified live 2026-09-09: real tool = `surfpool-core` 1.5.0 trustpub SHA `86493c4bf716b01d4bedbc531aa9288cb14d5f36`.

**Required before the doc is cited as a specification:**

- [ ] **C2** — each contradiction in P0-2..P0-6, P1-3, P3-1, P3-5, P5-1, P5-4, P6-9, P6-11 resolved to a single authoritative statement.
- [ ] **P3-7** — every asserted API (`create_mint`, `Pubkey::short`, `to_base64`, `TryFrom<base64>`) either compiled against the pinned version or removed.
- [ ] **P3-2/P3-3/P3-4** — one `send_with_retry` specification; the fee claim corrected; the blockhash cache reconciled with fetch-before-sign.
- [ ] **P4-1/P4-2** — single FFI secret-ownership contract; domain separation on the signing export.
- [ ] **C5** — every `ready` / `verified` demoted to `unverified (by inspection)` until a spike PASS block exists, per the document's own rule at L1483.
- [ ] **P6-4** — `wiremock` either un-rejected and added to dev-deps, or row 22 rewritten against surfpool.
- [ ] **D-6** — MSRV set to the repo standard (1.98.1) or the divergence justified in an ADR.
- [ ] **D-7/D-8** — the two referenced-but-missing companion docs written, or the references removed.

**Standard gates (inherited, apply once code exists):**

- [ ] `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` green.
- [ ] No `unimplemented!()` / `todo!()` / `// FIXME` in paths claimed done.
- [ ] `cargo geiger` shows no new `unsafe` without justification.
- [ ] `SECURITY.md` updated — the FFI raw-secret export and the mainnet smoke gate are new attack surface.

---

## Audit axes score

| Axis | Score | Basis |
|---|:--:|---|
| **A1** Drift scan | **1**/3 | 15/15 crate pins verified live and current — genuinely good. Offset by D-2 (wrong algorithm attributed to a crate), D-1/D-4/D-15 (surfpool install command wrong and a crates.io squat exists with the obvious name — tool choice, vendor, and `surfpool-core` 1.5.0 version are correct), D-5 (phantom `0.6.x` version), D-6 (MSRV drift vs repo standard), D-7/D-8 (two referenced docs missing). |
| **A2** Threat model | **1**/3 | An 18-row risk register exists and covers real Solana-specific hazards (ATA seed footgun, blockhash expiry, Metaplex licence, devnet wipes). It contains no entry for the four highest-severity findings in this audit: derivation mismatch, mainnet-on-PR, CI secret exposure, FFI blind-signing. |
| **A3** Test coverage | **1**/3 | Coverage matrices are unusually thorough — 38 rows, per-file mapping, reverse mapping, fixture inventory, explicit gap table. Undercut by: the one test that would catch P2-1 is pre-marked `ready` (P2-5); a rejected dependency is required (P6-4); letter/row references are systematically off (P5-5, P6-12); totals do not reconcile (P6-13); the suite self-certifies (P6-10). |
| **A4** Iron-law compliance | **0**/3 | **BLOCK.** No failing-test-first ordering anywhere. 22 commands, 14 spike rows, and 5 compile targets marked `ready` / `verified (Phase 4)` for a crate that does not exist, with no verification command attached to any of them. The document states the rule itself at L1483 and then violates it in the table two lines below. |
| **A5** Tracker alignment | **0**/3 | **BLOCK, by construction.** No Solana issue exists (`gh issue list --search solana` → empty). No `rust-sol-core` label exists. No plan document. No ADR for the Anza-vs-alternatives decision, which is exactly the architectural choice an ADR exists to record. Doc is untracked. |
| **A6** Spec-implementation distance | **2**/3 | Strongest axis. Deep modules with real seams (4-trait PAL, `chain` / `tx` / `wallet` split); errors carry actionable context (`InsufficientFunds { required_lamports, available }`, `InvalidTokenProgram { mint, expected, actual }`); protocol detail is wrapped, not leaked. Deductions: `disambig` is a grab-bag of five unrelated guards, and the FFI leaks a raw 32-byte secret through the public boundary. |
| **A7** Iron anti-patterns | **1**/3 | *"Audit passes"* with nothing run (L3913). *"Status: ready … by inspection, NOT by spike PASS"* immediately followed by a table of `ready` (L1483). Heavy deferral: 25 features to V0.1.5, 20 to V0.2, 5 to V0.3 — including P2-6, the fix for the critical finding. |
| **A8** Worktree / branch discipline | **0**/3 | **BLOCK, by construction.** No branch named; sibling chains use `rust-tron-core` / `evm/phase-N-*`. No worktree step, no finishing-a-branch step, no issue-close checklist. The doc sits untracked on `main` alongside 8 other untracked paths. |
| **Total** | **6**/24 | Three axes at 0 → **BLOCK ship** per guide §Audit axes. |

**Reading the score.** A5 and A8 are zero because this is research, not a plan — that is the guide's own framing and should not be read as a defect in the research. The score that matters is **A4 = 0** alongside the 🔴 rows in P2 and P6: the document asserts readiness it has not verified, and the two things it is most confident about (the HD crate, the CI workflows) are the two that are wrong. Fixing P2 and P6 and demoting the unverified `ready` claims moves this to roughly 12/24 and unblocks a plan document.

---

## What to keep

Recorded so remediation does not discard the good work:

- The full Anza subcrate desync analysis and the `=x.y.z` exact-pin requirement — verified correct.
- The Token-2022 vs classic ATA derivation footgun (`token_program_id` as a PDA seed) and the `disambig::reject_wrong_token_program` guard — the highest-value correctness insight in the document.
- Dynamic decimals via `Mint::unpack` with `transfer_checked` mandatory — right call, consistently applied.
- The Metaplex `NFT Open Source License v1.0` exclusion — correctly identified as non-OSI and correctly scoped out.
- Testnet exclusion, with migration table (P1-4).
- Argon2id + AES-256-GCM wallet-file design (P4-4).
- The loud-RED gated-test contract (P6-15).
- Surfpool as the local-validator strategy — correct tool choice (drop-in for `solana-test-validator`, LiteSVM-based, used in production by hundreds of Solana devs). Only the install command is wrong; fix is `curl -sL https://run.surfpool.run/ | bash`. Also worth adding to the V0.1 design: surfpool's built-in MCP server (`surfpool mcp`) — free agent integration that pairs well with the doc's existing PAL layering.

---

## Filing the audit issue

Per guide §Output template, the audit doc is filed **first**, then the issue links to it. Two
preconditions before the command below can run:

1. **The `rust-sol-core` label does not exist.** Current chain labels: `rust-eth-core`,
   `rust-tron-core`, `rust-tron-cli`, `rust-evm-core`. Create it first:

   ```bash
   gh label create rust-sol-core \
     --description "Solana core crate work — rust-wallet-app/crates/sol-wallet-core/ (future)" \
     --color 9945FF
   ```

2. **Commit this audit doc first**, so the issue body can reference a landed path.

Then file the issue. Per the `gate-guard-gh-pr-classifier` lesson, use `--body-file` with the body
staged in `/tmp` rather than inline prose:

```bash
gh issue create \
  --title "audit: solana-rust-sdks-deep-dive — 5 critical findings, BLOCK before sol-wallet-core work" \
  --label rust-sol-core --label backlog --label task \
  --body-file /tmp/sol-deepdive-audit-issue.md
```

Issue body should carry only: link to `docs/audit/2026-09-09-solana-rust-sdks-deep-dive-security-audit.md`,
the 🔴 ids (C1/P2-1, P2-2, P2-3, P1-1, P6-1, P6-2, P6-3), the axis total, and the blocking
ship-gate checkboxes. Do not duplicate the finding tables into the issue body — the doc is the
canonical list (guide §Anti-patterns).

**PAUSE.** No label creation, no commit, no `gh issue create` executed by this audit pass.
Awaiting human review and approval per `workflow-approval-required` and `never-auto-commit`.

---

## Sources consulted

- `.local/plugins-docs/2026-09-06-mattpocock-vs-superpowers-audit-guide.md` — audit procedure
- `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` — audited artefact, read in full (4,932 lines)
- `crates.io` API — 22 crate/version queries, 2026-09-09 (15 pin checks + 7 surfpool-related)
- `github.com/solana-foundation/surfpool` README + crate inventory — surfpool tool identity for D-1 reframe
- `crates.io/api/v1/crates/surfpool` + `crates.io/api/v1/crates/surfpool-core` — squat vs real-crate distinction for D-15
- `github.com/typed-io/rust-ed25519-bip32` README — scheme identity for D-2
- `.github/workflows/rust-tron-core-ci.yml` — MSRV baseline for D-6
- `rust-wallet-app/crates/` — workspace inventory
- `gh label list`, `gh issue list --search solana` — A5 tracker state
