# Guide: Audit + Plan Stack Methodology (8.8/10 rating pattern)

**Date:** 2026-08-05
**Purpose:** Capture the methodology that produced the 8.8/10 average rating across the 10-doc stack for the Bitcoin wallet rewrite. Reusable for any future audit + plan + design work (Lightning, hardware wallet, multi-sig, etc.).
**Use this guide when:** starting a new area of research that needs a plan, a design, user stories, and an audit. Examples:
- "Plan v0.2 (encrypted mnemonic + Argon2id + AES-256-GCM)"
- "Audit the multi-sig feature and write a plan"
- "Design the Lightning integration"
- "Build a doc stack for the hardware-wallet area"

**Do NOT use this guide when:** implementing features (use the `add-rust-bitcoin-wallet` skill), writing a single doc, or modifying an existing plan.

## The 7 principles (why this scored 8.8/10)

1. **Layer everything.** No single doc stands alone. Every doc cross-references 5+ other docs by path. The 5-layer structure (research → design spec → plan → user stories → audit) is not a suggestion — it's the floor for an 8+/10 rating.
2. **Cite every claim.** A docs.rust.rs URL or a `file:line` reference. No assertions without sources. The 8 audit reports each have a "Sources" section at the end.
3. **Use a verification matrix.** Every audit closes with a matrix that maps features ↔ plan tasks ↔ SDK APIs. The matrix is the single most useful artifact for a future implementer.
4. **Distinguish layers (types vs operations).** `rust-bitcoin 0.32` is the types + primitive operations layer. `bdk_wallet 3.1` is the wallet engine + workflow layer. They overlap only in the type alias `bdk_wallet::bitcoin::*`. The audit caught this and the doc stack makes it explicit per-feature.
5. **Catch anti-patterns upfront.** A "Common pitfalls" section at the top of every index doc captures the 5-10 most likely bugs a contributor will hit. Saves hours of debugging.
6. **Rate yourself honestly.** A per-doc 1-10 score table in the verification report. The 8.8/10 average is verifiable; readers can see which doc scores lower and why.
7. **End with a next-actionable list.** Every doc closes with "what to do next" — Task 31 spike, missing story mapping, deferred v1.1 features. The doc stack is not a static artifact; it's a living checklist.

## The 5-layer doc stack (the floor for 8+/10)

| Layer | Doc file (under `docs/`) | When needed | Scoring impact |
|---|---|---|---|
| **1. Research** | `<area>/<topic>-research.md` | Always for new areas. Skip for small features within an existing area. | If missing, the plan scores 8.0/10 max because reviewers can't verify the foundation. |
| **2. Design spec** | `superpowers/specs/<date>-<feature>-design.md` | For new areas or non-trivial features. Skip for tiny patches. | Without it, the plan body becomes ambiguous because boundary decisions are scattered. |
| **3. Plan** | `superpowers/plans/<date>-<feature>.md` | Always. | The artifact itself. |
| **4. User stories** | `wallets/<date>-<feature>-user-stories.md` | For user-facing features. | Traces acceptance criteria; without it, the Task 31 spike can't validate anything. |
| **5. Audit** | `<area>/<date>-<feature>-audit.md` OR `superpowers/plans/<date>-<feature>-review.md` | For new areas. Skip for small features. | Cross-checks against existing layers. The "use case coverage matrix" lives here. |

**Layering rule:** research → design → plan → user stories → audit. Adding a layer out of order leaves gaps.

## The 7-step process (reproduces 8.8/10)

### Step 1 — Research (if new area)

- Dispatch **N parallel `Agent` subagents** (one per sub-area) via the `Agent` tool. N = number of independent sub-areas. Typically 2-4.
- Each agent writes its own file. Example: `2026-08-05-rust-bitcoin-features-01-core-tx-network.md`, `-02-script-address-consensus.md`, etc.
- Each agent prompt must include: (a) explicit file path to save, (b) explicit format spec, (c) explicit "cite sources" requirement, (d) explicit "include 'what's NOT in this crate'" section.
- After all agents return, write a top-level **index doc** that consolidates the per-part files. The index has a master table of categories + APIs.
- **Cost:** ~$2-5 per agent (parallel keeps wall-clock low).

### Step 2 — Design spec (always for new areas)

- Read the research index. Identify the **2-3 boundary decisions** that need to be locked.
- Write a 13-section spec at `superpowers/specs/<date>-<feature>-design.md`:
  1. Goal & non-goals (be explicit)
  2. Crate layout (3 crates max for a feature like this)
  3. Core dependencies (Cargo.toml block)
  4. Module architecture (public API surface)
  5. Derivation paths / address types (if applicable)
  6. PSBT flow
  7. Data flow (create, send, broadcast — with sequence diagrams)
  8. REST API OR CLI surface
  9. CLI surface
  10. Error handling
  11. Testing
  12. Build, CI, release
  13. Phase plan
- Each section is 80% code, 20% prose. Use the existing design spec as a template.
- Lock the design decisions: pick one approach per section, with rationale.

### Step 3 — Plan

- Open the design spec as the source of truth.
- Save at `superpowers/plans/<date>-<feature>.md`.
- Copy the **Global Constraints** from any existing plan (the 5-7 cross-cutting rules — e.g. "no `unsafe`", "MSRV", "default network testnet", etc.).
- Copy the **File Structure** block (the 2-3 crates' directory tree).
- Add the **Tech Stack** block (only deps that change).
- Add **N tasks** (typically 25-35 for a feature of this scope). Each task has:
  - **Files:** `Create:` / `Modify:` / `Test:` paths
  - **Interfaces:** `Consumes:` / `Produces:` — what earlier tasks must produce before this task can start
  - **Steps:** 5-7 sub-steps (write test, run it fails, implement, run it passes, commit) with actual code examples
- The plan body should be **80% code blocks and 20% prose.**
- Close with: **Self-Review (writer checklist)** + **What's NOT in scope** + **Output of the skill** (commit message format).

### Step 4 — User stories

- Read the plan. For each task, ask: "What user-facing capability does this enable?"
- Save at `wallets/<date>-<feature>-user-stories.md`.
- Use the **"As a [role]..."** format. Each story has 5-10 acceptance criteria.
- Include a **Story → BDK feature map** table (or equivalent for the new area) — this is the traceability matrix.
- Include a **use case coverage matrix** at the end — maps each rust-bitcoin or BDK use case to the story that exercises it. If any use case is uncovered, add a story.

### Step 5 — Audit (always for new areas)

- Audit the plan + user stories + design spec. For each feature/area, produce a per-area audit doc (or one combined doc).
- The audit should:
  - Enumerate the public API surface of each crate used
  - Map every feature to the underlying SDK call
  - Flag what the SDK does NOT do (deferred to higher version)
  - List the 5-10 most likely pitfalls (with the fix for each)
- The audit is the **risk register** for the implementation. It catches bugs BEFORE coding.

### Step 6 — Verification report

- Cross-check the 5 docs against each other. For each user story, find the corresponding plan task. For each plan task, find the corresponding BDK / SDK API. For each SDK API, find the corresponding doc reference.
- Save at `superpowers/plans/<date>-verification-report.md`.
- Use a verification matrix: rows = stories, columns = plan tasks, BDK features, rust-bitcoin features.
- Flag any gaps: stories without tasks, tasks without stories, APIs without docs.
- End with a **per-doc 1-10 rating table** — this is the only way to make the rating visible across the stack.

### Step 7 — Skills + CLAUDE.md update

- After all docs land, create **2 skills** in `.claude/skills/`:
  - `add-<area>` — for implementing features (6 stages: Intent / Rebase / Review / Test / Document / Lint)
  - `plan-<area>` — for writing plans (6 stages: Intent / Layering / Research / Spec / Tasks / Verification)
- Update `CLAUDE.md` with:
  - The doc stack map (the 5-layer structure)
  - The "Common pitfalls" section (5-10 most likely bugs)
  - The "Rust crate rules" (e.g. "use BDK re-exports, don't add standalone secp256k1")
- These capture the methodology for future contributors.

## The 5 templates (copy these)

### Template 1: Per-feature research agent prompt

```text
Deep research task. Goal: enumerate the public API of <crate-name> <version> for <N> modules only: <list>. Save findings to <path>.

Source of truth:
- <docs.rs URL>
- <GitHub repo URL>
- <release-tag URL>

Tools:
- mcp__exa__web_search_exa for general searches
- mcp__firecrawl__firecrawl_scrape for docs.rs pages
- WebFetch for raw URLs

Output format for the file:
1. Summary table per category: function | signature (Rust syntax) | version status | notes
2. For each function: the actual Rust signature, brief docstring, source file:line if findable
3. Section "What's NOT in <crate> <version>" for these <N> categories
4. List of error types in this area
5. Quality bar: cite URL or file:line for every claim
6. If unsure, say "verify in <X> spike" rather than assert
7. Do not invent

Time budget: ~$2 of API calls.
```

### Template 2: Per-doc index (the consolidation doc)

```text
# <Doc-name> — Complete <Category> Surface

**Source:** Live docs.rs (<version>, <month year>) + GitHub source paths in <repo>.
**Companion docs (split for parallel research):**
- [Part 1: <area>](<part1-path>)
- [Part 2: <area>](<part2-path>)
- ...

## TL;DR
<3-5 sentences: what this is, what layer it occupies, what's missing>

## Master index — N categories
<table with category | surface highlights | plan uses it for | re-export?>

## Critical findings (changes plan design)
<3-10 items that affect implementation>

## What's NOT in <crate> <version> (explicit gaps)
<table of missing features + workaround>

## Plan impact (concrete corrections)
<table of plan task | original assumption | corrected>

## Full-task verification list for <Spike>
<25-40 items: every API assumption the implementation depends on>

## Sources
<full URL list>
```

### Template 3: Per-doc rating table (in the verification report)

```text
| File | Score / 10 | Note |
|---|---|---|
| Plan (<path>) | **X.X** | N tasks. Specific note about what's correct + what's stale. |
| Task-SDK map (<path>) | **X.X** | N task entries. Note about which secp256k1 paths are correct. |
| User stories (<path>) | **X.X** | N stories + M traceability matrices. |
| BDK features index (<path>) | **X.X** | N categories + 34-item spike list. |
| rust-bitcoin features index (<path>) | **X.X** | N modules + 8 critical findings. |
| Comparison doc (<path>) | **X.X** | N-row feature map + X% coverage. |
| ADR | **X.X** | Decision + rejected alternatives. |
| Decision doc | **X.X** | 8-method rating + pick per release. |
| Survey doc | **X.X** | N wallets × 9 fields. |
| Deep-dive doc | **X.X** | 2 paths + alternatives. |

**Average: X.X / 10.** <1-paragraph summary of remaining gaps + non-blocking items.
```

### Template 4: Skill (for implementing features)

```text
---
name: add-<area>
description: <what the skill does> + <when to trigger> + <when NOT to use> + <related skill>
---

# <Verb> a <noun> in <area>

**When to use:** <list of use cases>
**When NOT to use:** <list of excluded cases>

## The 6 stages (mandatory)
<pipeline diagram>

| Stage | Output |
|---|---|
| **Intent** | <1 paragraph statement of what the user wants> |
| **Rebase** | <how the new feature fits the existing plan> |
| **Review** | <read the relevant doc-stack layers> |
| **Test** | <TDD: write failing test, implement, run, commit> |
| **Document** | <update user stories + task-SDK map> |
| **Lint** | <cargo fmt, clippy, test> |

## Step-by-step
<Stage 1: Intent> ...
<Stage 2: Rebase> ...
...

## Don't do these
<5 explicit anti-patterns>

## Reference: <Spike> checklist
<25-40 item API verification list>

## Output of the skill
<commit message format + branch naming>
```

### Template 5: Skill (for writing plans)

Same structure as Template 4, but the 6 stages are different:

```text
| Stage | Output |
|---|---|
| **Intent** | <1 paragraph statement of what the user wants> |
| **Layering** | <which doc-stack layers this plan needs> |
| **Research** | <optional: dispatch Agent subagents> |
| **Spec** | <optional: write design spec first> |
| **Tasks** | <write the plan body with bite-sized tasks> |
| **Verification** | <cross-check against the 5 key docs> |
```

## The 5-layer doc stack checklist (run before committing)

For each layer, ask:

| Check | Yes/No |
|---|---|
| **Research**: Does it cover all independent sub-areas? Are sources cited? | |
| **Spec**: Does it have 13 sections? Are boundary decisions locked? | |
| **Plan**: Does each task have Files + Interfaces + Steps with code? | |
| **User stories**: Does each story have 5-10 acceptance criteria? Is there a traceability matrix? | |
| **Audit**: Does it have a "use case coverage matrix" + "what's NOT in the SDK" section? | |
| **Verification report**: Does it have a 1-10 rating per doc? | |
| **Skills**: Are there 2 skills (implement + plan)? | |
| **CLAUDE.md**: Is there a "Common pitfalls" section? | |
| **Cross-references**: Does every doc link to 5+ other docs? | |
| **Pipeline format**: Is the task display rule followed (per CLAUDE.md)? | |

## The 8 anti-patterns (each drops 0.5+ points from the rating)

1. **Code without sources.** A claim like "BDK has `from_mnemonic()`" without a docs.rs URL — drops 0.5 from the audit score.
2. **Missing cross-references.** A plan that doesn't cite any other doc — drops 0.5 from the plan score.
3. **Invented API names.** Using `secp256k1::Keypair::from_secret_key` in the plan when the audit found that the real name is `from_private_key` — drops 1.0 from the plan score.
4. **No Self-Review checklist.** Plan without the "Self-Review (writer checklist)" section — drops 0.5 because placeholder text can leak in.
5. **No Out-of-Scope section.** Plan without explicit deferrals — drops 0.5 because reviewers can't tell what's expected.
6. **Task bodies without code.** "Implement the X feature" without actual code examples — drops 0.5 because the task isn't testable.
7. **Single doc instead of stack.** A plan without user stories, audit, or spec — drops 1.0+ because traceability is impossible.
8. **No rating at the end.** Without a 1-10 per-doc rating, the reader can't tell which doc to improve. Drops 0.5 from the verification score.

## The 6 common pitfalls (also in CLAUDE.md, from the rust-bitcoin audit)

1. **PSBT v2 is NOT in rust-bitcoin 0.32.11.** Use v1 only.
2. **PublicKey::from_secret_key was renamed** to `from_private_key(secp, &PrivateKey::new(sk, network))` in 0.31+.
3. **Sighash newtypes** (`SegwitV0Sighash`, `TapSighash`, `LegacySighash`) need `.to_byte_array()` before `Message::from_digest(*bytes)`.
4. **TxOut::value is `Amount`** (not `u64`) since 0.31. Use `.to_sat()` / `.to_btc()`.
5. **Script constructors live on `ScriptBuf`**, not `Script`. `Script::p2pkh` does NOT exist.
6. **5 sibling error enums** at `bdk_wallet::error::*`, not a single `bdk_wallet::Error` (in 0.x).

## The 10 most-actionable items (after the 8.8/10 doc stack)

For the next similar task (e.g. multi-sig, hardware wallet, Lightning):

1. **Reuse the 5-layer doc stack** as the floor for any new area.
2. **Dispatch parallel agents** for any independent research area (typically 2-4).
3. **Use BDK re-exports** for any Bitcoin feature; don't add standalone `secp256k1`/`miniscript`/`bip39` deps.
4. **Include a use case coverage matrix** at the end of every audit — links every feature to its underlying SDK call.
5. **Include a Self-Review (writer checklist)** at the end of every plan — catches placeholder text.
6. **Include an Out-of-Scope section** at the end of every plan — sets expectations.
7. **Include a per-doc 1-10 rating table** at the end of the verification report — makes the rating visible.
8. **Update CLAUDE.md** with the common pitfalls from the new area's audit.
9. **Create 2 skills** (implement + plan) for the new area — these make the work reproducible.
10. **Close with a Next-Actionable list** — the doc stack is a living checklist, not a static artifact.

## References

- **Source doc stack that scored 8.8/10** (the canonical example):
  - `docs/blockchain-sdks/2026-08-05-tangem-blockchainsdk-rust-sdks.md`
  - `docs/superpowers/specs/2026-08-05-rust-bitcoin-wallet-design.md`
  - `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet.md`
  - `docs/superpowers/plans/2026-08-05-rust-bitcoin-wallet-task-sdk-map.md`
  - `docs/wallets/2026-08-05-btc-wallet-user-stories.md`
  - `docs/wallets/2026-08-05-tangem-vs-btc-wallet-comparison.md`
  - `docs/wallets/2026-08-05-bitcoin-rust-sdks-deep-dive.md`
  - `docs/wallets/2026-08-05-mnemonic-handling-wallet-survey.md`
  - `docs/wallets/2026-08-05-mnemonic-handling-decision.md`
  - `docs/wallets/2026-08-05-feature-sdks-support.md`
  - `docs/wallets/2026-08-05-bdk-wallet-features.md` + 4 part docs
  - `docs/wallets/2026-08-05-rust-bitcoin-features.md` + 4 part docs
  - `docs/superpowers/plans/2026-08-05-adr-0001-signing-model.md`
  - `docs/superpowers/plans/2026-08-05-verification-report.md`
  - `docs/superpowers/plans/2026-08-05-review-2026-08-05.md`
- **Skills that implement this methodology:**
  - `.claude/skills/add-rust-bitcoin-wallet/SKILL.md` (implement features)
  - `.claude/skills/plan-rust-bitcoin-wallet-feature/SKILL.md` (write plans)
- **CLAUDE.md** — the user-facing rules + doc stack map + common pitfalls

## How to use this guide

When you start a similar task:

1. **Print this guide** (or open it in another tab). You'll reference it throughout.
2. **Start with the 7-step process.** Don't skip steps. Each step's output is the input to the next.
3. **Use the 5 templates** as starting points. Adapt them to your area.
4. **Run the 5-layer doc stack checklist** before committing each layer. If a check fails, fix the layer before moving on.
5. **Score yourself with the 1-10 rating** at the end of the verification report. The score is honest; trust it.
6. **Update CLAUDE.md + create skills** at the end. These make the methodology reproducible for the next contributor.

The methodology is general enough to apply to any "audit a library + plan a Rust feature" task. The 8.8/10 rating is the bar to beat.
