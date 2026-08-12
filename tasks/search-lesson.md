# search-lesson.md

Search + content + code-block review playbook. Companion to `plan-lesson.md` (planning) and `review-lesson.md` (review).

Scope: deep search of SDKs / tools / libraries, content review of markdown / docs, code-block review of specific functions. Excludes planning (PL1–PL3, PL7–PL16) and code review (PL4–PL6, PL17).

Read at session start when researching a new SDK, reviewing a markdown doc, or auditing a specific code block.

---

## Index

- [PL18] Plugins for CONTENT review (markdown, docs, prose)
- [PL19] Plugins for CODE-BLOCK review (specific functions, drift, correctness)
- [PL20] Plugins for DEEP SEARCH of SDKs / tools / libraries + agent management

---

## PL18 — Plugins for CONTENT review (markdown, docs, prose)

**Trigger:** User asks "review this README" / "check this spec for clarity" / "audit this CHANGELOG." Content review ≠ code review: focus on prose, structure, audience fit, not just correctness.

**Rule:** For markdown / docs / spec / CHANGELOG content, use the content-review stack. Skip code-only plugins (`pr-review-toolkit:test-analyzer`, `type-design-analyzer`) unless the doc has embedded code blocks (then layer PL19 on top).

| Content type | Plugin / skill | What it catches |
|---|---|---|
| Markdown structure | `superpowers:claude-code-guide` (or `mcp__exa__web_fetch_exa` for examples) | Heading hierarchy, list nesting, table syntax |
| Spec clarity | `superpowers:brainstorming` (spec self-review checklist) | Placeholders, contradictions, ambiguity, scope |
| User-facing docs | `voltagent-biz:technical-writer` (or `mcp__plugin_context7_context7__query-docs`) | Audience fit, missing prerequisites, broken examples |
| README quality | `voltagent-biz:readme-generator` (or human review) | Setup, "What's New", "Try it" column per L24 |
| CHANGELOG | L24 + `voltagent-biz:content-quality-editor` | "Keep a Changelog" format, user-story table |
| PR body | L9 (PR body = fix analysis with table) | Vague summaries, missing receipt links |
| Issue body | L9 (issue body = status with [ ] boxes) | Vague descriptions, missing acceptance criteria |
| Lesson capture | `tasks/lessons.md` rules (Boris Cherny workflow) | Trigger → rule → why → how to apply format |
| AI writing patterns | `voltagent-biz:ai-writing-auditor` (or `voltagent-biz:content-quality-editor`) | "delve into", "leverage", "robust solution" AI tells |
| Cross-SDK comparison | `mcp__exa__web_search_exa` + `mcp__firecrawl__firecrawl_scrape` | Stale SDK docs, missing alternatives |
| Citation accuracy | L30 + PL1 + PL2 (in `plan-lesson.md`) | Outdated URLs, broken links, drift to current API |

**Anti-patterns:**
- Running `pr-review-toolkit:code-reviewer` on a doc-only PR. Wrong tool — no code to review.
- "The markdown renders, ship it." Renders ≠ readable. Run content-quality pass before merge.
- Skipping AI-pattern audit on public-facing docs. Public release + AI tells = credibility hit.

**Companion:** L9 (PR/issue body format), L24 (doc cascade on merge), L30 (drift scan), PL1/PL2 (plan-doc drift, in `plan-lesson.md`).

---

## PL19 — Plugins for CODE-BLOCK review (specific functions, drift, correctness)

**Trigger:** Reviewing one function, one file, or one code block in a plan / spec / doc — not a whole PR. Code-block review is narrower than PR review: focus on a specific snippet.

**Rule:** For targeted code-block review, layer the right plugin on top of the doc-content review (PL18). If the code block claims to be from a real file, verify with `cat` (PL1) before reviewing.

| Code-block review type | Plugin / skill | What it catches |
|---|---|---|
| Function correctness | `compass:debugger` | Wrong return type, off-by-one, missing edge cases |
| Type design | `pr-review-toolkit:type-design-analyzer` | Encapsulation, invariant expression, usefulness |
| API design | `compass:api-designer` | REST/gRPC/protobuf contract, versioning, backward compat |
| Security | `compass:security-auditor` OR `ecc:security-reviewer` | Authz, secrets, injection, trust boundaries, crypto |
| Performance | `compass:perf-profiler` | Latency, throughput, CPU, memory, allocations |
| Test coverage | `compass:test-architect` OR `pr-review-toolkit:pr-test-analyzer` | Missing edge cases, weak assertions |
| Drift vs current code | `tasks/plan-lesson.md` PL1, PL2, L30 | Plan code block ≠ actual file on `main` |
| Compile / type check | `compass:build-error-resolver` OR `ecc:rust-build-resolver` (per language) | Type errors, missing deps, borrow checker issues |
| Silent failure | `pr-review-toolkit:silent-failure-hunter` | Swallowed errors, bad fallbacks, missing propagation |
| Comment accuracy | `pr-review-toolkit:comment-analyzer` | Doc rot, wrong claims, stale references |
| Code simplify | `pr-review-toolkit:code-simplifier` OR `code-simplifier:code-simplifier` | Duplication, unclear names, over-engineering |
| Library API usage | `mcp__plugin_context7_context7__query-docs` | Wrong API call, deprecated method, missing feature flag |
| Refactor safety | `compass:refactoring-specialist` OR `ecc:refactor-cleaner` | Dead code, broken callers, missed imports |

**Drift-first rule:** If reviewing a code block from a plan or doc, run `cat <path>` first. If the block matches the file, then layer the language-specific reviewer. If it drifts, fix the doc (PL1), not the code.

**Anti-patterns:**
- Running 12 plugins on one function. Pick the 1-2 that match the review goal.
- "It compiles, ship it." Compile = syntax. Review = semantics + design.
- Reviewing a code block without checking it matches the current file. Stale review = noise.

**Companion:** PL1, PL2, L30 (drift, in `plan-lesson.md`), L12 (PR review order), `pr-review-toolkit:code-reviewer` (broader PR scope).

---

## PL20 — Plugins for DEEP SEARCH of SDKs / tools / libraries + agent management

**Trigger:** Researching an unfamiliar SDK, comparing libraries, or managing long-running subagent fleets. Quick reference for tools that go beyond this repo.

**Rule:** Match the search depth to the question. Don't use a research-grade tool for a one-fact lookup; don't use grep for cross-repo pattern discovery.

| Search task | Tool / agent | Why |
|---|---|---|
| One-fact lookup (this repo) | `Read` + `Grep` | Direct, no overhead |
| Multi-file search (this repo) | `caveman:cavecrew-investigator` (read-only, no fix) | Token-efficient summary |
| Broad code search across dirs | `Explore` (general-purpose search agent) | Fan-out across many files |
| Repo-wide audit / review | `compass:code-reviewer` (read-only) | Confidence-based prioritized findings |
| Cross-SDK research | `mcp__exa__web_search_exa` + `mcp__firecrawl__firecrawl_scrape` | Multi-source synthesis |
| Library docs lookup | `mcp__plugin_context7_context7__resolve-library-id` + `query-docs` | Up-to-date API + examples |
| Async deep research | `mcp__firecrawl__firecrawl_agent` + `firecrawl_agent_status` | Multi-source synthesis over minutes |
| Paper / academic search | `mcp__firecrawl__firecrawl_research_search_papers` + `read_paper` | Indexed corpus + full-text passages |
| GitHub issue / PR search | `mcp__firecrawl__firecrawl_developer_search` (developer category) | Curated dev-source index |
| Web search (general) | `WebSearch` | General web |
| URL fetch + answer | `WebFetch` | Single page → markdown + prompt answer |

**Agent management (long-running / multi-agent):**

| Need | Agent / pattern | When |
|---|---|---|
| Loop engineering patterns | `mcp__loop-engineering__loop_list_patterns` + `loop_get_pattern` | Recurring automation design |
| Swarm coordination | `claude-code-guide` (multi-agent setup) | Multi-agent fleet routing |
| Background subagent | `Agent` tool with `run_in_background: true` | Long tasks, want to interject |
| Worktree isolation | `Agent` tool with `isolation: "worktree"` | Parallel file mutations, no conflicts |
| Multi-agent workflow | `Workflow` tool (user opt-in via "ultracode" or explicit ask) | ≥3-phase work needing deterministic control flow |
| Loop supervision | `mcp__loop-engineering__loop_list_state_files` + `loop_get_state` | Monitoring recurring loops |
| Agent creator (new agent) | `plugin-dev:agent-creator` | User asks for new agent type |
| Plugin validator | `plugin-dev:plugin-validator` | After creating/modifying plugin components |
| Skill reviewer | `plugin-dev:skill-reviewer` | After creating/modifying skills |

**Search-vs-review boundary:** Search = "where is X, what calls Y, list all uses of Z." Review = "is X correct, secure, idiomatic, well-typed." Don't conflate — search agents (`caveman-investigator`, `Explore`) refuse to suggest fixes; review agents (`compass-code-reviewer`, `pr-review-toolkit:code-reviewer`) read selectively.

**Anti-patterns:**
- Using `Agent` (general-purpose) for every task. Pick the specialist (PL19 table) when one matches.
- Running 5 search agents in parallel when 1 fan-out covers it. Parallel = barrier cost; pick `Explore` for fan-out.
- Using `mcp__firecrawl__firecrawl_agent` for one-fact lookups. Asynchronous research for known-simple questions = wasted budget.
- Skipping the `caveman-investigator` for "where is X in this repo." Direct Read+Grep is fine for 1-2 files; investigator wins at 5+ files.

**Companion:** PL16 (planning plugins, in `plan-lesson.md`), PL17 (reviewing plugins, in `review-lesson.md`), PL18 (content review), PL19 (code-block review). PL20 = the research + agent-management layer that supports all four.

---

## Self-Review

1. **Scope:** All 3 lessons (PL18, PL19, PL20) apply to deep search, content review, and code-block review.
2. **Removed from this file:** PL1–PL16 (planning, in `plan-lesson.md`), PL4–PL6, PL17 (review, in `review-lesson.md`).
3. **Companion mapping:**
   - PL18 = content review (markdown / docs / prose).
   - PL19 = code-block review (specific functions / drift / correctness).
   - PL20 = deep search + agent management (SDKs / tools / libraries).
4. **Anti-pattern audit:** Each lesson names the failure mode it prevents.

---

## Update protocol

When a search or content review surfaces a reusable lesson:

1. **Draft** new PL entry in this file (terse: trigger → rule → why → how to apply → anti-patterns).
2. **Add** to Index above.
3. **Cross-link** to related `plan-lesson.md` and `review-lesson.md` entries.
4. **Commit** on its own PR titled `docs(search-lesson): PL<N> <short title>`.
5. **Wire** into search/content/code-block review checklist.

Do NOT add to `lessons.md` (corrections ledger), `plan-lesson.md` (planning-only), or `review-lesson.md` (code review-only).