# siemens_rpp Implementation Roadmap

Before beginning any phase, review:

- [siemens_rpp_plan.md](siemens_rpp_plan.md) (the plan + verified Node behavior)
- [ARCHITECTURE_PRINCIPLES.md](ARCHITECTURE_PRINCIPLES.md)
- [IMPLEMENTATION_RULES.md](IMPLEMENTATION_RULES.md)
- [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md)
- recent [PHASE_LOG.md](PHASE_LOG.md) entries

At the completion of each phase:

1. Update [PHASE_LOG.md](PHASE_LOG.md)
2. Create the Codex review handoff (`notes/codex_handoff_phase_X.txt`)
3. Complete the Codex review (`notes/codex_review_phase_X.txt`)
4. Address findings
5. Proceed to the next phase

Do not begin a future phase until the current phase has been reviewed.

---

# The Phases

These mirror the "Migration plan" in [siemens_rpp_plan.md](siemens_rpp_plan.md).
Each phase is one branch and one Codex review.

## Phase 0 — Shared crates

Build `rpp_log`, `rpp_db`, `rpp_redis`. Smoke-test by writing one "hello" run into
`util.app_run_logs`.

**Exit:** one Rust-produced row visible alongside Node rows in `util.app_run_logs`.

## Phase 0.5 — Container

Multi-stage Dockerfile + compose file (mirroring `data_acquisition`).

**Exit:** `docker compose run --rm app_tools siemens_rpp --help` exits 0; a "hello"
run from inside the container produces both a file in `/opt/run-logs/siemens_rpp/`
and a row in `util.app_run_logs`; the run-logs dir is writable by `RUN_USER`.

## Phase 1 — `parse` subcommand

Build `siemens_rpp parse` (single-system mode) and wire it against one dev SME in
`--dry-run`. Covers boot-row mapping, path building, file scan, regex selection,
row construction, and the persistence calls.

**Exit:** output rows match Node output rows for the same file, byte-equal except
`host_datetime` (validated in Phase 2).

## Phase 2 — Datetime parity

Implement `time::host_datetime(...)`; run the parity test over ~1000 prod rows
through both luxon and the Rust function.

**Exit:** 100% match (or DST-ambiguous rows isolated and the policy chosen per Open
Decision #3).

## Phase 3 — Shadow run

Cron the binary in parallel with Node, writing to `log.siemens_ct_shadow` /
`log.siemens_mri_shadow` and a `shadow.`-prefixed Redis namespace. ~1 week.

**Exit:** row count + content diffs vs the prod tables are zero (or explainable).

## Phase 4 — First cutover

Move one SME to Rust: exclude it from the Node boot query, include it in Rust; the
Redis cursor for that SME is owned by Rust.

**Exit:** one week stable.

## Phase 5 — Expand

Ramp the remaining CT systems, then MRI.

**Exit:** all `siemens_ct` + `siemens_mri` SMEs on Rust; Node no longer invoked for
CT/MRI.

## Phase 6 — Deprecate Node

Retire `hhm_rpp_siemens` (resolve `win_7`: port it or move it to a
`siemens_rpp legacy` subcommand).

**Exit:** Node cron entries and `npm run` scripts removed.

---

# Open Decisions

Resolved decisions are in [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md)
(TD-015…TD-019); the still-open ones are decided at their phase.

| Decision | Needed by | Status |
| -------- | --------- | ------ |
| #1 file path shape | Phase 1 | ✅ `/opt/resources/acqu_files/<system_id>/<file_name>` (TD-015) |
| #5 CRLF vs LF cursor handling | Phase 1 | ✅ strip trailing `\r` symmetrically (TD-016) |
| #6 bad-match behavior | Phase 1 | ✅ warn-and-skip; divergence (TD-017) |
| #8 invocation syntax | Phase 0.5 | ✅ native subcommand, Option A (TD-018) |
| #2 date format strictness | Phase 2 | ◐ regex allows single-digit m/d; no date/time separator (TD-005) |
| #3 DST ambiguous-hour policy | Phase 2 | ☐ default: luxon "earliest" |
| #4 `note` serialization shape | Phase 0 | ☐ |
| #7 shadow tables now vs migration time | Phase 3 | ☐ |
| #9 runtime base image | Phase 0.5 | ☐ default: `debian:bookworm-slim` |
| #10 image user UIDs | Phase 0.5 | ☐ |

---

# Writing Phase Prompts

Each phase prompt should:
- reference the relevant plan section and the architecture/implementation docs
- state scope and explicit non-goals
- name the parity constraints in play
- list the Open Decisions it depends on (and confirm they're resolved)
- name the expected verification (commands, parity harness, `--dry-run` diffs)

Per-phase handoff and review artifacts live in `notes/`, not `markdown/`.
