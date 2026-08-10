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
| 30 | L20 constant audit | ~150k | ~50k | unknown (assumed Sonnet-equivalent) | ~$1.20 | Retroactive; 1 review-fix commit; --admin merge |
| 31 | F21 typed Sighash | ~1.2M | ~400k | unknown (assumed Sonnet-equivalent) | ~$9.60 | Retroactive; 3-agent parallel review (heavy); CI deny.toml fix |
| Process | estimate-report.md (client bill) | ~80k | ~30k | unknown | ~$0.65 | Retroactive; mid-session pivot from eng self-improvement to client billing |
| Process | docs/ housekeeping | ~50k | ~20k | unknown | ~$0.42 | Retroactive; doc + lesson work |
| **Total (retroactive est.)** | | **~1.48M** | **~500k** | | **~$11.87** | Subject to ±50% (rates + token split assumed) |

## Live tracking (going forward)

For new tasks, append a row at task pickup with token counts from `usage` field on each response. Update cost at L13.17 with final totals.

```
| NN | <title> | <input> | <output> | <model> | <computed> | <notes> |
```

## Why separate from `docs/estimate-report.md`

L20 client bill shows hours × rate. AI token cost is engineer overhead, not client deliverable. Conflating them leaks internal cost structure and creates billing volatility. Two docs, two audiences:

- `docs/estimate-report.md` → client (hours × $50)
- `docs/ai-cost-report.md` → engineer / internal (tokens × model rate)