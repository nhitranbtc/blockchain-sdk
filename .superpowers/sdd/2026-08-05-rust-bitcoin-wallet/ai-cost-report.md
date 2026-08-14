# AI Cost Report — per-task token + cost tracking

Per-task AI (Claude) spend: input tokens, output tokens, model, computed cost. Internal; not for client billing.

> **Caveat (retroactive):** Token counts are session-aggregate estimates from cost-warning signals, not per-task exact figures. Per-task accounting requires the host runtime to expose `usage.input_tokens` / `output_tokens` per task. The session began without per-call token logging, so retroactive rows below are `~` qualified.

## Cost basis

Default rates (Anthropic Sonnet 4.5, mid-2026 reference):

| Model | Input ($/MTok) | Output ($/MTok) |
|---|---|---|
| Sonnet 4.5 | 3.00 | 15.00 |
| Haiku 4.5 | 0.80 | 4.00 |
| Opus 5 | 15.00 | 75.00 |

`cost = (input_tokens × input_rate + output_tokens × output_rate) / 1_000_000`

If the host model is unknown (this session: model ID `MiniMax-M3`, not a published Claude SKU), rates are best-effort estimates. Mark `model` column with actual host model ID when known.

## Tasks

| # | Title | Input (Tok) | Output (Tok) | Model | Est. cost (USD) | Notes |
|---|---|---|---|---|---|---|
| 19a | Task 9a Wallet::from_mnemonic | ~1.4M | ~480k | unknown | ~$11.20 | 3-agent critical-tier review (7 findings, 1 of 3 rounds); PR #48 merged (SHA `a34fe0e`) |
| 19b | Task 9b Wallet::sync (partial) | ~250k | ~80k | unknown | ~$2.00 | URL validation + coin_type_for + descriptor path; Esplora build + start_full_scan + F14 deferred; PR #51 merged |
| 19c | Task 9c Wallet::balance (partial) | ~200k | ~70k | unknown | ~$1.60 | URL validation + coin_type_for; bdk_wallet::Wallet construction + UTXO aggregation deferred |
| 20 | Task 8 chain::network (coin_type_for) | ~200k | ~70k | unknown | ~$1.60 | 1 review-fix round (7 findings); PR #42 merged |
| 30 | L20 constant audit | ~150k | ~50k | unknown (assumed Sonnet-equivalent) | ~$1.20 | Retroactive; 1 review-fix commit; --admin merge; PR #38 merged |
| 31 | F21 typed Sighash | ~1.2M | ~400k | unknown (assumed Sonnet-equivalent) | ~$9.60 | Retroactive; 3-agent parallel review (heavy); CI deny.toml fix; PR #39 merged 2026-08-10 |
| Process | docs/estimate-report.md (client bill) | ~80k | ~30k | unknown | ~$0.65 | Retroactive; mid-session pivot from eng self-improvement to client billing; PR #40 merged |
| Process | docs/ai-cost-report.md | ~30k | ~10k | unknown | ~$0.24 | Retroactive; this file |
| Process | tasks/lessons.md L21-L23 | ~60k | ~25k | unknown | ~$0.50 | Retroactive; rule captures |
| Process | Phase 1 closure — PR #96 (CI workflow + plan drift + CHANGELOG cascade) | ~150k | ~50k | MiniMax-M3 (session model) | ~$0.50 | House-keeping; L13 step 11 verify gate (fmt + clippy + test 89/0/1 + geiger); PR #96 SHA `2c1c5e7` |
| Process | PR #97 (Task 1 Step 10 flip — L13 step 14 post-merge) | ~15k | ~5k | MiniMax-M3 (session model) | ~$0.05 | Doc-only; 1-line plan checkbox flip; PR #97 SHA `51822ad` |
| Process | Branch cleanup (post-#96, post-#97) + L21 update prep | ~20k | ~8k | MiniMax-M3 (session model) | ~$0.07 | `git branch -d` (chore/phase-1-closure local) + remote deletes for both PRs + this L21 PR prep |
| Feature | PR #101 — `btc wallet import` subcommand (Story 2 / Issue #99) | ~200k | ~60k | MiniMax-M3 (session model) | ~$0.70 | TDD red-green (5 lib + 6 CLI tests) + verify gate + demo script; PR #101 SHA `90d6e6f` |
| Feature | PR #102 — security fix + demo expansion (all 5 BIP-39 word counts) | ~150k | ~45k | MiniMax-M3 (session model) | ~$0.50 | Remove `--passphrase` (broken-security-control); expand Step 4 to 12/15/18/21/24; PR #102 SHA `786ce4b` |
| Process | L21 update for #101 + #102 + branch cleanup | ~10k | ~4k | MiniMax-M3 (session model) | ~$0.03 | Estimate-report footer entries + ai-cost-report rows; L21 ledger closure |
| Feature | PR #105 — `btc config show` subcommand (Story 11 / Issue #100) | ~180k | ~55k | MiniMax-M3 (session model) | ~$0.65 | TDD red-green (5 CLI tests) + verify gate + Try it examples; PR #105 SHA `4de4ea7` |
| Process | PR #106 — CI workflow reorganization (move btc-import-demo to btc-cli-demo.yml) | ~50k | ~15k | MiniMax-M3 (session model) | ~$0.18 | Architectural move (ci.yml → btc-cli-demo.yml) + bug fix (SKIP_BUILD=1); PR #106 SHA `ecb0228` |
| Process | Phase 1 §MVP final closure (issue #100 ACs flip + plan file marker) | ~10k | ~4k | MiniMax-M3 (session model) | ~$0.03 | L13 step 14 + plan marker |
| Feature | PR #114 — `btc-regtest-smoke` testcontainers suite (Issue #112) | ~250k | ~80k | MiniMax-M3 (session model) | ~$0.95 | testcontainers regtest smoke (2/3 tests pass) + F36 localhost exception + tprv prefix fix for regtest + ci.yml multi-job split + cargo deny/audit ignore flags; PR #114 SHA `a5df1ab` |
| **Total (retroactive est. + Phase 1 closure + wallet-import + Story 11 + testcontainers)** | | **~2.82M** | **~953k** | | **~$18.37** | Subject to ±50% (rates + token split assumed); Phase 1 closure subset ~$0.62, wallet-import subset ~$1.20, Story 11 subset ~$0.65, testcontainers subset ~$0.95 |

## Live tracking (going forward)

For new tasks, append a row at task pickup with token counts from `usage` field on each response. Update cost at L13.17 with final totals.

```
| NN | <title> | <input> | <output> | <model> | <computed> | <notes> |
```

## Why separate from `docs/estimate-report.md`

L20 client bill shows hours × rate. AI token cost is engineer overhead, not client deliverable. Conflating them leaks internal cost structure and creates billing volatility. Two docs, two audiences:

- `docs/estimate-report.md` → client (hours × $50)
- `docs/ai-cost-report.md` → engineer / internal (tokens × model rate)