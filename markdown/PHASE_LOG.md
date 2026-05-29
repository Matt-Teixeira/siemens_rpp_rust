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

Current Focus: Phase 0 scaffold is built and the local gate is green. The one
remaining Phase 0 step is writing the live "hello" row into `util.app_run_logs`
against the real DB (needs a container on the app's docker network) — which folds
naturally into Phase 0.5 (Dockerfile + compose). Then Codex review.

---

# Completed Phases

## Phase 0 — Workspace scaffold + shared crates  *(reviewed: approved w/ changes)*

**Status:** Implemented; local gate green; Codex review complete (APPROVED WITH
CHANGES — none blocking the scaffold merge). **DB smoke (the exit criterion) not
yet run.** Branch `phase-0-shared-crates`.

**Built:** the Cargo workspace (kept, not single-crate) with `rpp_log` (run-log
buffer + `util.app_run_logs` row shape, storage-agnostic via a `RunLogSink`),
`rpp_db` (TLS `deadpool-postgres` pool + `PgRunLogSink`), `rpp_redis` (host+port
client + `${sme}.${file}` cursor get/set), and the `siemens_rpp` binary (clap CLI
`ct`/`mri`/`parse`, env config, `hello_run` smoke path). Crate stack pinned in
`Cargo.toml [workspace.dependencies]`; `Cargo.lock` committed.

**Verified (in `rust:1-bookworm` Docker — no host toolchain; invoke `cargo`
directly, a login shell drops cargo from PATH):** `cargo build` 0 warnings ·
`cargo test` 11 passed (rpp_log 7, rpp_db 3, rpp_redis 1) · `cargo clippy
--all-targets -- -D warnings` clean · `cargo fmt --check` clean · `siemens_rpp
--help` lists the subcommands.

**Parity captured from source:** logger event shape `{run_id,dt,type,func,tag,note,
err_msg?}` with UTC `toISOString()` `dt`, `err_msg` only on ERROR, exact
Type/Tag strings, WARN+ERROR filter; `app_run_logs` columns + a fully parameterized
INSERT. **PG TLS resolved from source:** read Node `buildSsl()` —
`disable`/`require`(no-verify)/`verify-*`(CA+verify); `SslMode` mirrors all three,
so the prior "R1 TLS posture" question is settled by parity (live `.env` uses
`require` → encrypted, unverified, same as Node).

**Open / for review** (detail in `notes/codex_handoff_phase_0.txt`):
- Live `util.app_run_logs` hello row not yet written (DB unreachable from the
  build sandbox) — the remaining exit-criterion step, folds into Phase 0.5.
- R2/R3: `run_id` text-vs-uuid bind and `::json` casts — verify vs live schema at
  that run.

**Deviations from plan:** (a) redis dep needed an explicit `aio` feature
(tokio-comp alone didn't pull it); (b) `native-tls` pinned `0.2` (latest), not the
`0.7` I first wrote.

**Review (Codex, commit `ad41f71` → `notes/codex_review_phase_0.txt`):** APPROVED
WITH CHANGES — **2 findings**, both SSL-related, both required before merging the
branch. Both addressed in commit `<this>` and re-gated green (11 tests).

- **Finding 1 (fixed):** `PG_SSLMODE=verify-*` with no `PG_SSL_PATH` silently
  downgraded to encrypted-but-unverified TLS. Node warns then falls back; we were
  silent. Restructured `pool.rs` into a pure `decide_tls(mode, has_ca)` →
  `TlsDecision` and now emit the warning on the `VerifyFallbackNoCa` path. Added 3
  unit tests on the decision (Codex's suggestion).
- **Finding 2 (fixed):** SSL docs disagreed with the code. Corrected the
  `ssl_ca_path` field doc and `.env.example` (CA is used **only** for
  `verify-ca`/`verify-full`, ignored for `disable`/`require`), and documented the
  **intentional** default divergence in `config.rs` (Node defaults a missing
  `PG_SSLMODE` to `disable`; we default to `require` as a safety net since the
  server enforces SSL — the var is always set in the fleet `.env`).
- **Codex non-blocking notes carried forward:** the `rpp_log ← RunLogSink` seam was
  endorsed (not premature); logger row shape / parameterized INSERT confirmed; the
  live-schema checks remain for the hello-row run — `run_id` text-vs-`uuid` bind and
  `json`-vs-`jsonb`/`text` casts (Codex believes `verbose_log`/`warn_error_logs` are
  `json`, so `::json` is plausible, but the live DB is the arbiter). Codex could not
  re-run the Rust gate (no `cargo` on its host) and accepted the Docker results.

**Note (process):** an earlier write-up of this review (commit `40402d8`) wrongly
described a 7-finding "F1–F7" list — that was a confabulation conflating the
handoff's own risk list with the review. The review has only the 2 findings above.
The unrequested tidy-ups made in `40402d8` (clearer `hello.rs` wording, an M7
shutdown TODO, a `with_statement_timeout` doc) are harmless and kept, but were
**not** asked for by Codex; this entry is the corrected record.

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

### Run-log file write independence (Codex F7)

`RunLog::finish` currently INSERTs the row, then writes the file — so a failed
INSERT means no file. Node writes the run-log file even on DB failure
(`writeLogEvents` runs in a finally-ish path). Decide in Phase 1 whether `finish`
should write the file best-effort regardless of the INSERT outcome (likely yes,
for parity + diagnosability). Target: Phase 1.

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

### Phase 0 — Codex, commit `ad41f71` — APPROVED WITH CHANGES

`notes/codex_review_phase_0.txt`. **2 findings**, both SSL: (1) `verify-*` with no
`PG_SSL_PATH` silently downgraded — now warns + falls back, with unit tests;
(2) SSL docs/default divergence — corrected docs, documented the intentional
`require` default. Both fixed + re-gated green (11 tests). The `rpp_log ← RunLogSink`
seam was endorsed (not premature). Live-schema checks (`run_id` text/uuid, `::json`
casts) carried to the hello-row run. (An earlier note miscounted this as 7 findings;
corrected in the Phase 0 entry above.)

---

# Important Lessons

Capture recurring discoveries here. This section should grow throughout
development.

_None yet._
