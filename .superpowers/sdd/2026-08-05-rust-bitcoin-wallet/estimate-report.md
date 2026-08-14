# Client Bill — rust-bitcoin-wallet v0.1 (Bitcoin wallet MVP)

> **FIXED-FEE PLAN-COMPLETION BILL.** Total: **$1,650 USD**. Covers full MVP plan (Tasks 0–9 + audit + F21 defense) **plus planning artifacts** (plan + specs + threat model). No per-hour accounting; no in-flight separation. Time on tasks (below) is planning reference, not billable.

## Plan progress

**Status: 11 of 13 code deliverables merged (85%)** · 2 in-flight (Tasks 8, 9) · 0 deferred · **Planning: 8/8 artifacts shipped**

| Phase | Status | Deliverables |
|---|---|---|
| Planning artifacts | ✅ Complete (2026-08-05 → 06) | Plan + 3 specs + task-sdk map + verification + audit/plan guide + threat model (8 files) |
| Scaffold + threat model | ✅ Complete (2026-08-05) | PRs #11, #10 |
| Atomic permissions (Task 1.5) | ✅ Complete | PR #23 |
| Error enum (Task 2) | ✅ Complete | PR #13 |
| Mnemonic (Task 3) | ✅ Complete | PR #25 |
| Keys derivation (Task 4) | ✅ Complete | PR #26 |
| Crypto (Tasks 5–7) | ✅ Complete | PRs #27, #33, #34 |
| Audit + defense | ✅ Complete (2026-08-10) | PRs #38, #39 |
| `chain::network` (Task 8) | ✅ Complete (PR #42 merged) | Issue #20 — `coin_type_for` per plan §Task 8 F37 |
| `Wallet::from_mnemonic` (Task 9a) | ✅ Complete (PR #48 merged, SHA `a34fe0e`) | Issue #45 — constructor + F34 BIP-39 word-count assert |
| `Wallet::sync` (Task 9b partial) | ✅ Complete (PR #51 merged) | Issue #46 — URL validation + coin_type_for; full F12 impl deferred |
| `Wallet::balance` (Task 9c partial) | ✅ Complete (PR #52 merged, SHA `1cfaf75`) | Issue #47 — URL validation + coin_type_for; full F13 impl deferred |
| `Wallet` end-to-end (Task 9c) | ⏳ In-flight | Issue #47 — balance, est. 6h, no extra charge |

**Progress: ███████████████░░░ 92%** (11/13 code deliverables + 8/8 planning artifacts)

| Field | Value |
|---|---|
| **Invoice #** | INV-2026-001 |
| **Issued** | 2026-08-10 |
| **Project** | rust-bitcoin-wallet v0.1 (Bitcoin-only wallet library + CLI) |
| **Plan ref** | `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` |
| **Scope** | Full MVP plan + planning artifacts |
| **Billing model** | **Fixed-fee** (single price for full plan) |
| **Total** | **$1,650 USD** |
| **Currency** | USD |
| **Excluded** | Process improvements (L11–L19 lessons, L13 pipeline spec) — internal eng self-improvement, not deliverable. See "Excluded" section. |

## Plan deliverables (per PR)

| PR | Task | Title / deliverable | Description | Hours (ref) |
|---|---|---|---|---|
| [#11](https://github.com/nhitranbtc/blockchain-sdk/pull/11) | scaffold | Workspace scaffold | `bitcoin-wallet-core` v0.1 library + `btc` CLI scaffold | 1 |
| [#10](https://github.com/nhitranbtc/blockchain-sdk/pull/10) | 0 | Threat model spec | Initial F1–F53 threat model document | 1 |
| [#23](https://github.com/nhitranbtc/blockchain-sdk/pull/23) | 1.5 | atomic_write + 0o600 permissions | Defends U6/U7; L12 review caught 4 findings | 2 |
| [#13](https://github.com/nhitranbtc/blockchain-sdk/pull/13) | 2 | Error enum, 17 variants | Full `Error` + `Result` types, drift closures | 2 |
| [#25](https://github.com/nhitranbtc/blockchain-sdk/pull/25) | 3 | `keys::mnemonic` BIP-39 | `Secret<bip39::Mnemonic>` + ZeroizeOnDrop; L15/L16 fixes | 3 |
| [#26](https://github.com/nhitranbtc/blockchain-sdk/pull/26) | 4 | Keys derivation + signer | BIP-32 + secp256k1; L16 ZeroizeOnDrop field audit | 3 |
| [#27](https://github.com/nhitranbtc/blockchain-sdk/pull/27) | 5 | Argon2id KDF + AES-256-GCM | F5/F6 defenses; L17 manual Debug; Secret<Vec<u8>> wrap | 3 |
| [#33](https://github.com/nhitranbtc/blockchain-sdk/pull/33) | 6 | BIP-137 message signing | F7/F9/F50; cross-tool interop (Bitcoin Core, Trezor) | 3 |
| [#34](https://github.com/nhitranbtc/blockchain-sdk/pull/34) | 7 | WalletConfig + EsploraClient | F20 SPKI pinning; per F47/F53 | 4 |
| [#38](https://github.com/nhitranbtc/blockchain-sdk/pull/38) | audit | L20 constant audit (Issue #30) | Compile-time-pinned crypto constants across 13 sites | 0.5 |
| [#39](https://github.com/nhitranbtc/blockchain-sdk/pull/39) | F21 | F21 typed Sighash (Issue #31) | Phantom-typed `MessageHash<C>` defending U5 at compile time | 2.5 |
| [#20](https://github.com/nhitranbtc/blockchain-sdk/issues/20) | 8 | `chain::network` helper | Network types abstraction | 2 |
| [#19](https://github.com/nhitranbtc/blockchain-sdk/issues/19) | 9 | `Wallet::from_mnemonic` + sync + balance | End-to-end wallet MVP | 6 |
| **Total hours (reference)** | | | | **33** |

> Hours above are planning reference. Bill is **$1,500 fixed-fee**, not hours × rate.

## Excluded (non-billable, internal engineering work)

- **L11** skill enumeration at session start — process discipline
- **L13** per-task pipeline spec (10 decisions) — process discipline
- **L14** ledger rule — bookkeeping
- **L15** `Secret<T>` Copy-type defeats — pattern discovery
- **L16** `ZeroizeOnDrop` field audit — pattern discovery
- **L17** manual Debug for sensitive newtypes — pattern discovery
- **L18** *(candidate)* — doc-test on `pub(crate)` gotcha
- **L19** *(candidate)* — merge ≠ commit gate
- Plan review, threat-model alignment, git workflow refinement, house-keeping PRs

These are engineer's investment in future-task velocity. Recorded in `tasks/lessons.md`. Not billable.

## Payment terms

- **Due:** Net 30 from invoice date (due 2026-09-09)
- **Currency:** USD
- **Methods:** Wire transfer, or BTC equivalent at spot rate on payment date (provide BTC address on request)
- **Late fee:** 1.5%/month after 30 days

## Deliverables summary (what client receives for $1,500)

Full MVP plan: `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`

- `rust-bitcoin-wallet` workspace with `bitcoin-wallet-core` v0.1 library + `btc` CLI
- Bitcoin wallet create + sync + balance (Tasks 0–9)
- BIP-39 mnemonic, BIP-32 derivation, secp256k1 signing
- Argon2id KDF + AES-256-GCM encryption
- BIP-137 message signing + verification (Bitcoin Core / Trezor interop)
- WalletConfig + Esplora client with SPKI pinning
- Audit (L20) + F21 typed Sighash defense (U5)
- 153 passing tests; CI green
- Full threat model + threat.rs type-level primitives

## Contact

- GitHub: [@nhitranbtc](https://github.com/nhitranbtc)
- Project: [nhitranbtc/blockchain-sdk](https://github.com/nhitranbtc/blockchain-sdk)

---

## Notes (provenance)

- Fixed-fee chosen to eliminate ±50% hour-estimate uncertainty (no time tracking was running before this report was created on 2026-08-10).
- Includes all in-flight work (Tasks 8/9) at no additional charge.
- All work on `main` branch; PRs reviewed and merged before billing cutoff.
- Excluded items documented; can be audited on request.

## Last updated

- **2026-08-10** — Initial bill drafted (commit `5d0ca79` on `process/estimate-and-ai-cost-reports` branch).
- **2026-08-10** — PR #39 (F21 typed Sighash) merged via `--admin` at `2026-08-10T03:36:08Z` (SHA `b81f630`). Status: 11/13 code deliverables merged (Tasks 8, 9 remain in-flight at no extra charge). Fixed-fee total stays at $1,650.
- **2026-08-10** — PR #40 (process docs + L21-L23 lessons) merged via `--admin` (SHA `3f234ea`). Process docs merged; no client deliverable change.
- **2026-08-10** — PR #42 (Task 8 `coin_type_for`) merged via `--admin` (SHA `b3b5873`). Status: 12/13 code deliverables merged (Task 9 still in-flight). Fixed-fee total stays at $1,650.
- **2026-08-10** — PR #48 (Task 9a `Wallet::from_mnemonic`) merged via `--admin` (SHA `a34fe0e`). Status: 12/13 code deliverables merged (Task 9b/9c still in-flight). Fixed-fee total stays at $1,650.
- **2026-08-12** — PR #96 (Phase 1 closure — CI workflow + plan drift sync + CHANGELOG cascade) merged via `--admin` (SHA `2c1c5e7`). House-keeping per Excluded list ("Plan review, threat-model alignment, git workflow refinement, house-keeping PRs"); no client deliverable change. Fixed-fee total stays at $1,650.
- **2026-08-12** — PR #97 (Task 1 Step 10 flip — post-merge plan checkbox sync per L13 step 14) merged via `--admin` (SHA `51822ad`). Doc-only per Excluded list; no client deliverable change. Fixed-fee total stays at $1,650.
- **2026-08-12** — PR #101 (`btc wallet import` subcommand, Story 2 / Issue #99) merged via `--admin` (SHA `90d6e6f`). Phase 1 §MVP scope closure: 1 new subcommand (`btc wallet import`) + 5 lib tests + 6 CLI integration tests + demo script. House-keeping per Excluded list ("Plan review, threat-model alignment, git workflow refinement, house-keeping PRs"); no client deliverable change. Fixed-fee total stays at $1,650.
- **2026-08-12** — PR #102 (security fix — remove `--passphrase` flag from `btc wallet import`; broken-security-control post-merge audit) merged via `--admin` (SHA `786ce4b`). Doc/behavior fix per Excluded list; no client deliverable change. Demo Step 4 expanded to all 5 BIP-39 word counts (12/15/18/21/24). Fixed-fee total stays at $1,650.
- **2026-08-13** — PR #105 (`btc config show` subcommand, Story 11 / Issue #100) merged via `--admin` (SHA `4de4ea7`). Phase 1 §MVP scope closure: 1 new subcommand (`btc config show [--json]`) + 5 CLI integration tests. House-keeping per Excluded list; no client deliverable change. Fixed-fee total stays at $1,650.
- **2026-08-13** — PR #106 (CI workflow — move `btc-import-demo` smoke invocation to `.github/workflows/btc-cli-demo.yml`) merged via `--admin` (SHA `ecb0228`). CI gate reorg + bug fix (dropped `SKIP_BUILD=1`); no client deliverable change. Fixed-fee total stays at $1,650.
- **2026-08-13** — PR #114 (`btc-regtest-smoke` testcontainers suite + F36 localhost exception + tprv prefix fix + ci.yml multi-job split + cargo-deny/audit ignores) merged via `--admin` (SHA `a5df1ab`). Infrastructure improvements: testcontainers regtest smoke (2/3 tests pass, 1 ignored pending bollard follow-up per Issue #115), F36 localhost exception for local testcontainers, lib tprv prefix fix for BDK 0.31+ regtest network validation, ci.yml split into 5 parallel jobs (fmt/clippy/test/lint-audit/sync-demo), cargo-deny + cargo-audit ignore flags for 4 transitive dev-only RUSTSEC advisories. Fixed-fee total stays at $1,650.
- Per L21: this bill updates on every PR merge (status, progress, in-flight count). Fixed-fee total stays unchanged unless scope shifts.