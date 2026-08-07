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

**Single source of truth: [`tasks/lessons.md`](tasks/lessons.md)** — L1 through L13 cover all workflow rules (workspace path consistency, config validation, never-auto-commit, pause-before-state-modifying, issue-checkbox flip, threat model, skill enumeration, code-review-before-verify, per-task pipeline spec, etc.). Read at every task pickup per L11.

MEMORY.md (auto-loaded via SessionStart hook) holds the same durable rules for cross-project consistency; the project-specific form lives in lessons.md.

Current execution target: v0.1 `bitcoin-wallet-core` library inside `rust-wallet-app/crates/` per active plan.

## Task display rule

Whenever tasks are shown or tracked in this project — including the
internal TodoWrite list, status updates, pipeline summaries, and any
ad-hoc enumeration of work items — use the **pipeline format** below.
This format matches the project's task-display convention (see the
pipeline screenshot: header, branch, status, and per-task icons with
durations).

### Required workflow order

Every work pipeline must use these stages in this exact order unless the
user explicitly overrides it:

1. **Intent**
2. **Rebase**
3. **Review**
4. **Test**
5. **Document**
6. **Lint**

### Layout

```text
Pipeline
<branch-or-context>                              <status>
  ✓ Task A   12.3s
  ✓ Task B    2.4s
  ⋮ Task C    4.1s         <progress info>
  ○ Task D
  ○ Task E
```

### Required visual states

Every task must carry exactly one of these three status icons:

| Icon | Meaning     | When to use                                    | Shows duration? |
| ---- | ----------- | ---------------------------------------------- | --------------- |
| `✓`  | completed   | The task has finished and succeeded.           | Yes             |
| `⋮`  | in-progress | The task is currently running (one at a time). | Yes             |
| `○`  | pending     | The task has not started yet.                  | No              |

### Rules

1. **One in-progress at a time.** Exactly zero or one task may carry
   the `⋮` icon at any moment. Mark the previous task `✓` or back to
   `○` before promoting the next task to `⋮`.
2. **Duration.** Record the elapsed time next to a task as soon as it
   finishes (`✓`) or while it is running (`⋮`). Pending tasks (`○`)
   carry no duration.
3. **Header.** When presenting more than one task as a pipeline, prefix
   the block with a header line (`Pipeline`) and a context line that
   names the branch / workspace on the left and the overall status
   (`running`, `success`, `failed`) on the right.
4. **Progress detail.** When a task has a partial counter (e.g. "0 of 1
   fixes applied"), append it to the right of that task's row, not as
   a separate line.
5. **No invented states.** Do not introduce a fourth icon (✗, ⚠, ✕, …)
   for failures or skips; if a task fails, replace its icon with `○`
   and add a short note in the same row, prefixed with `failed:`.

### Examples

**Planning an implementation** (TodoWrite-style) — use a feature-branch
name on the left so the example matches how real work moves:

```text
Pipeline
fix/left-rail-master-toggle                running
  ✓ Intent     1.2s
  ✓ Rebase     2.4s
  ⋮ Review     2.1s
  ○ Test
  ○ Document
  ○ Lint
```

**Reporting progress to the user** — when the pipeline finishes,
retitle the status from `running` to `success` or `failed`; a stale
`running` header is a bug in the report, not a stylistic choice.

```text
Pipeline
fix/left-rail-master-toggle             success
  ✓ Intent       1.2s
  ✓ Rebase       2.4s
  ✓ Review       5.6s
  ✓ Test         6.3s
  ✓ Document     2.1s
  ✓ Lint         4.0s
```

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

**Audit archive:**

- [`docs/superpowers/reviews/2026-08-05-rust-bitcoin-wallet.md`](docs/superpowers/reviews/2026-08-05-rust-bitcoin-wallet.md) — 50 doc-review findings audit
- [`docs/superpowers/reviews/2026-08-06-rust-bitcoin-wallet-clean.md`](docs/superpowers/reviews/2026-08-06-rust-bitcoin-wallet-clean.md) — pre-merge snapshot (kept for diff)

**Research:** [`docs/blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md`](docs/blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md) (commit `0c20f77`) — reference Swift implementation, no project affiliation.

**Workflow:**

1. **Bulk-create GitHub issues first.** One issue per umbrella task.
   Title = `Task N: <name>`. Body = plan steps as checkbox list.
   Labels = `task`, `priority/p0|p1|p2`, `week/N`. Milestone = week.
   Use `gh` CLI. Verify `gh auth status` before bulk create.
2. **Per-task loop** (consolidated — see "Per-task loop" section below).
3. **PR granularity:** weekly batched (one PR per week, accumulates all tasks for that week).
4. **Commit gate:** combined commit + push + PR into single approval pause.
5. **Issue close:** close issue after PR merge. Update spec + plan if
   implementation drifted.

### Per-task loop

**Per-task pipeline spec lives in [`tasks/lessons.md` L13](tasks/lessons.md#l13--per-task-pipeline-spec-10-decisions-2026-08-07-grill)** — single source of truth for the pipeline, complexity tier, off-rails recovery, and 10-decision rationale. CLAUDE.md intentionally does not duplicate it. Operational rules (L6/L7/L8) auto-load from `~/.claude/projects/-home-nhitran-Projects-blockchain-sdk/memory/MEMORY.md` via SessionStart hook; see also [`tasks/lessons.md`](tasks/lessons.md) for the project-specific form.

### Plugins / skills

Plugin **types** (intent) live here; exact plugin IDs change between sessions. Resolve at runtime via `Skill(skill="<type>", args="...")` or `Agent(subagent_type="...", ...)`. Fall back to inline equivalent if a plugin is missing (ask user first).

- karpathy-guidelines (behavioral anchor)
- TDD: `superpowers:test-driven-development` / `ecc:tdd-workflow`
- Cargo: `ecc:rust-build`, `ecc:rust-test`, `ecc:rust-review`
- Security: `ecc:security-review`, `compass:security-auditor`
- Verification: `superpowers:verification-before-completion`
- Commit/PR: `commit-commands:commit`, `commit-commands:commit-push-pr`, `pr-review-toolkit:code-review`, `compound-engineering:ce-doc-review`
- Branch: `superpowers:finishing-a-development-branch`
- Matt Pocock: `grill-with-docs`, `triage`, `to-spec`, `to-tickets`, `wayfinder` (router: `ask-matt`)

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

### Commit verification pipeline (per task)

1. TDD cycle produces red→green tests
2. `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test` (combined)
3. **verification-gate** — forces real cargo output before any "done" claim (current plugin: `superpowers:verification-before-completion`)
4. **cargo-deny** — license + advisory + bans check (requires `deny.toml` at workspace root)
5. **cargo-audit** — security CVE check (requires `Cargo.lock`)
6. **PAUSE for user commit approval** (per `never-auto-commit` rule)
7. **commit-push-pr** — combined commit + push + open PR (current plugin: `commit-commands:commit-push-pr`)
8. **pr-review** — full diff audit (current plugin: `pr-review-toolkit:code-review`)
9. Fix loop max 3 rounds
10. Merge + close issue + update ledger
11. Drift update (if implementation differs from plan) — update plan + spec in same PR

**Note:** Geiger (unsafe audit) is disabled for MVP — `rust-wallet-app/` is a virtual manifest. Re-add when codebase grows beyond MVP scope or when bitcoin-wallet-core/ lands.

### Ledger

Per SDD skill: track progress in `.superpowers/sdd/<plan>/progress.md`. Update on:
- Pick up (task start)
- Commit (task progress)
- Merge (task complete)

Ledger survives compaction — trust it over session memory.

### Project scaffolding (current v0.1)

v0.1 deliverables per the merged plan: `rust-wallet-app/crates/bitcoin-wallet-core/` (library) + `rust-wallet-app/crates/btc/` (CLI), inside the umbrella `rust-wallet-app/` workspace. `chain-traits/` exists as the v0.2 umbrella scaffold.
