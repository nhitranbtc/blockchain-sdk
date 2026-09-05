# issues-lesson.md

Issue-creation discipline playbook. Companion to `plan-lesson.md` (planning), `review-lesson.md` (review), `search-lesson.md` (search), `task-map-lesson.md` (task maps), and `lessons.md` (corrections ledger).

Scope: GitHub issue creation via `gh` CLI for every workflow (direct, spec → issue, architectural → spec → issues, triage, wayfinder). Excludes planning-only lessons and review-only lessons.

Read at session start when running issue pickup, `/triage`, `/to-spec`, `/to-tickets`, `/wayfinder`, or any workflow whose terminal step is `gh issue create`.

---

## Index

- [PL21] Issue-creation workflows (5 paths; one per situation)
- [PL22] Pause-then-act pattern (state facts → wait approval → run → report URL)
- [PL23] Issue body templates (feature variant + bug variant)
- [PL24] Two-layer label system (triage roles + chain/area labels)
- [PL25] Wayfinder + native blocking edges (GitHub issue dependencies)
- [PL26] Backlog triage issue creation (L13 step 11a — 7 classes, deterministic decision)

---

## PL21 — Issue-creation workflows (5 paths)

**Trigger:** Any "create issue" request. Match the request to a workflow before reaching for `gh`.

**Rule:** Five workflows, ordered by work size. Each ends in one or more `gh issue create` calls.

| # | Workflow | When | Skill prefix |
|---|---|---|---|
| 1 | **Direct** | One well-scoped bug or feature | none |
| 2 | **Spec → issue** | One piece of work that needs `CONTEXT.md` + ADRs | `mattpocock-skills` |
| 3 | **Architectural → spec → issues** | New project, subsystem, or interface restructure | `superpowers` + `mattpocock-skills` |
| 4 | **Triage** | Incoming issue you didn't create | `mattpocock-skills:/triage` |
| 5 | **Wayfinder** | Idea too big for one session | `mattpocock-skills:/wayfinder` |

**Workflow 1 (Direct):**

```text
state facts (title, body, labels, assignee)
  → wait approval
  → gh issue create --title ... --body ... --label ...
  → report URL
```

**Workflow 2 (Spec → issue, mattpocock):**

```text
/mattpocock-skills:grill-with-docs         # stateful; build CONTEXT.md + ADRs
  → /mattpocock-skills:to-spec             # synth → spec on tracker
    → /mattpocock-skills:to-tickets        # spec → tracer-bullet tickets
      → gh issue create per ticket (one file = one issue)
        → gh api .../issues/<n>/dependencies/blocked_by (wire edges)
```

**Workflow 3 (Architectural → spec → issues, superpowers + mattpocock):**

```text
/superpowers:brainstorming                  # classify spike / bounded / architectural
  → if architectural: write spec to docs/superpowers/specs/
  → /superpowers:writing-plans              # spec → bite-sized plan
    → /mattpocock-skills:to-tickets         # plan → tracker tickets
      → gh issue create per ticket
```

**Workflow 4 (Triage, mattpocock):**

```text
/mattpocock-skills:triage                   # state machine; only for issues you didn't create
  → apply triage label (needs-triage / ready-for-agent / etc.)
  → write agent-ready brief
  → gh issue edit <n> --add-label ...
```

**Workflow 5 (Wayfinder, mattpocock):**

```text
/mattpocock-skills:wayfinder               # chart wayfinder:map
  → gh issue create --label "wayfinder:map"   # the map issue
    → gh issue create --label "wayfinder:<type>"  # child tickets, one per question
      → gh api .../issues/<child>/dependencies/blocked_by  # wire frontier
        → /mattpocock-skills:wayfinder again to resolve one ticket per session
```

**Why:** Workflow 1 wastes CONTEXT.md work for one-line tasks. Workflows 2-3 produce underspecified issues without brainstorming. Workflow 4 misapplies to your own tickets (already agent-ready from `/to-tickets`). Workflow 5 turns a foggy idea into a series of decision tickets instead of one mega-issue that nobody can pick up.

**How to apply (at issue-creation time):**

1. Classify the request: well-scoped? needs state? architectural? incoming? foggy?
2. Pick the matching workflow number (1-5).
3. Run the skill prefixes in order; pause at every `gh` write.
4. For workflows 2 and 3, run one `gh issue create` per ticket from `/to-tickets`.
5. For workflow 5, the map is itself an issue (`wayfinder:map` label); children reference it.

**Anti-patterns:**

- Defaulting to workflow 1 for everything. One-line `--title` issues waste triage time.
- Running `/triage` on tickets `/to-tickets` produced. `/to-tickets` already emits agent-ready output.
- Skipping `superpowers:brainstorming` for architectural work. Then the spec lacks the path classification (spike / bounded / architectural).
- Running `wayfinder` then jumping straight to `gh issue create` per ticket. The collapse happens in `/to-spec`, not at issue creation.
- Creating a wayfinder map without `gh api` blocking edges. Children then have no visible frontier.

---

## PL22 — Pause-then-act pattern (state-modifying `gh` writes)

**Trigger:** Any `gh` command that mutates tracker state.

**Rule:** Per workspace `memory/MEMORY.md` (`workflow-approval-required`) and project `CLAUDE.md`, all of these pause for explicit user approval before execution:

- `gh issue create`
- `gh issue edit`
- `gh issue comment`
- `gh issue close`
- `gh issue lock`
- `gh issue transfer`
- `gh pr create`
- `gh pr merge`
- `gh api` writes

Read-only actions (`view`, `list`, `search`) run without approval.

**Pattern (4 steps):**

```text
1. STATE THE FACTS (GateGuard / project rule):
   - Title
   - Body source (inline / heredoc / file)
   - Labels to apply
   - Assignee
   - Milestone (if any)
2. WAIT for explicit approval.
3. RUN the command.
4. REPORT the new issue URL for confirmation.
```

**Why:** `gh issue create` is irreversible once submitted (edits cost history). The GateGuard `Fact-Forcing Gate` + project `workflow-approval-required` rule exist to keep agents from auto-publishing. Pausing catches wrong labels, wrong assignees, wrong repos, wrong titles.

**How to apply (at every `gh` write):**

1. State the command you intend to run, with all flags.
2. State the inferred repo (from `git remote -v`).
3. State the body source (inline / heredoc / `/tmp/issue-body.md`).
4. State the labels and assignee you will apply.
5. Wait for user approval.
6. Run.
7. Report URL.

**Anti-patterns:**

- Running `gh issue create` immediately on user request. Bypasses the pause; bypasses GateGuard.
- Skipping step 1 because the command looks "obvious." Body content with `rm` / `rmdir` / destructive verbs fails the GateGuard classifier — always preview.
- Reporting "done" after the command without step 7 (URL). The user needs the URL to verify.

**Companion:** `memory/MEMORY.md` (`workflow-approval-required`, `update-issues-before-merge`).

---

## PL23 — Issue body templates (feature variant + bug variant)

**Trigger:** Drafting the body for `gh issue create`.

**Rule:** Use one of two workspace templates (per `docs/agents/issue-tracker.md` and workspace convention).

**Feature template:**

```markdown
## Context

<why this matters; what triggered it; any links to prior issues, commits, docs>

## Goal

<one-sentence outcome>

## Acceptance criteria

- [ ] criterion 1
- [ ] criterion 2
- [ ] criterion 3

## Out of scope

<what this issue does NOT do>

## References

- Spec: <path or URL>
- Related: #<n>, #<n>
- Doc: <path>
```

**Bug template:**

```markdown
## Repro

1. ...
2. ...

## Expected

<what should happen>

## Actual

<what happens>

## Environment

- Rust: <version>
- Chain: <name>
- RPC: <URL or env var>

## Acceptance

- [ ] Regression test reproduces the bug
- [ ] Fix passes the regression test
- [ ] Existing tests still pass
```

**Why:** Bare `--title` issues waste triage time. A maintainer needs context, repro, environment to assign work. The template enforces the minimum surface area.

**How to apply:**

1. Pick the variant (feature / bug).
2. Draft body in markdown; use heredoc with quoted delimiter (`<<'EOF'`) for inline.
3. For bodies with `rm` / `rmdir` / destructive verbs (e.g., a migrator issue), write to `/tmp/issue-body.md` first and pass via `--body-file`. GateGuard fails inline destructive prose.
4. Always include at least one Acceptance criterion — otherwise the issue is unactionable.

**Anti-patterns:**

- Skipping "Context." The next reader needs to know why.
- Writing "see code" in place of repro. Repro is concrete; "see code" is not.
- Listing "Out of scope" with no entries. Either delete the section or list what is explicitly excluded.
- Forgetting `Related: #n`. Cross-link prevents duplicate work.

**Companion:** PL22 (pause-then-act), PL24 (labels).

---

## PL24 — Two-layer label system (triage roles + chain/area labels)

**Trigger:** Every `gh issue create` or `gh issue edit --add-label`.

**Rule:** This repo uses two layers (per `docs/agents/triage-labels.md`).

**Layer 1: Triage roles (lifecycle state).**

| Label | Meaning |
|---|---|
| `needs-triage` | Maintainer needs to evaluate |
| `needs-info` | Waiting on reporter |
| `ready-for-agent` | Fully specified, AFK-executable |
| `ready-for-human` | Requires human implementation |
| `wontfix` | Will not be actioned |

**Layer 2: Chain / area labels (work domain).**

| Label | Scope |
|---|---|
| `rust-evm-core` | Ethereum / EVM-core work |
| `rust-btc-core` | Bitcoin-core work |
| `rust-eth-core` | Ethereum-specific (vs generic EVM) |
| `rust-tron-core` | TRON-core work |
| `polygon-core` | Polygon-specific work |
| `backlog` | Accepted but not scheduled |
| `task` | Generic task label |

Apply both layers at create time: one triage role + one chain/area label.

```bash
gh issue create \
  --title "..." \
  --body "..." \
  --label "needs-triage,polygon-core,task"
```

**Why:** Single-layer labels lose either state (where in the lifecycle?) or domain (which chain?). Two layers let the maintainer filter both axes.

**How to apply:**

1. New issues default to `needs-triage` (Layer 1) + chain label (Layer 2) + `task` (Layer 2 generic).
2. After triage, swap `needs-triage` for the resulting role (`ready-for-agent`, `needs-info`, `wontfix`).
3. Cross-cutting issues (e.g., doc cleanup) get `task` without a chain label.
4. Verify labels exist via `gh label list` before creating — typo = silent failure.

**Anti-patterns:**

- Using only Layer 1 (`needs-triage` alone). No chain signal.
- Using only Layer 2 (`polygon-core` alone). No state signal.
- Inventing new labels mid-stream. Add to `docs/agents/triage-labels.md` first, then use.
- Forgetting to swap `needs-triage` after triage. Issues accumulate in `needs-triage` forever.

**Companion:** `docs/agents/triage-labels.md`, PL21 workflow 4 (triage workflow).

---

## PL25 — Wayfinder + native blocking edges (GitHub issue dependencies)

**Trigger:** Workflow 5 (wayfinder) or any time tickets have explicit blocker relationships.

**Rule:** Use GitHub's **native issue dependencies** to wire blocking edges. Native = UI-visible in the GitHub issue view (rendered as "Blocked by" / "Blocking"). Fall back to a body-line convention only if native dependencies aren't available in the repo.

**Create the map:**

```bash
gh issue create \
  --title "wayfinder: <effort name>" \
  --label "wayfinder:map" \
  --body "$(cat <<'EOF'
## Destination

<one or two lines: what reaching the end of this map looks like>

## Notes

<domain; standing preferences for this effort>

## Decisions so far

<!-- empty initially -->

## Not yet specified

<!-- in-scope fog -->

## Out of scope

<!-- closed, never graduates -->
EOF
)"
```

**Create a child ticket:**

```bash
gh issue create \
  --title "<ticket question>" \
  --label "wayfinder:grilling,polygon-core" \
  --body "$(cat <<'EOF'
Part of #<map-number>

## Question

<the decision this ticket resolves>
EOF
)"
```

**Wire a blocking edge:**

```bash
# Get the database id (NOT the #number)
BLOCKER_DB_ID=$(gh api repos/<owner>/<repo>/issues/<blocker-num> --jq .id)

# Add edge
gh api --method POST \
  repos/<owner>/<repo>/issues/<child-num>/dependencies/blocked_by \
  -F issue_id=$BLOCKER_DB_ID
```

**Claim + resolve:**

```bash
gh issue edit <n> --add-assignee @me           # claim (first write)
gh issue comment <n> --body "<the answer>"     # record resolution
gh issue close <n>                             # close
# Then append a context pointer (gist + link) to the map's Decisions-so-far
```

**Frontier query (open children with no open blockers and no assignee):**

```text
gh issue list --state open
  → scope to map's sub-issues / task list
  → drop any with issue_dependencies_summary.blocked_by > 0
  → drop any with assignee
  → first in map order wins
```

**Why:** Without native blocking edges, the frontier is invisible — every session opens the map body, parses `Blocked by: #n` lines by hand, checks each blocker status. With native edges, GitHub renders the frontier in the issue view, the slash `Blocked by:` query in `gh` returns the live gate, and `issue_dependencies_summary.blocked_by` reports open blockers only.

**How to apply:**

1. After creating child tickets, get the database id of each blocker (`gh api ... --jq .id`).
2. POST to `/dependencies/blocked_by` for each (child, blocker) pair.
3. Verify via `gh api repos/<owner>/<repo>/issues/<child> --jq .issue_dependencies_summary`.
4. In the wayfinder skill, gate ticket pickup on `blocked_by == []` AND `assignee == null`.

**Anti-patterns:**

- Using the GitHub `#number` instead of the database id. The endpoint needs `.id`, not `.number`.
- Wiring edges in one pass at chart time. Tickets need IDs first; wire in a second pass after creation.
- Falling back to body-line `Blocked by: #n` when native is available. Body text drifts; native is the truth.
- Closing a ticket without commenting the answer. The answer must travel with the close (comment + close).
- Re-opening a closed out-of-scope ticket. Per `wayfinder`, out-of-scope never graduates; re-route by closing and creating a fresh effort.

**Companion:** PL21 workflow 5, `mattpocock-skills:/wayfinder` SKILL.md (canonical ticket/map mechanics), `docs/agents/issue-tracker.md` "Wayfinding operations" section.

---

## PL26 — Backlog triage issue creation (L13 step 11a)

**Trigger:** L13 step 11 (verify gate) surfaces an error that can't be fixed in-task. Sequence per `lessons.md` L13 step 11a: step 11c (systematic-debugging) → step 11a (triage decision) → either fix-and-rerun-11 OR create-backlog-item. This lesson covers the create-backlog-item branch.

**Rule:** Classify the finding into one of 7 triage classes using deterministic criteria. Only 4 of the 7 classes produce a GitHub issue. The other 3 are handled in-session (fix now, log to L14 progress.md, or PR-body deferral).

**The 7 classes:**

| Class | Produces issue? | Decision criteria | Where it lands |
|---|---|---|---|
| **Fixable now** | No | ≤10 min + in scope + no new test required | Fix in current commit, re-verify, continue |
| **In-PR follow-up** | No | >10 min OR scope-creep risk | Commit in current PR (before merge), not main yet; lands via the feature PR's pipeline |
| **Small deferred** (cosmetic / follow-up) | No | Touches adjacent code OR needs new test but doesn't block | Log in current session's backlogs list + L14 progress.md events |
| **Big task** (multi-PR, multi-week) | **Yes** | Own PR OR multi-week OR cross-crate | Create GitHub issue, label `backlog`, link to parent task |
| **Code smell / debt** (knip / depcheck / dead-code finding from L12 sub-agent or `refactor-clean` audit) | Conditional | ≤10 min + in scope + no new test → fixable now; touches adjacent code OR scope-creep risk → small deferred with `refactor-clean` audit as acceptance criteria; cross-crate OR multi-PR → big task with backlog issue + parent task ref | Per sub-rule |
| **Future milestone** (v0.1.1, v0.2) | No | Doesn't ship before parent task's release | Log with `priority/p2\|p3` tag |
| **External gate** (operator-driven, L29 manual smoke / L28 Gate B) | No | Can't run in CI | Mark `[ ]` in PR body with `<!-- TODO: <operator-action> -->` deferral note (per step 14 external-gate discipline) |

**Deterministic decision tree (apply top-down, first match wins):**

```text
1. ≤10 min + in scope + no new test → fixable now (no issue)
2. >10 min OR scope-creep → in-PR follow-up (no issue; commit before merge)
3. needs new test OR adjacent code → small deferred (no issue; log to L14)
4. multi-week OR cross-crate → big task (CREATE ISSUE)
5. doesn't ship before parent release → future milestone (no issue; priority/p2|p3 tag)
6. operator-driven (L29 / L28 Gate B) → external gate (no issue; PR body TODO)
7. otherwise → fixable now (catch-all, no issue)
```

**GitHub issue format for backlog items (the only class that always issues):**

```bash
gh issue create \
  --title "Backlog: <short description>" \
  --label "backlog,priority/p<N>,week/N,<chain>,task" \
  --milestone "<parent task milestone>" \
  --body "$(cat <<'EOF'
## Acceptance criteria

- [ ] <criterion>
- [ ] <criterion>

## Priority

priority/p<N> — <rationale>

## Parent task

Refs #<parent-issue>

## L14 progress

<path to L14 progress.md link>
EOF
)"
```

**Priority ladder (per L13 step 11a):**

| Tag | Meaning |
|---|---|
| `priority/p0` | Blocks release (ship-stopper) |
| `priority/p1` | Blocks merge (must-fix before parent task closes) |
| `priority/p2` | Doesn't ship before parent release |
| `priority/p3` | Same as p2, lower urgency |

**Why:** Without deterministic triage, agents default to one of two failure modes:

- **Fix everything now** (scope creep, breaks the verify gate, may not even be in scope).
- **Skip and forget** (the finding surfaces again next session, wastes time).

The 7-class taxonomy forces a decision per finding, with deterministic criteria so two agents pick the same class for the same finding.

**How to apply (at L13 step 11a):**

1. Verify gate (cargo fmt + clippy + test) fails.
2. Apply step 11c (`superpowers:systematic-debugging`) — root cause first.
3. If the fix is in-scope and small, apply the decision tree above.
4. If class is "big task": state facts (title, body, labels, milestone, parent ref), wait approval, `gh issue create` with the format above, append the issue URL to L14 progress.md.
5. If class is "small deferred" or "future milestone": log to L14 progress.md only — no issue.
6. Re-run step 11 verify gate after fix or after backlog-issue creation.

**Anti-patterns:**

- Creating issues for fixable-now items. Pollutes the backlog with tiny fixes that drain triage time.
- Logging big-task items to L14 progress.md without creating an issue. They get lost in the next session's noise.
- Using `--label "backlog"` alone. Always add the priority tag + week + chain + task. Multi-axis filterable.
- Forgetting the parent task ref (`Refs #<n>`). Backlog items without parentage orphan.
- Skipping step 11c (`systematic-debugging`) before classifying. Symptom-classification produces wrong classes.

**Companion:** `lessons.md` L13 step 11a (canonical rules), `lessons.md` L13 step 11c (systematic-debugging before triage), `lessons.md` L14 (ledger rule + progress.md), `docs/agents/triage-labels.md` (label vocabulary), PL21 workflow 1 (single-issue creation), PL24 (two-layer label system).

---

## Self-Review

1. **Scope:** 6 lessons (PL21-PL26) cover the full issue-creation surface: workflows, pause-then-act, body templates, labels, wayfinder mechanics, backlog-triage issue creation (L13 step 11a).
2. **Removed from this file:** Planning-only lessons (in `plan-lesson.md`), review-only lessons (in `review-lesson.md`), search-only lessons (in `search-lesson.md`), task-map lessons (in `task-map-lesson.md`).
3. **Companion mapping:**
   - PL21 ↔ workflow 1-5 in `.local/plugins-docs/2026-08-31-gh-issue-creation-guide.md`
   - PL22 ↔ `memory/MEMORY.md` (`workflow-approval-required`, `update-issues-before-merge`)
   - PL23 ↔ `docs/agents/issue-tracker.md` "Conventions" section
   - PL24 ↔ `docs/agents/triage-labels.md`
   - PL25 ↔ `mattpocock-skills:/wayfinder` SKILL.md + `docs/agents/issue-tracker.md` "Wayfinding operations" section
   - PL26 ↔ `lessons.md` L13 step 11a (canonical rules), L13 step 11c (debugging-before-triage), L14 (progress.md ledger)
4. **Anti-pattern audit:** Each lesson names the failure mode it prevents.

---

## Update protocol

When an issue-creation step surfaces a reusable lesson:

1. **Draft** new PL entry in this file (terse: trigger → rule → why → how to apply → anti-patterns).
2. **Add** to Index above.
3. **Cross-link** to related `plan-lesson.md`, `review-lesson.md`, `lessons.md`, and `.local/plugins-docs/2026-08-31-gh-issue-creation-guide.md` entries.
4. **Commit** on its own PR titled `docs(issues-lesson): PL<N> <short title>`.
5. **Wire** into L13 pickup checklist (read at task pickup per L11).

Do NOT add to `lessons.md` (corrections ledger), `plan-lesson.md` (planning-only), `review-lesson.md` (review-only), `search-lesson.md` (search-only), or `task-map-lesson.md` (task-map-only).
