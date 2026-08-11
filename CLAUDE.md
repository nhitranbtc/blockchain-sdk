# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working
with code in this repository.

## What this is

Markdown research + analysis workspace for blockchain SDK investigations.
Focus: Bitcoin wallet SDKs (rust-bitcoin, BDK), Lightning,
stablecoin integration, wallet security comparisons.

Two coexisting layers:

- **Docs/research layer** (`docs/`) — pure prose, markdown only
- **Code layer** (`rust-wallet-app/`) — Rust workspace containing `bitcoin-wallet-core/` (v0.1 library) + `btc/` (v0.1 CLI) + `chain-traits/` (umbrella v0.2 scaffold, exists)

## Agent skills

- **Tracker:** GitHub via `gh` — `docs/agents/issue-tracker.md`
- **Triage labels:** canonical — `docs/agents/triage-labels.md`
- **Domain docs:** glossary at — `docs/agents/domain.md`

## Implementation workflow (rust-bitcoin-wallet v0.1)

### Session-start rule

Every session begins by stating the verifiable goal of the session in one sentence. If unclear, ask before acting. Per karpathy-guidelines §1 "Think Before Coding".

Scan `tasks/lessons.md` for relevant project lessons before starting work (auto-loaded via SessionStart hook; new entries after any correction).

**Session-start goal prompt template:**

```text
Session goal: <verifiable outcome in one sentence>
Success criteria: <observable check that proves goal met>
Scope: <in-scope items> | <out-of-scope items>
Pauses required: <state-modifying actions needing approval>
```

**Example:**

```text
Session goal: Commit the review-cleaned Bitcoin v0.1 plan to main.
Success criteria: `git log -1` shows new commit with plan file changes.
Scope: `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md` only.
Pauses required: commit (per never-auto-commit).
```

v0.1 deliverables per the merged plan: `rust-wallet-app/crates/bitcoin-wallet-core/` (library) + `rust-wallet-app/crates/btc/` (CLI), inside the umbrella `rust-wallet-app/` workspace. `chain-traits/` exists as the v0.2 umbrella scaffold.

## Operational rules

**Single source of truth: [`tasks/lessons.md`](tasks/lessons.md)** — 10 numbered lessons (L1, L6, L8, L9, L11-L14, L21, L24, L28). Gaps L2-L5, L7, L10, L15-L20, L22-L23, L25-L27, L29-L34 retired per audit. **L13 is the per-task pipeline spec** (19 steps) — apply it literally on every task pickup: skill-tag → TDD → L12 review → verify (step 11) → backlog triage (11a) → L24 doc updates on local branch (was 11b, now lives at 15b) → PAUSE (12) → commit-push-pr (13) → flip checkboxes (14) → PR review + merge + close (15) → tech doc (15a) → verify L24 + release-cut (15b) → ledger (17) → harvest lessons (18) → L21 reports (19).

Read at every task pickup per L11.

`~/.claude/projects/-home-nhitran-Projects-blockchain-sdk/memory/MEMORY.md` (auto-loaded via SessionStart hook) holds only the cross-project subset (never-auto-commit, workflow-approval-required, update-issues-before-merge, l13-read-on-task-pickup). Project-specific rules live in `tasks/lessons.md` only — not mirrored to memory.

Current execution target: v0.1 `bitcoin-wallet-core` library inside `rust-wallet-app/crates/` per active plan.

## Conventions

- **Docs/research layer** (`docs/`): no build, no tests, no lint. Markdown only.
- **Code layer** (`rust-wallet-app/`): standard Rust tooling — `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo geiger`.
- **Filenames:** `YYYY-MM-DD-<topic>.md`; ADRs `YYYY-MM-DD-adr-NNNN-<title>.md` (NNNN zero-padded, monotonic).
- **Cross-SDK comparison tables:** one index + per-area reports, column per SDK.
- **Use case coverage matrices:** link each user story to the SDK primitive that fulfils it.
- **ADRs:** capture decision + rejected alternatives, not just the chosen path.
- **Plan review before commit:** re-read `docs/superpowers/plans/*.md`; flag drift.
- **Research methodology:** parallel Agent subagents (one per area) + exa/firecrawl MCP web sources. Each finding cites its source.
- **No invented content:** every claim links back to a source file or external URL.
