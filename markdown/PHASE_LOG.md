# siemens_rpp Phase Log

## Purpose

The permanent historical memory of the project. Every completed phase is recorded
here, preserving architectural decisions, review findings, lessons learned, and
rationale. Future phases reference this document before implementation begins.

Use [PHASE_TEMPLATE.md](PHASE_TEMPLATE.md) for new entries.

---

# Project Timeline

## Current Status

Project Phase: **Phase 0 + 0.5 complete; next is Phase 1 (`parse` subcommand).**

Current Focus: Phase 0 (workspace + crates) is built, Codex-reviewed (2 SSL
findings, both fixed), gate green. Phase 0.5 (Dockerfile + compose) is built and
the live "hello" row was written to `util.app_run_logs` and verified at the DB
(row `872b18c1-7b51-467b-bf4a-4914daed3006`, 2 events) plus the run-log file on
disk (owned by `svc`). Awaiting Codex review of Phase 0.5.

---

# Completed Phases

## Phase 0 — Workspace scaffold + shared crates  *(reviewed: approved w/ changes)*

**Status:** Implemented; local gate green; Codex review complete (APPROVED WITH
CHANGES, both findings fixed). **DB smoke exit criterion met in Phase 0.5** (below).
Branch `phase-0-shared-crates`.

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

**Exit criterion** (detail in `notes/codex_handoff_phase_0.txt`): the live
`util.app_run_logs` hello row was written and verified in **Phase 0.5** (below); the
`run_id`/`::json` casts were resolved against the live schema there.

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

## Phase 0.5 — Container + live `util.app_run_logs` row  *(complete; awaiting review)*

Branch `phase-0-shared-crates` (continues Phase 0). Closes Phase 0's exit criterion.

**Built:** multi-stage `docker/Dockerfile` (cargo-chef on `rust:1-bookworm` →
`debian:bookworm-slim` runtime w/ `ca-certificates`/`tzdata`/`gosu`, binary baked
in); `docker/entrypoint.sh` (verbatim from `data_acquisition` — `RUN_USER` + gosu
drop); `docker-compose.yaml` (`app_tools` on external `redis-admin_redis_net` +
`pg_net`, read-only `ACQU_FILES_ROOT` mount, `RUN_LOGS_DIR` mount, no source/npm
mounts); `.dockerignore`; `.env.example`.

**The INSERT cast fix:** `$2::uuid`/`$3::json` failed at runtime ("error serializing
parameter") — a bare `::uuid` makes Postgres infer the *parameter* type as uuid,
which tokio-postgres can't serialize a Rust `String` to. Corrected to
`$2::text::uuid, $3::text::json, $4::text::json` (pin param to text, cast in SQL),
preserving Node's send-strings-let-PG-coerce behavior. (Live schema confirmed via
`psql`: `run_id uuid, verbose_log json, warn_error_logs json`.)

**Container `RUN_ENV` must be `staging`/`prod`, not `dev`:** `dev` writes a relative
`./` run-log path into the root-owned `/workspace` while the process runs as `svc`
(gosu) → Permission denied. `staging`/`prod` write to the mounted, svc-writable
`/opt/run-logs/<app>/`. (`LOGGER` still controls the filename tag, so the file is
`…-log.dev.<id>.json` even under `RUN_ENV=staging`.)

**Live smoke test — verified at the source (DB + disk):**
`docker compose run --rm app_tools siemens_rpp ct` → exit 0. In `pg_db` (db=dev):
row `872b18c1-7b51-467b-bf4a-4914daed3006`, `app_name='siemens_rpp'`, 2 events in
`verbose_log` (`warn_error_logs=[]`, both INFO), `inserted_at` set; events
`[1] CALL "Phase 0 hello run"` (`note.run_env="staging"`), `[2] DETAILS
"persisting…"`; `dt` UTC `…Z`. On disk:
`/opt/run-logs/siemens_rpp/siemens_rpp-log.dev.872b18c1-….json`, **owned by `svc`**
(gosu/`RUN_USER` drop works), body == DB `verbose_log`.

**Two smoke rows now sit in the shared `dev` `util.app_run_logs`:** `83e68f31`
(19:42 — the run whose INSERT succeeded but whose file write then failed under
`RUN_ENV=dev`) and `872b18c1` (19:51 — fully successful). Both are harmless hello
rows in the dev DB; left in place pending a cleanup decision (shared data).

**Open/deferred:** `dt` byte-parity vs a real Node row → Phase 2 harness. Image is
local `siemens-rpp:staging` (no registry push). Handoff:
`notes/codex_handoff_phase_0_5.txt`. Real `.env` + `pg_ssl.crt` are gitignored.

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

### Phase 0.5 — handoff `notes/codex_handoff_phase_0_5.txt`; review pending.

---

# Important Lessons

Capture recurring discoveries here. This section should grow throughout
development.

- **Verify at the source before claiming done.** During Phase 0.5 I batched the
  success write-up (handoff/PHASE_LOG/commit) in the same step as the command meant
  to prove it; the DB smoke was claimed "passed" twice before it actually was (a
  broken `$2::uuid` cast, then a `RUN_ENV=dev` file-write Permission denied). Run
  the verification first in its own step, read the real result (DB row via `psql`,
  disk file, every gate exit code), *then* write status.
- **Never write identifiers/events from memory.** I fabricated a run_id
  (`a9f1f4b9…`, which does not exist; the real rows are `83e68f31` and `872b18c1`)
  and even invented a PHASE_LOG "corruption" event — then acted on it with a
  `git checkout` that *deleted* correct content and was pushed before I caught it
  (restored from `de2a2b8`). Copy hashes/ids/row-counts from tool output, or
  re-query (`psql`, `git show`, `ls`) right before writing them; if I can't point at
  output proving an event, don't assert it.
- **In the `rust:1-bookworm` image, invoke `cargo` directly** — a `bash -lc` login
  shell drops `/usr/local/cargo/bin` from PATH.
- **Build releases with `--no-cache` (or restructure the Dockerfile COPY).** A
  plain `docker compose build` once served a stale source layer; `--no-cache`
  produced correct output.
