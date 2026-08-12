# plan-lesson.md

Planning-phase discipline playbook. Distinct from `lessons.md` (corrections ledger = post-mortem). This file = preventive = what to verify, check, and resolve BEFORE picking up an issue or starting a task.

Read at session start when picking up issue work, plan edits, or drift audits.

Scope: planning only. Review, code-block, content, and deep-search plugin references live in companion files:
- Review lessons (PL4, PL5, PL6, PL17) → `tasks/review-lesson.md`
- Content / code-block / deep-search lessons (PL18, PL19, PL20) → `tasks/search-lesson.md`

---

## Index

- [PL1] Plan docs are living specs — sync to `main` before treating as truth
- [PL2] Drift scan plan-cited deps, files, and SHAs before fixing "missing" code
- [PL3] Story coverage matrix must match CLI surface in same plan
- [PL7] Plugin stack for plan authoring — `brainstorming` + `writing-plans` + executor
- [PL8] Matt Pocock / Daniel Stern / Carlos Arguelles pattern: plugin-first SDK design
- [PL9] Subagent-driven development (SDD) is the default executor for ≥5-task plans
- [PL10] Step 1 — Idea capture: log the verbatim ask before brainstorming
- [PL11] Step 2 — Brainstorming skill: hard gate, never skip
- [PL12] Step 3 — Design spec: write to `docs/superpowers/specs/`, run self-review, get user sign-off
- [PL13] Step 4 — Writing-plans skill: spec → step-by-step plan with TDD steps
- [PL14] Step 5 — Plan review + drift scan (PL1, PL2, L30) before commit
- [PL15] Step 6 — Executor pick (PL9), pickup, run, merge — close the loop
- [PL16] Plugins for PLANNING (use during plan authoring)
- [PL19] Approval-before-execute gate — pause for "approved" before any state-modifying action
- [PL20] Tier 2 planning plugins — heavier ceremony for greenfield / multi-agent projects

---

## PL1 — Plan docs are living specs

**Trigger:** Session 2026-08-12, PR review of `2026-08-05-rust-bitcoin-wallet.md`. Plan claimed `bitcoin-wallet-core/Cargo.toml` listed 14 deps; actual file listed 27. Plan was 6 PRs stale relative to `main`.

**Rule:** When reviewing or referencing a plan/spec file, treat it as a **living document**. Before quoting a code block, dependency list, or file path from a plan, verify against `main` with `git log --all -- <path>` and `cat` of current file. Plans drift silently — no CI catches a stale code block.

**Why:** Plans live on disk for months. Code evolves via PRs that don't always update the plan. Reading the plan first (vs reading the code first) primes you with stale context. Fixing the code to match the plan = bug. Fixing the plan to match the code = correct.

**How to apply:**
- Before picking up plan-cited work: `git log --all -- <plan-path>` to check freshness.
- Before quoting plan content: cross-check against actual files on `main`.
- Plans describe intent; code describes reality. When they conflict, **the plan is the bug**, not the code.

**Anti-patterns:**
- "Plan says X, so code must be wrong" — flip the assumption.
- Treating plan files as immutable specs.
- Skipping plan updates on PRs that touch dependencies, file structure, or task scope.

**Companion:** L30 verifies plan-cited SHAs; PL1 verifies plan-cited file CONTENT.

---

## PL2 — Drift scan plan-cited deps, files, and SHAs before fixing "missing" code

**Trigger:** Same session as PL1. Reviewer proposed adding `bitcoin` and `bip39` to Cargo.toml. `git log` showed both deps present since PR #69. The "missing" deps were a plan-doc bug, not a code bug.

**Rule:** When a review or audit flags "X is missing", verify with `git log --all -- <file>` and `cat <file>` before editing. Three possible outcomes:
1. **Drift** — file is correct, plan is stale → fix the plan (PL1).
2. **Revert** — file was correct, recent commit broke it → revert or fix forward.
3. **Real bug** — file genuinely missing it → fix the file.

Default to outcome #1; outcome #3 is the least common.

**Why:** Code-review feedback can come from any source (human, AI, CI). Drift between reviewer context and `main` is more common than genuine missing code. Fixing code to match stale reviewer context = silent regression.

**How to apply:**
- Audit/Review step in L13: always `git log -1 main -- <path>` before first Edit.
- If review says "missing dep X" and X appears in current file: STOP. Update plan, don't touch code.
- If review says "missing dep X" and X really absent: proceed with Edit, cite the review finding.

**Anti-patterns:**
- Trusting review/audit output without ground-truth check.
- Editing code to satisfy a stale plan citation.

---

## PL3 — Story coverage matrix must match CLI surface

**Trigger:** Same session. Plan's Story Coverage Matrix claimed Stories 2 (Import wallet) and 12 (Config show) in MVP scope, but Task list ended at Task 9 = `from_mnemonic + sync + balance`. No `btc wallet import` or `btc config show` subcommand exists.

**Rule:** When a plan claims story X is in scope, the same plan MUST contain a task that produces the CLI subcommand (or library API) fulfilling that story. Story matrix = status; task list = evidence. Mismatch = aspirational coverage.

**Why:** Coverage matrices are read first by stakeholders. `[x]` boxes checked against `core` label signal "we shipped this." If the underlying CLI doesn't exist, the audit trail bakes in an unfulfilled promise (L24 closed-with-`- [ ]` rule).

**How to apply:**
- At plan write time: for each story row in the matrix, point to a specific task number in the task list.
- At review time: trace every story back to its task. Empty trace = remove story from MVP or add task.
- "Library API only" stories: mark explicitly in matrix (e.g., status: `lib-only (no CLI)`) so reader doesn't assume user-facing delivery.

**Anti-patterns:**
- Marking a story "in scope" because the library can technically do it.
- Deferring CLI tasks to a vague "Phase 2" without a follow-up issue.

---

## PL7 — Plugin stack for plan authoring

**Trigger:** Session 2026-08-12 brainstorm about which superpowers plugins / skills to invoke when writing a plan for a new feature or refactor. Multiple plugins in `~/.claude/plugins/` overlap; without a routing rule, plan authors invoke the wrong skill (or skip the right one).

**Rule:** For any plan authoring task, invoke this stack in order. Do not skip steps even if the plan feels "obvious."

| Step | Plugin / skill | Purpose | When to skip |
|---|---|---|---|
| 1 | `superpowers:brainstorming` | Turn idea into design spec | Never (hard gate per skill) |
| 2 | `superpowers:writing-plans` | Spec → step-by-step plan | Never for ≥3-task work |
| 3 | `superpowers:executing-plans` OR `superpowers:subagent-driven-development` | Run the plan | Pick one (PL9) |
| 4 | `superpowers:test-driven-development` | Every task step starts with a failing test | Never (red-green-refactor) |
| 5 | `superpowers:code-reviewer` (PR review toolkit) | L12-style review BEFORE local verify | Never (L12 + L13 step 4) |
| 6 | `superpowers:requesting-code-review` (superpowers:code-review skill) | Async review handoff | When no human reviewer available |
| 7 | `superpowers:systematic-debugging` | When a step fails | Only when step fails |
| 8 | `superpowers:verification-before-completion` | Before claiming done | Never (L28) |

**Why:** Each plugin encodes a discipline the author would otherwise skip. `brainstorming` is the only one with an explicit hard gate ("do NOT invoke any implementation skill until design approved"). The other plugins are advisory — without a stack rule, authors cherry-pick based on mood, not discipline.

**How to apply:**
- At plan kickoff: create todos for steps 1-2 (brainstorming → writing-plans). After plan committed, todos for steps 3-8.
- Step 1 (`brainstorming`) writes `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`.
- Step 2 (`writing-plans`) reads spec → writes `docs/superpowers/plans/YYYY-MM-DD-<topic>.md`.
- Steps 4-8 belong to *each task* in the executor phase, not to plan authoring itself.

**Anti-patterns:**
- "The plan is small, skip brainstorming." Per skill hard gate: never skip.
- "Spec looks obvious, skip writing-plans." Plans with <3 tasks are fine inline; ≥3 tasks require the plan file.
- Picking executor plugin based on "what worked last time" — pick per PL9 rules.

**Companion:** L13 step 1 = skill-tag (which skills apply to THIS task). L13 step 2 = skill invocation. PL7 is the planning-phase plugin stack; L13 is the per-task skill stack.

---

## PL8 — Plugin-first SDK design (Pocock / Stern / Arguelles)

**Trigger:** Session 2026-08-12 review of `bitcoin-wallet-core` plan surfaced gaps that, if shipped, would block cross-language hosts (UniFFI Swift, NAPI Node, PyO3 Python). Authors who don't think host-first ship libraries that can't be wrapped.

**Rule:** When planning an SDK or library intended for plugin/host use, run these four design checks **at spec time**, not at integration time:

| Check | Question | Plan section it lands in |
|---|---|---|
| **Host event loop safety** | Will any `.await` or sync call block the host's event loop? | `Architecture` (mutex types, async strategy) |
| **Consumer ergonomics** | Can a host consume the API with `import` + 3-line example? | `lib.rs` (flat re-exports, see PL4 in `review-lesson.md`) |
| **Stability contract** | What breaks between minor versions? | `Stability Policy` (see PL6 in `review-lesson.md`) |
| **Type narrowness** | Can a host `match` on error variants without SemVer risk? | `Error` enum (`#[non_exhaustive]`, see PL4 in `review-lesson.md`) |

**Three reference patterns:**

1. **Matt Pocock (Total TypeScript / MCP servers):** flat `pub use` at lib root + `#[non_exhaustive]` errors + example folder per public API. Every public type should appear in an `examples/` runnable binary.
2. **Daniel Stern (Rust API design):** prefer `&str` over `String` in args, `impl AsRef<Path>` over `&Path`, `Result<T, Error>` not `Option<T>` for fallible ops. Library crates use `pub mod` not `pub use` re-exports UNLESS top-level ergonomics demand it.
3. **Carlos Arguelles (Tangem / cross-chain wallet):** "**data first, code second**." Threat model before code (Task 0a in current plan). Wire formats and serialization chosen before type definitions.

**Why:** SDK consumers (Swift, JS, Python, Go hosts) have stricter constraints than Rust callers: no `unsafe`, no panic, no blocking I/O on the main thread, no lifetime parameters in the foreign surface. Libraries written without these constraints get forked or abandoned. Designing host-first means the SDK works for the consumer the day it ships, not "after we add UniFFI."

**How to apply:**
- At spec write time: write a one-page "Host integration sketch" showing 3-5 lines of host code (Swift / TypeScript / Python) calling the SDK.
- At plan review: reject plans missing the four checks in the table above.
- For Rust-only libraries (no host integration): PL8 still applies for API ergonomics — drop the host sketch requirement.

**Anti-patterns:**
- "We'll figure out UniFFI later." You won't, and it shows.
- Library-only ergonomics that ignore host runtime constraints.
- Shipping a `0.1.0` without a stability policy (PL6).

**Companion:** PL4 (flat re-exports + `#[non_exhaustive]`), PL5 (async mutex), PL6 (stability policy) — all in `review-lesson.md`. PL8 is the umbrella rule that calls those three.

---

## PL9 — Subagent-driven development (SDD) is the default executor for ≥5-task plans

**Trigger:** Session 2026-08-12 review of `2026-08-05-rust-bitcoin-wallet.md` plan (11 tasks, 2 crates). Inline execution = one agent, single context, no review between tasks. Subagent execution = fresh context per task, review between tasks, parallelizable.

**Rule:** Pick the plan executor plugin based on plan size + review depth:

| Plan size | Executor plugin | Why |
|---|---|---|
| 1-2 tasks, no review needed | `superpowers:executing-plans` inline | Low overhead, one shot |
| 3-4 tasks, simple steps | `superpowers:executing-plans` inline OR `superpowers:test-driven-development` per task | Single context OK |
| ≥5 tasks OR cross-cutting changes | `superpowers:subagent-driven-development` | Fresh context per task, code review between, isolation |
| ≥10 tasks OR multi-crate refactor | `superpowers:subagent-driven-development` (mandatory) | Inline = context collapse |

**For the `2026-08-05-rust-bitcoin-wallet.md` plan (11 tasks, 2 crates):** `subagent-driven-development` is mandatory.

**Why:** Inline execution of multi-task plans runs into context-window exhaustion by task 5-6. Subagent-driven execution isolates each task in a fresh subagent context, returning a structured report. Review happens between tasks via `superpowers:code-reviewer`. Cost: more turns + orchestration overhead. Benefit: no context collapse, no skipped steps, audit trail per task.

**How to apply:**
- At plan write time: count tasks. If ≥5, declare executor in plan header ("Execution: SDD").
- At plan pickup: invoke `superpowers:subagent-driven-development` skill before any task.
- Each subagent gets: plan path + task number + L13 step 4a drift scan instruction.
- Review between tasks: invoke `superpowers:code-reviewer` on subagent diff before merging to working branch.

**Anti-patterns:**
- Inline-execute a 10-task plan. Context collapse by task 6.
- Skip subagent code review. Defeats the isolation benefit.
- SDD for 1-2 task plans. Overhead exceeds value.

**Companion:** L13 step 13-15 (commit → push → PR → admin-merge). SDD outputs feed the PR body table per L9.

---

## PL10 — Step 1: Idea capture

**Trigger:** User says "let's build X" / "do next session" / "pick up #N" / "add feature Y." Without verbatim capture, brainstorming rephrases the ask and intent drifts.

**Rule:** Before invoking any skill, write the user's verbatim instruction to a temp note or message. Use that as the source of truth during brainstorming.

**Why:** Between session start and the brainstorming skill's first question, rephrasing creeps in. "Build a Bitcoin wallet" might mean "replace tangem-app-ios Bitcoin module" or "add a CLI to existing core" — different scopes. Verbatim capture prevents the agent from guessing.

**How to apply (3 sub-steps):**

1. **Verbatim log:** Copy the user's instruction into a session-start note, quoted exactly. Example:
   ```
   User (2026-08-12): "write tasks/plan-lesson.md, this files add these lessons in planning progress"
   ```
2. **Scope probe:** Ask 1-2 clarifying questions before brainstorming if scope ambiguous. For "next session" / "do your recommendations" — check the issue tracker for explicit pickup signals.
3. **Handoff:** Pass verbatim log into the brainstorming skill's first message as context.

**Anti-patterns:**
- "I understood the gist" — skip verbatim. Result: agent rephrases, user re-corrects, two rounds wasted.
- Starting brainstorming immediately on a one-line instruction without scope probe. Brainstorming's first question will be "what are you building?" — already answered if you logged verbatim.

**Companion:** L11 (scan skills first) applies here too — before logging, scan `~/.claude/skills/` for an applicable skill. "Write plan-lesson.md" → no skill match → log + proceed to PL11.

---

## PL11 — Step 2: Brainstorming skill (hard gate)

**Trigger:** User confirms scope. Next decision: spec vs. jump-to-code. Brainstorming skill enforces a hard gate — no implementation action until design approved.

**Rule:** Invoke `superpowers:brainstorming` for any non-trivial feature, refactor, or new subsystem. The skill has a 9-step checklist; follow it literally:

1. **Explore project context** — check files, docs, recent commits, related specs/plans.
2. **Offer the visual companion** (just-in-time, not upfront) — only if a question would be clearer shown than told.
3. **Ask clarifying questions** — one at a time, multi-choice preferred.
4. **Propose 2-3 approaches** — with trade-offs + recommendation.
5. **Present design** — in sections, get user approval after each.
6. **Write design doc** — `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`.
7. **Spec self-review** — fix placeholders, contradictions, ambiguity, scope inline.
8. **User reviews written spec** — wait for sign-off.
9. **Transition to implementation** — invoke `writing-plans` skill.

**Why:** The hard gate exists because "this is too simple" projects cause the most wasted work. The design can be 3 sentences for a tiny feature, but the approval step forces the user and agent to agree on intent before any code lands.

**How to apply:**
- Create todos for each of the 9 steps. Mark step 1 in-progress, do it, mark done, move to next.
- For tiny features (single function, ≤3-file edit): collapse steps 4-5 to a 2-sentence design + approval. Steps 1-3 and 6-9 still apply.
- After user approves, immediately invoke `superpowers:writing-plans` (PL13). Do NOT call any other skill.

**Anti-patterns:**
- Skipping the skill because "user already explained the idea in their initial message." The skill exists to formalize that explanation into a design.
- Calling frontend-design / mcp-builder / implementation skills directly. Per skill hard gate: writing-plans is the ONLY next step after brainstorming.

**Companion:** L11 (skill-tag at pickup) — brainstorming is one of the 3-5 skills you tag at session start.

---

## PL12 — Step 3: Design spec (write + self-review + user sign-off)

**Trigger:** Brainstorming produces an approved design. Next: write the design doc to disk so it survives session boundaries and can be referenced by the plan.

**Rule:** After brainstorming approval, write the design to `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`. Then run the 4-point self-review and wait for user sign-off before invoking `writing-plans`.

**Spec self-review checklist (per brainstorming skill):**

1. **Placeholder scan:** any "TBD" / "TODO" / "implement later" / incomplete sections? Fix inline.
2. **Internal consistency:** do sections contradict each other? Does the architecture match feature descriptions?
3. **Scope check:** focused enough for one implementation plan, or needs decomposition?
4. **Ambiguity check:** could any requirement be interpreted two different ways? Pick one and make it explicit.

**How to apply (5 sub-steps):**

1. **File path:** `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md` (date = brainstorm approval date).
2. **Header:** `# Design: <topic>`. One-paragraph summary. Link to research / related specs / prior art.
3. **Sections:** Architecture, Components, Data Flow, Error Handling, Testing, Open Questions.
4. **Self-review pass:** run the 4 checks. Fix inline. No re-review needed.
5. **User sign-off:** present spec to user, wait for "approved" or change requests. Do NOT proceed without explicit approval.

**Why:** Specs that go to plan without user sign-off bake unverified assumptions into executable steps. A single misread in the design = 10 task-step reversions.

**Anti-patterns:**
- Skipping the spec file because "the chat already has the design." Chat context is ephemeral; the spec file is the durable handoff to the plan author and future subagents.
- Skipping self-review. Placeholders + contradictions surface during execution, when they're expensive.
- Proceeding to writing-plans before user reads the spec. Defeats the sign-off gate.

**Companion:** L30 (drift scan) applies AFTER spec is written — verify cited research/prior art still exists.

---

## PL13 — Step 4: Writing-plans skill (spec → step-by-step plan)

**Trigger:** User signed off on the spec. Next: turn design into executable TDD-flavored plan.

**Rule:** Invoke `superpowers:writing-plans`. The skill reads the spec + produces a plan with checkbox-tracked steps. Every task step uses TDD: failing test → implement → refactor.

**Plan file path:** `docs/superpowers/plans/YYYY-MM-DD-<topic>.md`

**How to apply (8 sub-steps):**

1. **Read the spec** (PL12 output). Don't paraphrase — link to it.
2. **Decompose into tasks:** each task = one TDD cycle (test, impl, refactor, commit).
3. **Order by dependency:** threat model (Task 0) before workspace scaffold (Task 1) before crypto (Task 5) before wallet (Task 9). Data shapes before code that uses them.
4. **Each task step:**
   - `- [ ]` checkbox.
   - Files: list create/modify.
   - Step body: write failing test → run + watch fail → implement → run + watch pass → refactor → commit pause.
5. **Test priority:** for SDK code, test the public API first, internals via `#[cfg(test)]`.
6. **Pause points:** mark "pause for commit approval" at end of each task (L6 approval gate).
7. **Self-review:** placeholder scan, type consistency, story ↔ task trace (PL3).
8. **Link in plan header:** spec, architecture, research, review audit.

**Plan header template:**

```markdown
# <Topic> Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: <executor plugin> (PL9).
>
> **Review audit:** ../reviews/YYYY-MM-DD-<topic>.md
> **Spec:** ../specs/YYYY-MM-DD-<topic>-design.md
> **Architecture:** ../specs/YYYY-MM-DD-<topic>-architecture.md
> **Research:** ../../<area>/YYYY-MM-DD-<topic>.md
```

**Why:** Plans with TDD steps + checkbox tracking + pause points = auditable executor input. Subagent-driven executors (PL9) read the plan as their only source of truth. A plan with vague steps = subagent guesses = rework.

**Anti-patterns:**
- "Plan looks long, I'll trust the spec." Plan = spec's executable form. Spec says what; plan says how and in what order.
- Skipping the failing-test step "to save time." Per TDD iron law: no production code without a failing test first.
- "Tests can be added at the end." Per L28: stub honesty gate. Every step includes the test.

**Companion:** L1 (workspace path consistency across docs + manifests), L13 (per-task pipeline), TDD skill (red-green-refactor).

---

## PL14 — Step 5: Plan review + drift scan (PL1, PL2, L30)

**Trigger:** Plan written, before commit. Last chance to catch stale citations, missing story coverage, wrong file paths, broken task dependencies.

**Rule:** Run these 4 checks before committing the plan file. Re-run on every plan edit.

| Check | Tool | What it catches |
|---|---|---|
| Drift scan cited SHAs | `git log --all -- <path>` for each cited path (L30) | Plan cites artifact that was never committed |
| Drift scan cited code blocks | `cat <path>` on `main` for each code block (PL1) | Plan code block ≠ current code |
| Drift scan cited deps | workspace + crate manifests (PL2) | Plan deps list ≠ Cargo.toml |
| Story ↔ task trace | Read matrix, find each task, verify CLI subcommand (PL3) | Aspirational coverage, unfulfilled promise |

**How to apply (4 sub-steps):**

1. **L30 SHA scan:** for every SHA / commit ref in plan header, `git log --all -- <cited-path>`. Empty = drift.
2. **PL1 content scan:** for every code block (file:line), `cat` the file on `main`. Mismatch = drift.
3. **PL2 dep scan:** for every dep list, diff against workspace + crate manifests. Mismatch = drift.
4. **PL3 story trace:** for every story in coverage matrix, locate the task and confirm CLI subcommand exists in task body.

**Why:** Plans get committed with stale citations, then the executor hits drift mid-task, then the user re-grinds the executor through re-discovery. Drift scan is 5 minutes; re-discovery is 2 hours.

**Anti-patterns:**
- "Plan looks right, push it." Drift is silent — looks-right is not is-right.
- Skipping drift scan on small edits. Small edits to large files = high drift probability.
- Trusting git timestamps. A plan committed 6 months ago can cite artifacts that drifted last week.

**Companion:** L30 (step 4a in L13), PL1, PL2, PL3. Drift scan IS the close-the-loop for plan authoring.

---

## PL15 — Step 6: Executor pick + pickup + run + merge

**Trigger:** Plan committed and merged to main. Next user pickup signal: "do next session" / "pick up #N" / specific task. Pick the executor plugin, run the pipeline, close the loop.

**Rule:** Apply L13 per-task pipeline literally. Step 1 = skill-tag (3-5 skills for THIS task). Step 2 = invoke. Step 4a = drift scan. Step 4b+ = TDD + L12 review + verify + L24 docs + PAUSE + commit-push-PR + admin merge.

**How to apply (L13 reference):**

1. **Skill-tag (L11):** pick 3-5 from `superpowers:test-driven-development`, `code-reviewer`, `systematic-debugging`, `verification-before-completion`, etc.
2. **Drift scan (L30 / PL14):** re-verify every plan/SHA citation the picked-up issue references.
3. **TDD:** red → green → refactor per step.
4. **L12 review:** before local verify gate, not after.
5. **Verify gate:** `cargo fmt + clippy + test` (or equivalent per stack).
6. **L24 doc cascade:** CHANGELOG + User Stories + README on PR merge.
7. **PAUSE for commit** (L6): user types "commit" / "approved" before `git commit`.
8. **PR + admin merge** (L6 + admin-bypass rule): `gh pr merge --squash --admin --delete-branch` only after user types "admin bypass".
9. **L21 ledger update:** estimate-report + ai-cost-report on merge.
10. **L13 step 18 lesson capture:** if a reusable lesson surfaced, write to `lessons.md` or `plan-lesson.md`.

**Why:** Skipping any step in L13 = silent gap. L13 exists because the author already burned cycles discovering the order. Don't re-discover.

**Anti-patterns:**
- "L13 is overkill for this task." It isn't. The 18 steps are the minimum.
- "I'll do step 7 (PAUSE) after the commit." No — PAUSE comes before commit, not after.
- "Drift scan is paranoia." L30 lesson exists because drift cost real hours in 2026-08-12.

**Companion:** L13 (the pipeline), L6 (approval gates), L11 (skill-tag), L28 (verify-before-claim), L29 (operator-driven smoke), L30 (drift scan). PL15 = the close-the-loop step that invokes the whole L13 stack.

---

## PL16 — Plugins for PLANNING (use during plan authoring)

**Trigger:** Confused about which skill to invoke at each planning step. Quick reference.

| Phase | Plugin / skill | When |
|---|---|---|
| Skill discovery | `superpowers:using-superpowers` | Every session start, before any action |
| Skill selection | `superpowers:writing-skills` | When authoring a new reusable skill |
| Idea → design | `superpowers:brainstorming` | **Hard gate** before any implementation |
| Spec writing | `superpowers:writing-plans` | After spec approved, before code |
| Plan writing | `superpowers:writing-plans` | After spec approved; ≥3 tasks |
| Plan authoring assist | `superpowers:claude-code-guide` | When asking about Claude Code features, hooks, plugins |
| Visual mockups | `superpowers:brainstorming` (visual companion) | When question clearer shown than told |
| Subagent orchestration | `superpowers:subagent-driven-development` | ≥5-task plans (PL9) |
| Inline plan execution | `superpowers:executing-plans` | 1-2 task plans |

**Anti-patterns:**
- Calling implementation skills (frontend-design, mcp-builder) directly. Per brainstorming hard gate: writing-plans is the ONLY next step after brainstorming.
- Skipping `using-superpowers` at session start. The skill-index is the routing table.

---

## PL19 — Approval-before-execute gate

**Trigger:** Session 2026-08-12. Multiple times agent executed `git commit` or `gh pr merge` after user said only "approved" — but "approved" is ambiguous (approved to commit? approved to push? approved to merge? approved with admin-bypass?). L28 + L13 step 14 prohibit speculatively flipping acceptance boxes without explicit form. `never-auto-commit` (memory) and `workflow-approval-required` (memory) both require explicit state-modifying approval.

**Rule:** Before any state-modifying action, pause and ask the user for explicit approval of the **specific** action. "Approved" alone is not enough. The required form includes the action verb + scope.

**State-modifying actions requiring explicit approval (non-exhaustive):**

| Action | Required approval form | Why |
|---|---|---|
| `git commit` | "commit" + commit message body, OR `#commit-approved` marker in body | `never-auto-commit` rule |
| `git push` | "push" + branch name | Local commits are reversible; pushes are not |
| `gh pr merge` | "admin bypass" (literal phrase) for `--admin` flag, or "merge" for default flow | Admin merge bypasses branch protection; irreversible |
| `gh issue edit` | "update body" + specific issue #, OR "edit #N" | Changes public artifact |
| `gh issue close` | "close #N" | Closes receipt; affects issue tracker audit |
| `git branch -D` | "delete branch" + branch name | Force-delete is irreversible |
| File move outside `docs/` | "move <from> → <to>" | `workflow-approval-required` rule |
| `cargo publish` / npm publish | "publish <crate> <version>" | Public release; cannot unpublish |
| `.env` write / secret rotation | "rotate <secret-name>" | Affects production credentials |
| Force-push to `main` | "force-push main" (literal "force-push") | Rewrites shared history |

**How to apply:**

1. **Before any action in the table above**, surface the action in plain text and pause. Example: "About to run `gh pr merge --squash --admin --delete-branch 91`. Approve?"
2. **Wait for explicit form.** "Approved" alone → AskUserQuestion to disambiguate.
3. **For multi-step state changes**, ask once per step. Don't bundle "commit + push + merge" into one approval.
4. **For "approved" follow-ups that match a previous clear approval** (e.g., user typed "approved" right after you showed a commit message), re-confirm the action verb matches.

**Anti-patterns:**
- Treating "approved" as blanket permission for any state-modifying action.
- Speculatively flipping issue `[ ]` boxes after "approved" without operator evidence (L28).
- Bundling commit + push + merge into one approval question.
- Using "ok" / "sure" / "go ahead" as approval triggers. These are conversation acknowledgments, not action approvals.

**Companion:** L6 (approval gates before persistent changes), L28 (verify-before-claim), `never-auto-commit` (memory), `workflow-approval-required` (memory), L13 step 14 (per-box evidence).

---

## PL20 — Tier 2 planning plugins (heavier ceremony)

**Trigger:** Session 2026-08-12 plugin survey (`find ~/.claude/plugins -type d -name "skills"`) found 17 planning-relevant skills. PL16 covers Tier 1 (5 core skills for any project). This entry covers Tier 2 + Tier 3 for projects that need heavier ceremony.

**Rule:** For greenfield / multi-agent / multi-stage projects, layer Tier 2 plugins on top of Tier 1 (PL16). For ≥3-stage agent handoffs, use compound-engineering (`ce-*`) instead of superpowers (`superpowers:*`) for the planning side. Don't mix the two on the same project.

**Tier 2: planning-augmentation (use for harder projects):**

| Plugin / skill | When | Why it matters |
|---|---|---|
| `compound-engineering:ce-plan` | Multi-stage plans with handoffs | 6-stage plan; wider than `superpowers:writing-plans`; pairs with `ce-handoff` |
| `compound-engineering:ce-brainstorm` | Brainstorming with visual probes | Heavyweight alternative to `superpowers:brainstorming`; output-mode + section-order tests |
| `compound-engineering:ce-ideate` | Pure ideation, no commitments | When exploring options before committing to a plan |
| `mattpocock-skills:codebase-design` | Module/interface design (L11 row) | Pairs with `pr-review-toolkit:type-design-analyzer` for interface contracts |
| `mattpocock-skills:domain-modeling` | New domain or threat model | Already in L11 map; re-invoke for spec/threat-model review |
| `architecture-decision-records` (ADR skill) | When picking between ≥2 approaches (PL11 step 4) | Captures decision + rejected alternatives; write ADR per `docs/agents/adr` |

**Tier 3: adjacent / agent-only (use when scaling up):**

| Plugin / skill | When | Why it matters |
|---|---|---|
| `agent-planner` (or `agent-architecture`) | Multi-agent fleet planning | When planning a system of agents, not just a project |
| `agent-specification` | Writing specs for agent behavior | Lower priority than `superpowers:brainstorming` for human-facing projects |
| `agent-researcher` | Long-running research tasks | If planning requires deep prior-art search before spec |
| `compound-engineering:ce-handoff` | Plan → spec → plan handoffs | Stage transitions; useful for `ce-plan` integration |
| `compound-engineering:ce-compound` | End-to-end compound loop | "Compound" all CE stages; heavyweight — use only when CE is the chosen stack |

**How to pick (decision tree):**

1. **Is this a 1-feature project on an existing codebase?** → Tier 1 only. PL16.
2. **Is this a new subsystem / greenfield feature?** → Tier 1 + Tier 2 (`mattpocock-skills:codebase-design` + `mattpocock-skills:domain-modeling`).
3. **Is this a multi-stage project with multiple agents handing off work?** → Tier 1 + Tier 2 (`compound-engineering:ce-plan` + `ce-handoff`). Skip superpowers:writing-plans — `ce-plan` replaces it.
4. **Is this a multi-agent fleet / system of agents?** → All three tiers. Add `agent-planner` + `agent-architecture`.

**Why:** Two parallel planning pipelines exist. **superpowers** = simpler, narrower, hard-gated. **compound-engineering** = wider, more stages, handoff-driven. Both target the same outcome (spec → plan → execute) but with different ceremony. Mixing both on the same project = drift, handoff confusion, double-spec syndrome.

**Anti-patterns:**
- Skipping `using-superpowers` at session start. The skill-index IS the routing table.
- Using `ce-plan` AND `superpowers:writing-plans` on the same project. Pick one. Both produce plans; mixing produces drift.
- `agent-planner` for a 5-task human project. Overkill; use Tier 1 + `subagent-driven-development`.
- `architecture-decision-records` for trivial choices (single alternative, no trade-off). ADRs are for *rejected alternatives*, not announcements.

**Companion:** PL16 (Tier 1 quick-reference table), L11 mapping table in `tasks/lessons.md` (already references `mattpocock-skills:codebase-design` + `mattpocock-skills:domain-modeling`).

---

## Worked example — `2026-08-05-rust-bitcoin-wallet.md`

Worked audit showing how to apply PL1–PL16 when reviewing or extending an existing plan. Reference: PR review session 2026-08-12.

### Step 1: PL2 drift scan (before any edit)

```bash
# 1. Plan freshness
git log --all -- docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md
git log -1 main -- docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md

# 2. Cited code-block freshness (Cargo.toml example)
git log --all -- rust-wallet-app/crates/bitcoin-wallet-core/Cargo.toml
# If this changed more recently than the plan: PL1 = drift, fix plan, not code.

# 3. Cited file existence
test -f rust-wallet-app/crates/bitcoin-wallet-core/src/lib.rs && echo OK
```

Outcome (this audit, 2026-08-12): plan was 6 PRs stale relative to `main`. Cargo.toml block in plan listed 14 deps; actual file listed 27. Resolution per PL1: edit plan, not code.

### Step 2: PL3 story ↔ task trace

Read the Story Coverage Matrix (lines 70-75 in original). For each row, locate the task ID cited and verify the task body contains the CLI subcommand:

| Story | Cites task | CLI subcommand in task body? |
|---|---|---|
| 1 Create wallet | 3, 9 | ✅ Task 9 = `Wallet::from_mnemonic` |
| 2 Import wallet | 3, 9 | ❌ No `btc wallet import` subcommand |
| 3 Check balance | 9 | ✅ Task 9 = `Wallet::balance` |
| 4 Sync chain | 9 | ✅ Task 9 = `Wallet::sync` |
| 11 Persist | 9 | ✅ via `bdk_file_store` |
| 12 Config show | 9 | ❌ No `btc config show` subcommand |

Outcome: Stories 2 and 12 lack CLI evidence. Two options per PL3:
- **Option A:** Change matrix status from `core` to `lib-only (no CLI)`. Honest.
- **Option B:** Add `Task 10: btc wallet import` + `Task 11: btc config show`. More work.

For this plan, **Option A** fits MVP scope. PL3 keeps matrix honest without inflating task list.

### Step 3: PL7 plugin stack audit

```bash
# Verify each plugin in PL7 is installed
ls ~/.claude/plugins/cache/*/superpowers/*/skills/brainstorming
ls ~/.claude/plugins/cache/*/superpowers/*/skills/writing-plans
ls ~/.claude/plugins/cache/*/superpowers/*/skills/subagent-driven-development
```

Outcome: all three plugins present. PL7 stack applicable to this plan.

### Step 4: PL8 SDK design check (host-first)

Check the spec for: host integration sketch, stability policy, flat re-exports, async mutex. See PL4, PL5, PL6 (in `review-lesson.md`) for the rules.

Outcome: missing Stability Policy section. Add above Global Constraints per PL6 template.

### Step 5: PL9 executor pick

Count tasks. Plan = 11 tasks (0, 0a, 1, 1.5, 2-9). Per PL9: ≥10 tasks = SDD mandatory. Plan header should declare "Execution: SDD."

### Step 6: PL14 drift scan + review

Run the 4-check drift scan (SHA, code blocks, deps, story trace). Fix plan-doc issues, leave code untouched.

### Step 7: PL15 close the loop

When a future user picks up this plan, L13 pipeline applies: skill-tag → drift scan (PL14) → TDD per task → L12 review → verify gate → L24 docs → PAUSE → commit → PR → admin merge.

### Re-audit cadence

Run PL1–PL16:
- **Every task pickup** (L13 step 4a + PL1 drift scan).
- **Every plan edit** (before commit, scan cited paths).
- **Quarterly** (cold review of every plan file under `docs/superpowers/plans/`).

Per-session checklist:

```text
□ PL1: cat <plan-file> vs latest <plan-file> on main
□ PL2: any "missing X" review finding? git log to verify
□ PL3: trace each story row → task → CLI subcommand
□ PL7: brainstorming → writing-plans → executor chain applied?
□ PL8: spec covers host-first, stability, type narrowness?
□ PL9: plan size vs executor plugin match?
□ PL10: user's verbatim instruction logged?
□ PL11: brainstorming skill invoked for design step?
□ PL12: spec written + self-review + user sign-off?
□ PL13: writing-plans invoked for plan file?
□ PL14: drift scan run before plan commit?
□ PL15: L13 pipeline applied at pickup?
□ PL16: right planning plugin picked per phase?
```

---

## Self-Review

1. **Scope:** All 13 PL lessons (PL1, PL2, PL3, PL7–PL16) apply to plan authoring + plan review for library/SDK crates. They are NOT corrections (those belong in `lessons.md`).
2. **Removed from this file:** PL4, PL5, PL6 (review+planning overlap, moved to `review-lesson.md`), PL17, PL18, PL19, PL20 (review + search, moved to companion files).
3. **Companion mapping:**
   - PL1 + PL2 + L30 = full plan-doc drift detection (header SHA → file content → dep list).
   - PL3 + L24 = story coverage honesty (matrix ↔ task list ↔ issue close).
   - PL7 + PL11 + PL12 + PL13 = planning-phase plugin stack.
   - PL8 + PL4 + PL5 + PL6 = SDK quality bar (cross-file: plan-lesson.md + review-lesson.md).
   - PL9 + L13 step 13-15 = executor plugin → PR pipeline.
   - PL10 → PL11 → PL12 → PL13 → PL14 → PL15 = full plan-author workflow (6 steps).
   - PL16 = quick-reference table for the planning-phase plugin stack.
4. **Anti-pattern audit:** Each lesson names the failure mode it prevents. No lesson is purely aspirational.

---

## Update protocol

When a plan review surfaces a reusable lesson:

1. **Draft** new PL entry in this file (terse: trigger → rule → why → how to apply → anti-patterns).
2. **Add** to Index above.
3. **Cross-link** to related `lessons.md` entries (L30, L24) and to the originating plan/session.
4. **Commit** on its own PR titled `docs(plan-lesson): PL<N> <short title>`.
5. **Wire** into L13 pickup checklist (next to L30 step 4a).

Do NOT add to `lessons.md`. That file is for corrections, not planning discipline.