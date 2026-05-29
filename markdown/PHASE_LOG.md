# siemens_rpp Phase Log

## Purpose

The permanent historical memory of the project. Every completed phase is recorded
here, preserving architectural decisions, review findings, lessons learned, and
rationale. Future phases reference this document before implementation begins.

Use [PHASE_TEMPLATE.md](PHASE_TEMPLATE.md) for new entries.

---

# Project Timeline

## Current Status

Project Phase: **Planning / pre-scaffold (Phase 0 not yet started)**

Current Focus: the four scaffold-blocking Open Decisions are **resolved** (see
below); next is scaffolding the Cargo workspace and the shared crates (Phase 0).

No code written yet.

---

# Completed Phases

No phases completed yet.

---

# Major Architectural Decisions

These are summarized here for quick reference; the authoritative record is
[TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md) and
[siemens_rpp_plan.md](siemens_rpp_plan.md).

### Replace Node with a Rust port

`siemens_rpp` replaces `hhm_rpp_siemens` (Node) for CT and MRI. The hot path
(per-line regex over large files + tz-aware datetime) suits Rust, and a static
binary deploys to a much smaller container than `node:lts` and drops the per-run
`npm ci` step.

### North star: data-contract parity

v1 keeps the data contract bit-identical (same PG tables, Redis keys,
`util.app_run_logs` shape). Every divergence is deliberate and documented.

### Async runtime only at the I/O edge

`tokio` is used because the PG/Redis clients are async; the parse inner loop stays
synchronous and systems are processed serially for v1.

### Shared fleet crates

`rpp_log`, `rpp_db`, `rpp_redis` live in this workspace now, to be lifted into a
standalone workspace once a second Rust app exists.

### AI roles swapped for this project

Claude implements; Codex reviews. (The scaffolding template originally had these
reversed.)

---

# Technical Debt Register

Document deferred work here.

## Deferred

### `win_7` parser path

Reason: out of v1 scope. Target: Phase 6 (port or move to a `siemens_rpp legacy`
subcommand).

### CT scan-seconds metadata sidecar

Reason: not required for v1. Target: v1.1.

### `siemens_cv`

Reason: deprecated per project memory. Not planned.

---

# Open Decisions

**Resolved 2026-05-29** (source-verified against `~/apps/hhm_rpp_siemens`; full
rationale in [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md) TD-015…TD-019):

- **#1 file path** → `/opt/resources/acqu_files/<system_id>/<file_name>`; `dir_name`
  unused; root exposed as `ACQU_FILES_ROOT` (renamed from Node's `DATA_STORE_DEV`,
  same value). [TD-015]
- **#5 CRLF** → strip trailing `\r` symmetrically (emulate Node `readline`). [TD-016]
- **#6 bad-match** → warn-and-skip; documented divergence (Node throws → aborts the
  system + leaves the cursor un-advanced). [TD-017]
- **#8 invocation** → native subcommand, Option A. [TD-018]
- **naming** → repo `siemens_rpp_rust`, app/`APP_NAME` `siemens_rpp`. [TD-019]
- **#2 date format** (informed) → regex allows single-digit month/day; date+time
  concatenated with no separator — handle in Phase 2. [TD-005]

**Still open** (non-blocking, decided at their phase): #3 DST policy (default:
luxon "earliest"), #4 `note` serialization shape, #7 shadow tables timing, #9
runtime base image (default: slim), #10 image user UIDs. See
[PROMPTS.md](PROMPTS.md) for which phase each gates.

**Corrections found in the plan during source review:** the datetime input has no
date/time separator (`yyyy-MM-ddHH:mm:ss`); the Node `DATA_STORE_DEV` root was not
actually "dropped" (its value is the file-store root) and is carried into the Rust
`.env` as the renamed `ACQU_FILES_ROOT`; both `host_datetime` and
`capture_datetime` use luxon `.toISO()` (with `.000` ms and `±HH:MM` offset). All
folded into the plan and TD-005/TD-015.

---

# Review History

Document Codex review outcomes per phase (link to `notes/codex_review_phase_X.txt`).

_None yet._

---

# Important Lessons

Capture recurring discoveries here. This section should grow throughout
development.

_None yet._
