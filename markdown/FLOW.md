# Development Flow

| File                                   | Purpose                                          |
| -------------------------------------- | ------------------------------------------------ |
| `markdown/siemens_rpp_plan.md`         | The technical plan + source of truth for parity  |
| `markdown/ARCHITECTURE_PRINCIPLES.md`  | What the app fundamentally is                     |
| `markdown/IMPLEMENTATION_RULES.md`     | How implementation work is performed              |
| `markdown/TECHNICAL_DECISIONS.md`      | Durable technical decisions (TD-xxx)              |
| `markdown/PROMPTS.md`                  | Implementation roadmap (the migration phases)     |
| `markdown/REVIEW_CHECKLIST.md`         | Quality gate for review                            |
| `markdown/PHASE_LOG.md`                | Historical memory + reasoning                      |
| `markdown/DEPLOYMENT.md`               | Docker / compose / cron runbook                    |
| `markdown/CODEX_REVIEW_TEMPLATE.md`    | Review handoff template                             |
| `markdown/PHASE_TEMPLATE.md`           | Phase log entry template                            |
| `notes/codex_handoff_phase_X.txt`      | Phase-specific handoff prepared **for** Codex       |
| `notes/codex_review_phase_X.txt`       | Phase-specific review findings **from** Codex        |

This document defines the structured development workflow for `siemens_rpp`.

The goal is to maintain:
- behavioral parity with the Node app being replaced
- architectural consistency
- predictable, reviewable implementation phases
- clean review and testing discipline
- sustainable AI-assisted development

This project intentionally avoids unstructured "vibe coding."

---

# Core Philosophy

This project uses AI tools as:
- implementation assistants
- review assistants
- architecture reinforcement

NOT as autonomous decision-makers.

The human developer remains responsible for:
- architecture
- final approval
- testing
- commits
- the cutover decision

---

# AI Tool Roles

> Roles are **swapped** relative to the source template this scaffolding came from.
> On this project, **Claude implements and Codex reviews.**

## Claude Code Role — Implementation

Claude Code is the implementation tool.

Use Claude for:
- feature implementation
- scaffolding (Cargo workspace, crates, Dockerfile, compose)
- refactors and wiring components together
- porting verified Node behavior into Rust
- writing tests (unit, round-trip, parity, golden)

Claude should:
- stay within the current phase scope
- follow [ARCHITECTURE_PRINCIPLES.md](ARCHITECTURE_PRINCIPLES.md),
  [IMPLEMENTATION_RULES.md](IMPLEMENTATION_RULES.md), and the plan
- preserve the data contract (PG tables, Redis keys, log shapes)
- re-read the Node source rather than guessing at behavior
- avoid introducing unrelated systems or speculative architecture

## Codex Role — Review & Validation

Codex is the review and architecture-validation tool.

Use Codex for:
- reviewing diffs
- identifying parity drift (any observable difference from Node behavior)
- identifying architectural drift and overengineering
- checking SQL safety, error handling, and the Redis/cursor logic
- reviewing timezone/DST correctness
- security review
- validating folder/crate structure

Codex should NOT:
- freely rewrite large sections
- redesign architecture without approval
- override the plan or the architecture documents
- introduce unrelated crates or frameworks

---

# Phases

"Phase" on this project means a step in the **migration plan** (see
[siemens_rpp_plan.md](siemens_rpp_plan.md) "Migration plan" and
[PROMPTS.md](PROMPTS.md)):

| Phase | Focus |
| ----- | ----- |
| 0     | Shared crates `rpp_log` / `rpp_db` / `rpp_redis` |
| 0.5   | Multi-stage Dockerfile + compose file |
| 1     | `siemens_rpp parse` single-system subcommand |
| 2     | Datetime parity test (1000 prod rows) |
| 3     | Shadow run against `*_shadow` tables |
| 4     | First single-SME cutover |
| 5     | Expand to all CT, then MRI |
| 6     | Deprecate the Node `hhm_rpp_siemens` app |

Resolve the blocking Open Decisions in the plan (#1, #5, #6, #8) before the phase
that depends on them.

---

# Phase Execution Flow

Each phase follows the same lifecycle.

## Step 1 — Review Context

Before implementation, read:
- the relevant section of [siemens_rpp_plan.md](siemens_rpp_plan.md)
- [ARCHITECTURE_PRINCIPLES.md](ARCHITECTURE_PRINCIPLES.md)
- [IMPLEMENTATION_RULES.md](IMPLEMENTATION_RULES.md)
- [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md)
- recent [PHASE_LOG.md](PHASE_LOG.md) entries

Confirm: current phase goals, parity constraints, non-goals, and which Open
Decisions (if any) this phase depends on.

## Step 1A — Revalidate Roadmap Alignment

If a decision or the plan changed since a phase was written, decide whether that
phase should be implemented as-is, revised, split, deferred, or dropped. Record
the decision in [PHASE_LOG.md](PHASE_LOG.md) and update
[PROMPTS.md](PROMPTS.md)/the plan so a future session does not blindly resume an
outdated step.

## Step 2 — Create Or Checkout Phase Branch

Use a dedicated branch per phase (the human approves exceptions for tiny changes).

```bash
git status
git checkout main
git checkout -b phase-X-short-name
```

Examples: `phase-0-shared-crates`, `phase-0.5-docker`, `phase-1-parse-subcommand`,
`phase-2-datetime-parity`, `phase-3-shadow-run`.

Do not switch branches with unrelated uncommitted work present unless the human
developer decides how to handle it.

## Step 3 — Create Git Checkpoint

```bash
git status
git add .
git commit -m "checkpoint before phase X"
```

Provides rollback safety and clean diffs. Don't fold unrelated changes into the
checkpoint without confirming they belong to the phase.

## Step 4 — Implementation With Claude

Provide Claude: the phase prompt/plan section, the architecture and
implementation-rule docs, parity constraints, known non-goals, and the expected
verification commands.

Keep implementation scoped to the current phase. Avoid future features,
speculative abstraction, and unnecessary optimization. If the phase conflicts with
the architecture docs or the plan, pause and resolve it before implementing —
durable reference documents and the parity contract win unless the human updates
them.

## Step 5 — Manual Review

Before handing to Codex, review crate/folder structure, naming, env handling, and
dependency changes, then run:

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

plus phase-specific verification (parity harness; `siemens_rpp parse --dry-run`
against a dev SME). Record what was run and what failed.

Prepare the handoff at:

```txt
notes/codex_handoff_phase_X.txt
```

Do not place phase-specific handoffs in `markdown/`. The `markdown/` directory is
for durable workflow/reference documents; `notes/` is for per-phase AI handoff and
review artifacts. Durable outcomes are summarized in
[PHASE_LOG.md](PHASE_LOG.md) before the phase is committed.

## Step 6 — Codex Review

Point Codex at:

```txt
notes/codex_handoff_phase_X.txt
```

The handoff must include:
- git diff instructions, including untracked files when relevant
- the phase prompt / plan section
- architecture, implementation-rule, and technical-decision references
- relevant phase log entries
- commands run and results
- known tradeoffs or intentionally deferred work (and approved Node divergences)
- the explicit review output path

Suggested invocation:

```txt
Please perform a code review for phase X using notes/codex_handoff_phase_X.txt.
Write the full review to notes/codex_review_phase_X.txt, then summarize in chat.
```

Codex reviews: parity, architectural consistency, SQL safety, error handling and
failure isolation, Redis/cursor correctness, timezone/DST correctness,
maintainability, unnecessary complexity, and security. Use
[CODEX_REVIEW_TEMPLATE.md](CODEX_REVIEW_TEMPLATE.md) to prepare the handoff and
[REVIEW_CHECKLIST.md](REVIEW_CHECKLIST.md) as the gate.

Codex writes the full review to `notes/codex_review_phase_X.txt`, including:
1. critical issues
2. suggested improvements
3. acceptable tradeoffs
4. questions or assumptions
5. commit readiness

## Step 7 — Human Decision

The developer decides which findings matter, what is fixed now, what waits, and
what is rejected. Do not blindly apply all AI suggestions.

## Step 8 — Targeted Fixes

Use Claude for approved fixes and scoped cleanup only. Avoid broad rewrites,
architecture redesign, and "while we are here" expansion.

## Step 9 — Verification & Parity Testing

Run the relevant checks for the phase:
- unit + round-trip tests
- the datetime parity test (phase 2+)
- `--dry-run` output diff vs Node output for the same file (phase 1+)
- shadow-table row count/content diff (phase 3)

Confirm architecture still aligns with principles and the data contract still
holds.

## Step 10 — Update PHASE_LOG.md

Document what was built, key decisions, issues, review findings, follow-ups,
tradeoffs, checks performed, deferred work and why, and paths to the handoff and
review notes. Use [PHASE_TEMPLATE.md](PHASE_TEMPLATE.md).

## Step 11 — Commit Phase

```bash
git add .
git commit -m "complete phase X"
```

## Step 12 — Deploy / Cutover When Needed

When a phase needs container testing or a cutover:
- merge the completed phase into `main` and push to the Git remote
- build the image: `docker compose build app_tools`
- run a job: `docker compose run --rm app_tools siemens_rpp ct` (or `mri`)
- for shadow/cutover phases, follow the per-SME ramp in the plan's Migration plan

Use [DEPLOYMENT.md](DEPLOYMENT.md) as the command runbook. Keep the environment
boundary intact: secrets live only in `.env` / the container environment, never in
source or images.

---

# Architectural Guardrails

`siemens_rpp` must remain:
- parity-faithful to the Node app for CT/MRI
- a non-interactive batch job (no UI, no daemon)
- a small, immutable, binary-only container

Avoid drift toward:
- multi-manufacturer frameworks before a second consumer exists
- async/concurrent system processing without a measured need
- changing PG/Redis contracts or log shapes
- implementing deferred scope (`win_7`, `siemens_cv`, CT scan-seconds sidecar)

---

# Persistence Philosophy

- Postgres holds parsed rows and run logs; Redis holds the per-file cursor.
- The cursor is written **last** and **never retried** — duplicate-cursor on the
  next run beats losing committed rows.
- A failure on one system logs and continues; it must not abort the run.
- All SQL is parameterized; bulk insert via `UNNEST` for v1.

---

# Environment Rules

This machine hosts multiple fleet apps.

Rules:
- builds and tooling are project-local; use the workspace `Cargo.toml` / `Cargo.lock`
- the production image is binary-only — no source bind mount, no package install at runtime
- reach Postgres/Redis via the existing docker networks (or `host.docker.internal`
  for native host dev), matching the `data_acquisition` compose pattern
- the file store is mounted read-only at `/opt/resources/acqu_files`

---

# Development Priority Order

1. behavioral parity
2. correctness (SQL, timezone, cursor)
3. failure isolation / resilience
4. maintainability
5. performance optimization

---

# Definition of Done

A phase is complete when:
- requirements are implemented within scope
- `cargo build` / `test` / `clippy -D warnings` / `fmt --check` pass
- phase-specific parity/verification passes (or gaps are documented)
- the data contract still holds
- review findings are addressed or intentionally deferred
- [PHASE_LOG.md](PHASE_LOG.md) is updated
- code is committed cleanly

Not merely when:
- code compiles
- an AI says implementation is complete

---

# Long-Term Goal

A maintainable Rust port that retires `hhm_rpp_siemens` for CT/MRI with zero
downstream disruption, ships as a small static container, and establishes reusable
fleet crates (`rpp_log` / `rpp_db` / `rpp_redis`) and a disciplined AI-assisted
process. This workflow is considered part of the project architecture itself.
