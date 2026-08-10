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

Project-local vocabulary and threat-model hard rules live in [`rust-wallet-app/CONTEXT.md`](rust-wallet-app/CONTEXT.md). Read before code work.

## Agent skills

- **Tracker:** GitHub via `gh` — `docs/agents/issue-tracker.md`
- **Triage labels:** canonical — `docs/agents/triage-labels.md`
- **Domain docs:** single-context, glossary at `rust-wallet-app/CONTEXT.md` — `docs/agents/domain.md`

## Operational rules

**Single source of truth: [`tasks/lessons.md`](tasks/lessons.md)** — L1 through L17 cover all workflow rules (workspace path consistency, config validation, never-auto-commit, pause-before-state-modifying, issue-checkbox flip, threat model, skill enumeration, code-review-before-verify, per-task pipeline spec, etc.). Read at every task pickup per L11.

MEMORY.md (auto-loaded via SessionStart hook) holds the same durable rules for cross-project consistency; the project-specific form lives in lessons.md.

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

## Implementation workflow (rust-bitcoin-wallet v0.1)

**Direction (v0.1):** Bitcoin-only MVP. Build `rust-wallet-app/crates/bitcoin-wallet-core/` (library) + `rust-wallet-app/crates/btc/` (CLI) per merged plan. The umbrella `rust-wallet-app/` workspace already exists (scaffolded). `chain-traits/` (umbrella trait) exists; future v0.2 expands it for ETH/SOL.

**Active plan:** [`docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`](docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md) — review-cleaned canonical plan (50 doc-review findings applied, MVP scope). 10 tasks: Task 0 (threat model) + Tasks 1-9 (scaffold + crypto + wallet).

**Reference specs:**

- [`docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-design.md`](docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-design.md) — Bitcoin v0.1 design
- [`docs/superpowers/specs/2026-08-06-rust-bitcoin-wallet-architecture.md`](docs/superpowers/specs/2026-08-06-rust-bitcoin-wallet-architecture.md) — Bitcoin v0.1 architecture
- [`docs/superpowers/specs/2026-08-06-rust-wallet-app-architecture.md`](docs/superpowers/specs/2026-08-06-rust-wallet-app-architecture.md) — v0.2 multi-chain umbrella (future)

**Pipeline:**

1. **Bulk-create GitHub issues first.** One issue per umbrella task.
   Title = `Task N: <name>`. Body = plan steps as checkbox list.
   Labels = `task`, `priority/p0|p1|p2`, `week/N`. Milestone = week.
   Use `gh` CLI. Verify `gh auth status` before bulk create.
2. **Per-task loop** (consolidated — see `## Per-task loop` below).
3. **PR granularity:** weekly batched (one PR per week, accumulates all tasks for that week).
4. **Commit gate:** combined commit + push + PR into single approval pause.
5. **Issue close:** close issue after PR merge. Update spec + plan if
   implementation drifted.

### Per-task loop

**Per-task pipeline spec lives in [`tasks/lessons.md` L13](tasks/lessons.md#l13--per-task-pipeline-spec-10-decisions-2026-08-07-grill)** — single source of truth for the pipeline, complexity tier, off-rails recovery, and 10-decision rationale. CLAUDE.md intentionally does not duplicate it. Operational rules (L6/L7/L8) auto-load from `~/.claude/projects/-home-nhitran-Projects-blockchain-sdk/memory/MEMORY.md` via SessionStart hook; see also [`tasks/lessons.md`](tasks/lessons.md) for the project-specific form.

### Plugins / skills

Plugin **categories** (intent) live here; exact plugin IDs change between sessions. Resolve at runtime via `Skill(skill="<type>", args="...")` or `Agent(subagent_type="...", ...)`. Fall back to inline equivalent if a plugin is missing (ask user first).

- Behavioural anchor (karpathy-guidelines)
- TDD
- Cargo (build / test / review)
- Security (review / audit)
- Verification (before completion)
- Commit / PR (commit, commit-push-pr, code-review, doc-review)
- Branch (finishing a development branch)
- Matt Pocock (grill, triage, to-spec, to-tickets, wayfinder)

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
