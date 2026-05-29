# siemens_rpp Architecture Principles

## Mission

`siemens_rpp` is a Rust port of the Node app `hhm_rpp_siemens`: a cron-triggered
batch job that, for each Siemens system in a Postgres-driven worklist, parses the
system's most recent CT or MRI log file and persists the parsed rows, an offline
tracker, a Redis cursor, a gzip archive of the file, and a run-log record.

The application is a backend data pipeline. It has **no UI**, runs **non-interactively**
inside a container, and is invoked per-modality (`ct` / `mri`) on a schedule.

---

# North Star: Data-Contract Parity

The single most important property of v1 is **bit-identical behavior with the Node
app it replaces.**

The port must preserve, byte-for-byte where observable:

* The Postgres target tables and column sets (`log.siemens_ct`, `log.siemens_mri`,
  `alert.offline_hhm_conn`, `log.saved_files`, `util.app_run_logs`).
* The Redis cursor key shape (`${sme}.${file_name}`) and value (the file's first
  line, stored raw).
* The `util.app_run_logs` JSON shape (verbose + warn/error event arrays).
* `host_datetime` / `capture_datetime` string formatting and timezone behavior.

Any intentional deviation from Node behavior is a decision recorded in
[TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md) and the relevant Open Decision in
[siemens_rpp_plan.md](siemens_rpp_plan.md) — never an accident.

---

# Core Architectural Principles

## 1. Parity Before Cleverness

When the Node behavior and a "better" design conflict, parity wins for v1.
Improvements (binary `COPY`, parameterized SQL, warn-and-skip on bad matches) are
made deliberately, documented, and validated against the parity harness — not
slipped in.

## 2. Rust Owns the Whole Pipeline

There is no frontend. Rust owns:

* the boot query and worklist materialization,
* file scanning and regex parsing,
* timezone-aware datetime construction,
* Postgres persistence and Redis cursor management,
* gzip archival,
* run-logging.

Business rules live in typed Rust, not in shell, SQL strings, or config.

## 3. Postgres + Redis Are the Systems of Record

Postgres holds parsed data and run logs; Redis holds the per-file cursor. Do not
introduce SQLite, other datastores, message queues, or caches without an explicit
decision in [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md). The existing
connection contracts (no auth/TLS Redis; pooled `tokio-postgres`) are preserved.

## 4. The Inner Parse Loop Stays Synchronous

`tokio` exists only because the Postgres and Redis clients are async. The
per-line, CPU-bound parse loop stays synchronous and, in v1, systems are
processed serially to match Node ordering. Concurrency is added later, behind a
decision, only if a measured need exists.

## 5. Idempotency & Safe Failure

* The Redis cursor is written **last** and is **never retried** — committed rows
  plus a duplicate-cursor next run is preferable to losing rows.
* A failure on one system must not abort the whole run; log it and continue.
* Re-running a job must not corrupt state.

## 6. Simplicity Over Abstraction

Prefer explicit, predictable code. Avoid premature generalization (e.g.
multi-manufacturer frameworks) until a second real consumer exists. The shared
crates (`rpp_log`, `rpp_db`, `rpp_redis`) are the only forward-looking
abstraction, and only because the fleet will reuse them.

## 7. Configuration, Not Hardcoding

Ports, hosts, paths, credentials, and the file-store root come from the `.env`
contract. No secrets or absolute paths in source. The fixed resource root is
`/opt/resources/acqu_files/` (see Open Decision #1 for the per-system path shape).

## 8. Immutable, Minimal Runtime Image

Production runs a binary-only image (no source mount, no `npm ci`). The build is
multi-stage; the runtime is `debian:bookworm-slim` with `tzdata` + `gosu`. See
[DEPLOYMENT.md](DEPLOYMENT.md).

## 9. Scope Discipline (v1)

In scope: **CT and MRI parsers only.** Explicitly out of scope for v1:

* `siemens_cv` (deprecated per project memory),
* the `win_7` parser path (deferred),
* the CT scan-seconds metadata sidecar (deferred to v1.1).

Do not implement deferred paths without a phase that calls for them.

---

## Definition of Success

`siemens_rpp` is successful when it replaces `hhm_rpp_siemens` for CT and MRI with
zero observable change to downstream consumers (dashboards, alert tables, log
tables), deploys as a small static container, and removes the per-run `npm ci`
cost — all proven by the datetime parity test and the shadow-table comparison
described in [siemens_rpp_plan.md](siemens_rpp_plan.md).
