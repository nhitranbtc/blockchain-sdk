# task-map-lesson.md

How to map executable tasks from a plan file. Companion to `plan-lesson.md` (planning) and `review-lesson.md` (review).

Scope: extracting per-task inputs (files, tests, dependencies, acceptance) from `docs/superpowers/plans/*.md` so subagent executors (PL9 SDD) can run tasks in isolation. Excludes plan authoring (PL1–PL16, PL19, PL20) and code review (PL4–PL6, PL17).

Read at session start when picking up plan execution or running SDD on a multi-task plan.

---

## Index

- [TM1] What is a task map — definition + purpose
- [TM2] Step 1: Identify task boundaries (Task 0, Task 1, etc.)
- [TM3] Step 2: Extract per-task metadata (files / tests / deps / acceptance)
- [TM4] Step 3: Order tasks by dependency (DAG, not list)
- [TM5] Step 4: Filter drift before mapping (PL1, PL2, L30)
- [TM6] Step 5: Hand off to subagent (per PL9 SDD)
- [TM7] Mapping pitfalls — what NOT to do

---

## TM1 — What is a task map

**Definition:** A task map is a structured extract of executable work from a plan file. Each task becomes a self-contained input that a subagent (or human) can pick up without reading the full plan.

**Purpose:**

1. **SDD input (PL9):** subagent-driven development executors read task maps, not full plans. Fresh context per task; no plan-wide context pollution.
2. **Parallelism:** DAG ordering reveals which tasks can run concurrently vs. sequentially.
3. **Review checkpoints:** each task = one L12 review boundary, one commit, one PR slice.
4. **Progress tracking:** map = checklist. Update on commit/merge/grill per L14.
5. **Drift detection:** if a task map doesn't match the current plan, the plan has drifted (PL1, L30).

**Format (recommended):**

```markdown
# Task map: <plan-name>

Source: docs/superpowers/plans/YYYY-MM-DD-<topic>.md
Generated: YYYY-MM-DD
Tasks: N
Mode: SDD | inline

| # | Task | Files (create) | Files (modify) | Tests | Deps | Acceptance | Status |
|---|------|---------------|---------------|-------|------|------------|--------|
| 0 | Threat model | docs/.../threat-model.md | — | — | — | File exists with 7 sections | done |
| 1 | Workspace scaffold | Cargo.toml, src/lib.rs | — | tests/build_workspace.rs | — | cargo build + cargo test green | pending |
| 2 | Error enum | src/error.rs | src/lib.rs | (inline #[cfg(test)]) | 1 | 12 variants + From impls | pending |
| 3 | keys::mnemonic | src/keys/mnemonic.rs | src/keys/mod.rs | (inline #[cfg(test)]) | 1, 2 | 5-word-count support + 64-byte seed | pending |
```

**Why structured:** subagent can read 1 row, not 1 plan. Saves context. Forces explicit acceptance per task.

---

## TM2 — Step 1: Identify task boundaries

**Rule:** A task boundary is wherever the plan has `### Task N:` or `### Task N.x:` headings. Sub-tasks (Task 1.5, Task 0a) count as separate tasks. Don't invent boundaries the plan doesn't show.

**How to apply:**

1. Open the plan file.
2. `grep -nE "^### Task " <plan-path>` — lists all task headings.
3. For each heading, note: title, sub-tasks, position in document.
4. Skip sections that aren't tasks: "Phase 2 Backlog", "Self-Review", "Execution Handoff" — these are metadata, not work items.

**Output:** ordered list of task identifiers. Example: `[0, 0a, 1, 1.5, 2, 3, 4, 5, 6, 7, 8, 9]`.

**Anti-patterns:**
- Inventing task boundaries (e.g., "Task 0c: clean up cargo cache"). Plan didn't ask. Add via amendment, not invention.
- Bundling sub-tasks into the parent (e.g., "Task 1 covers Task 1.5 too"). Sub-tasks exist for a reason; respect them.
- Treating the Phase 2 Backlog as in-scope tasks. Per plan author intent, they're deferred.

---

## TM3 — Step 2: Extract per-task metadata

**Rule:** For each task, extract 6 fields: files-create, files-modify, tests, dependencies, acceptance, status. Without all 6, the subagent will guess and guess wrong.

**Field-by-field:**

| Field | Source in plan | Why needed |
|---|---|---|
| Files (create) | `**Files:**` line under each task | Subagent creates these verbatim |
| Files (modify) | `**Files:**` line under each task | Subagent edits these verbatim |
| Tests | Step bodies with `#[test]`, `#[tokio::test]`, or "Write failing test" | Subagent runs TDD cycle |
| Deps | Order in plan + import statements | DAG for parallel execution |
| Acceptance | Last "Run:" / "Expected:" line, or "Pause for commit" step | L13 step 14 evidence for `[x]` flip |
| Status | not in plan — initial = `pending` | Tracks per-task progress |

**How to apply (3 sub-steps per task):**

1. Read the task section. Pull files list. Pull test names (any test function or `#[test]`).
2. Find what imports/uses the task's output. If Task 3 imports Task 2's `Error` enum, then 3 depends on 2.
3. Find the last "Run" / "Expected" / "pause" line. That = acceptance.

**Worked example (from `2026-08-05-rust-bitcoin-wallet.md` Task 5):**

| Field | Value |
|---|---|
| Files (create) | `src/crypto/argon2.rs`, `src/crypto/aes_gcm.rs`, `src/crypto/mod.rs` |
| Files (modify) | — |
| Tests | `derive_key_produces_32_bytes`, `derive_key_deterministic_for_same_inputs`, `derive_key_different_salt_yields_different_key`, `roundtrip`, `wrong_key_fails` |
| Deps | Task 2 (uses `Error::Encryption` variant) |
| Acceptance | "Run tests, pause for commit" |

**Anti-patterns:**
- Omitting acceptance. Without it, L13 step 14 can't flip `[ ]` to `[x]`.
- Omitting deps. Subagent might run Task 3 before Task 2 → compile failure.
- Listing only one of (create / modify). Subagent misses the other half.

---

## TM4 — Step 3: Order tasks by dependency (DAG, not list)

**Rule:** Plan order is hint, not truth. Build a DAG (directed acyclic graph) by following imports + file references. Cycles = bug in the plan or the map.

**How to apply (4 sub-steps):**

1. List all files each task creates / modifies.
2. For each (task A, file F), find tasks that reference F after A. Those depend on A.
3. Topological sort the DAG. Verify no cycles.
4. Annotate the task map with `Deps` column = set of prerequisite task IDs.

**Worked example (rust-bitcoin-wallet v0.1 plan):**

```
Task 0  (threat model)     → no deps
Task 0a (threat model doc) → no deps
Task 1  (workspace)        → no deps
Task 1.5 (Secret + atomic)  → Task 1
Task 2  (Error enum)       → Task 1
Task 3  (mnemonic)         → Task 1, Task 2
Task 4  (derivation + sign) → Task 1, Task 2, Task 3
Task 5  (argon2 + aes)     → Task 1, Task 2
Task 6  (bip137)           → Task 4, Task 5
Task 7  (WalletConfig)     → Task 1, Task 2
Task 8  (network helper)   → Task 1
Task 9  (Wallet)           → Task 1, Task 2, Task 3, Task 4, Task 5, Task 7, Task 8
```

**Parallel execution opportunities:**

- Tasks 0, 0a, 1: parallel (no deps between them).
- Tasks 2, 7, 8: parallel (all depend only on 1, no deps between them).
- Tasks 3, 5: parallel after Task 2.
- Task 4: sequential after Task 3.
- Task 6: sequential after Task 4 + Task 5.
- Task 9: sequential at the end.

**Anti-patterns:**
- Trusting plan order without verifying. Plans are written top-to-bottom but execution can be different.
- Missing an indirect dep (Task X uses Task Y's output but Y doesn't appear in X's files list). Cross-check imports.
- Allowing cycles. If A depends on B and B depends on A, the plan is broken — escalate to user, don't silently drop a dep.

---

## TM5 — Step 4: Filter drift before mapping (PL1, PL2, L30)

**Trigger:** Task map extraction is the wrong time to discover the plan is stale. Filter drift first.

**Rule:** Before generating the task map, run PL14 drift scan. If any drift, fix plan first (per PL1 — plan is the bug), then re-extract.

**How to apply (4 drift checks from PL14):**

1. **L30 SHA scan:** for every SHA / commit ref in plan header, `git log --all -- <cited-path>`. Empty = drift.
2. **PL1 content scan:** for every code block (file:line) in plan, `cat` the file on `main`. Mismatch = drift.
3. **PL2 dep scan:** for every dep list, diff against workspace + crate manifests. Mismatch = drift.
4. **PL3 story trace:** for every story in coverage matrix, locate the task and confirm CLI subcommand exists in task body.

If any check fails: **fix the plan (PL1), not the code.** Then re-run drift scan. Then proceed to TM2.

**Why:** Generating a task map from a drifted plan = systematic wrong work. Subagent executes against a stale blueprint.

**Anti-pattern:** Generating the task map "and we'll fix drift later." Drift compounds. Fix at source, then map.

---

## TM6 — Step 5: Hand off to subagent (per PL9 SDD)

**Trigger:** Task map generated, drift clean, deps ordered.

**Rule:** For SDD (≥5-task plans per PL9), each subagent gets:
1. Plan path (read full plan for context, not just the row).
2. Task number (e.g., "Run Task 3").
3. Task map row (the 6-field extract for THIS task).
4. L13 step 4a instruction (drift scan, but we already did TM5 — subagent re-verifies the task's specific files).
5. Acceptance criteria from the row.

**Subagent prompt template:**

```text
You are running Task N of plan <plan-name>.

Read: docs/superpowers/plans/YYYY-MM-DD-<topic>.md (full plan for context).

Your task:
- Title: <task title>
- Files to create: <list>
- Files to modify: <list>
- Tests: <test names>
- Dependencies: <task IDs that must be done first>
- Acceptance: <last "Run" / "Expected" line>

Process (per L13 + TDD):
1. Drift scan files in this task (git log -1 main -- <path>).
2. Write failing test (TDD red).
3. Implement minimal code (TDD green).
4. Refactor.
5. Run local verify gate: cargo fmt + clippy + test.
6. L12 review before commit (pr-review-toolkit:code-reviewer).
7. PAUSE for commit approval (PL19 gate). Do NOT run git commit.
8. Report: test results, file list, anything that surprised you.
```

**Why this template:**
- Plan path = subagent gets the full context (not just the row) for cross-references.
- Task map row = subagent's specific work item.
- L13 step 4a re-instruction = catches drift introduced between TM5 and now.
- PAUSE for commit = PL19 gate. Subagent must not auto-commit.

**Anti-patterns:**
- Sending subagent only the task map row. Loses plan context.
- Sending subagent the full plan without the task map row. Subagent picks the wrong task.
- Skipping the PAUSE-for-commit instruction. Subagent may auto-commit (violates PL19).
- Reviewing the subagent's diff yourself instead of using `pr-review-toolkit:code-reviewer`. Loses parallel review.

---

## TM7 — Mapping pitfalls

| Pitfall | Symptom | Fix |
|---|---|---|
| Task 0a vs Task 0 confusion | Subagent runs wrong task | Use the exact heading from the plan as task ID |
| "Phase 2" tasks mapped | Subagent runs deferred work | Filter by status = in-scope only |
| Missing acceptance | Can't flip `[ ]` to `[x]` per L13 step 14 | TM3 must include acceptance for every task |
| Cycle in DAG | Impossible to schedule | Escalate; cycles are plan bugs |
| Plan uses variable names not in current code | Drift from PL1 | Re-run TM5, fix plan, re-extract |
| Subagent receives wrong file list | Compiles but doesn't match plan acceptance | TM3 must list ALL files (create + modify) |
| Acceptance criteria too vague | Subagent ships "done" but acceptance fails | TM3 acceptance = last "Run:" / "Expected:" line, verbatim |
| Subagent doesn't pause for commit | Auto-commit violates PL19 | Template must include "PAUSE for commit" step |
| Drift scan skipped | Plan was 6 PRs stale, subagent built on old code | TM5 mandatory before TM6 |

**Anti-patterns (top 3 to watch):**

1. **Skipping TM5 drift scan.** Drift at this stage = systematic wrong work.
2. **Omitting acceptance in TM3.** L13 step 14 fails silently.
3. **Subagent prompt missing PAUSE-for-commit.** PL19 violation.

---

## Self-Review

1. **Scope:** All 7 lessons (TM1–TM7) apply to extracting per-task inputs from a plan for subagent execution.
2. **Removed from this file:** PL1–PL20 (planning, in `plan-lesson.md`), PL4–PL6, PL17 (review, in `review-lesson.md`), PL18–PL20 (search, in `search-lesson.md`).
3. **Companion mapping:**
   - TM2–TM3 = plan → task map extraction (this file).
   - TM4 = DAG ordering (enables PL9 SDD parallel execution).
   - TM5 = drift filter (extends PL14 to task-map stage).
   - TM6 = subagent handoff (PL19 gate + L13 step 4a).
   - TM7 = pitfalls (anti-pattern guard).
4. **Anti-pattern audit:** Each lesson names the failure mode it prevents.

---

## Update protocol

When a task-map extraction surfaces a reusable lesson:

1. **Draft** new TM entry in this file (terse: trigger → rule → why → how to apply → anti-patterns).
2. **Add** to Index above.
3. **Cross-link** to related `plan-lesson.md` (PL9, PL14) and `lessons.md` (L13, L14, L28) entries.
4. **Commit** on its own PR titled `docs(task-map-lesson): TM<N> <short title>`.
5. **Wire** into SDD handoff template.

Do NOT add to `lessons.md` (corrections ledger), `plan-lesson.md` (planning-only), or `review-lesson.md` (code review-only).