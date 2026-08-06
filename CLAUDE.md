# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working
with code in this repository.

## What this is

Markdown research + analysis workspace for blockchain SDK investigations.
Focus: Bitcoin wallet SDKs (rust-bitcoin, BDK), Lightning,
stablecoin integration, wallet security comparisons.

Two coexisting layers:
- **Docs/research layer** (`docs/`) — pure prose, markdown only
- **Code layer** (`rust-wallet-app/` for v0.2 umbrella, `bitcoin-wallet-rs/` for v0.1) — Rust workspace, git-tracked, executable

Current execution target: v0.1 `bitcoin-wallet-rs/` per active plan.

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

- **Docs/research layer only:** no build, no tests, no lint on `docs/`. All output is markdown.
- **Code layer** (`rust-wallet-app/`, `bitcoin-wallet-rs/`): standard Rust tooling — `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo geiger`.
- **Filename pattern:** `YYYY-MM-DD-<topic>.md`. ADRs use
  `YYYY-MM-DD-adr-NNNN-<title>.md` with NNNN zero-padded and monotonic.
- **Cross-SDK comparison tables.** Each feature audit produces one
  index file plus per-area reports. Tables include explicit column per
  SDK (`rust-bitcoin`, `BDK`, …) so gaps are visible.
- **Use case coverage matrices** link each user story to the SDK
  primitive that fulfils it. Audit for missing rows whenever the
  source library changes.
- **ADRs capture decision + rejected alternatives**, not just the
  chosen path.
- **Plan review before commit.** `docs/superpowers/plans/*.md` describe
  implementation intent. Re-read before each commit and flag drift
  between plan and current docs.
- **Research methodology:** parallel Agent subagents (one per area) +
  exa/firecrawl MCP web sources. Available tools:
  `mcp__exa__web_search_exa`, `mcp__exa__web_fetch_exa`,
  `mcp__firecrawl__firecrawl_search`, `mcp__firecrawl__firecrawl_scrape`,
  `mcp__firecrawl__firecrawl_extract`, `mcp__firecrawl__firecrawl_crawl`,
  `mcp__firecrawl__firecrawl_map`, `mcp__firecrawl__firecrawl_deep_research`.
  Each finding cites its source.
- **No invented content.** Every claim links back to a source file or
  external URL.

## Implementation workflow (rust-bitcoin-wallet v0.1)

**Direction (v0.1):** Bitcoin-only MVP. Build `bitcoin-wallet-core` Rust library + `btc` CLI per merged plan. Future v0.2 adds multi-chain umbrella (`rust-wallet-app/`) consuming v0.1 as cargo path dep.

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

### Per-task loop (10 steps)

```text
Pipeline
task/N-<short-name>                       running
  ✓ Pick up issue
  ✓ karpathy-guidelines + branch checkout
  ⋮ TDD cycle (red test → green impl → refactor)
  ⋮ cargo fmt + clippy + test + verify
  ○ PAUSE — approval for commit + push + PR
  ○ commit-commands:commit-push-pr
  ○ PR review + merge + close issue
```

**Step details:**

1. **Pick up issue** — `gh issue view <N>` reads body + acceptance criteria.
2. **`andrej-karpathy-skills:karpathy-guidelines` + branch checkout** — invoke karpathy-guidelines once per task (load behavioral guidelines), then `git checkout task/N-<name>`; create from main if missing: `git checkout -b task/N-<name> main`. Pull + rebase to ensure branch starts from latest main.
3. **TDD cycle** — write failing test (plan Step 1), run (expect fail), implement (plan Step 2), run (expect pass), refactor (plan Step 3+).
4. **Verify** — `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --workspace`. Wrap with `superpowers:verification-before-completion` for the "done" claim gate.
5. **PAUSE** — report diff summary + test output. Ask: "commit + push + PR?" per `~/.claude/.../memory/MEMORY.md` rules.
6. **Commit + push + PR** — `commit-commands:commit-push-pr` (combined commit + push + open PR). `pr-review-toolkit:code-review` audits full diff. Fix loop max 3 rounds.
7. **Pre-merge checklist** (per user rule "update completed steps before merge"):
   - [ ] Update issue body checkboxes via `gh issue edit N --body "..."` — flip all completed steps from `[ ]` to `[x]`
   - [ ] PR review via `pr-review-toolkit` (or inline fallback if skill unavailable)
   - [ ] Merge via `gh pr merge N --squash --delete-branch`
   - [ ] Close issue via `gh issue close N` (or auto on merge)
   - [ ] Update ledger `.superpowers/sdd/<plan>/progress.md` with commit SHA + PR number + merge state
8. **Drift update** (if implementation differs from plan) — update plan + spec in same PR. Per CLAUDE.md "Update spec + plan if implementation drifted."

### Plugins / skills (apply in order per task)

CLAUDE.md lists **plugin types** (intent), not exact plugin IDs. Plugin IDs change between sessions — resolve at runtime via `Skill` tool.

- **karpathy-guidelines** — behavioral guidelines for code work (loaded once per task)
- **TDD-skill** — red test first (any superpowers:* or ecc:* skill providing TDD enforcement)
- **cargo-error-fix** — Cargo / compiler errors (current name: `ecc:rust-build`)
- **test-design** — test design + execution (current name: `ecc:rust-test`)
- **rust-review** — ownership / `unsafe` / crypto / multi-crate review (current name: `ecc:rust-review`)
- **security-review** — secret storage / signing / cross-chain trust, Tasks 5, 6, 9 (current name: `ecc:security-review`; deeper: `compass:security-auditor`)
- **verification-gate** — gate on real cargo output before "done" claim (current name: `superpowers:verification-before-completion`)
- **commit** — single commit with pre-commit hooks (current name: `commit-commands:commit`)
- **commit-push-pr** — combined commit + push + open PR (current name: `commit-commands:commit-push-pr`)
- **pr-review** — full diff audit (current name: `pr-review-toolkit:code-review`)
- **doc-review** — reconcile spec/plan contradictions if drift detected (current name: `compound-engineering:ce-doc-review`)
- **finish-branch** — at week end (current name: `superpowers:finishing-a-development-branch`)

### How to resolve plugin IDs at runtime

- Use `Skill` tool with description match: `Skill(skill="<type>", args="...")`
- Use `Agent` tool with `subagent_type` for subagents: `Agent(subagent_type="compass:rust-engineer", ...)`
- If plugin not found, fall back to inline equivalent (ask user first)

### Session-start rule

Every session begins by stating the verifiable goal of the session in one sentence. If unclear, ask before acting. Per karpathy-guidelines §1 "Think Before Coding".

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
4. **PAUSE for user commit approval** (per `never-auto-commit` rule)
5. **commit-push-pr** — combined commit + push + open PR (current plugin: `commit-commands:commit-push-pr`)
6. **pr-review** — full diff audit (current plugin: `pr-review-toolkit:code-review`)
7. Fix loop max 3 rounds
8. Merge + close issue + update ledger
9. Drift update (if implementation differs from plan) — update plan + spec in same PR

### Ledger

Per SDD skill: track progress in `.superpowers/sdd/<plan>/progress.md`. Update on:
- Pick up (task start)
- Commit (task progress)
- Merge (task complete)

Ledger survives compaction — trust it over session memory.

### Project scaffolding (current v0.1)

```text
rust-wallet-app/                    (workspace root)
├── Cargo.toml                      (workspace)
├── crates/
│   ├── chain-traits/               (ChainWallet trait — umbrella, exists)
│   ├── bitcoin-wallet-core/        (library — BDK 3.1 + rust-bitcoin 0.32, v0.1 to build)
│   └── btc/                        (CLI — clap 4, v0.1 to build)
└── .github/workflows/ci.yml
```

`bitcoin-wallet-core/` and `btc/` are v0.1 deliverables per the merged plan
(`docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`). They live INSIDE
the umbrella workspace `rust-wallet-app/`.
