# siemens_rpp Review Checklist

Use this checklist for every Codex review. It is the quality gate referenced by
[FLOW.md](FLOW.md) Step 6.

---

# Parity (highest priority)

* Does observable behavior match the Node app `hhm_rpp_siemens`?
* Are the PG target tables and column sets unchanged
  (`log.siemens_ct`, `log.siemens_mri`, `alert.offline_hhm_conn`,
  `log.saved_files`, `util.app_run_logs`)?
* Are NULL-in-Node fields still NULL (`domain_group`, `id_group`, `month`, `day`,
  `year`)?
* Is the Redis key `${sme}.${file_name}` and the raw first-line cursor value
  preserved?
* Is CRLF/LF cursor handling symmetric on read and write (Open Decision #5)?
* Is `host_datetime` / `capture_datetime` formatting byte-identical to luxon
  output, including timezone and DST policy?
* Is every divergence from Node behavior intentional, documented in
  TECHNICAL_DECISIONS.md, and covered by a test or an explicit note?

---

# Correctness

* Boot query mapped to typed rows correctly; `log_config` JSON deserialized safely.
* Correct regex variant selected per `file_config.parsers[0]` (re_v1 vs re_v2).
* Cursor-hit (break) logic stops scanning at the right line.
* Bad/blank-line handling matches the agreed behavior (warn-and-skip vs replicate
  the Node bug — Open Decision #6).
* Timezone fallback to `America/New_York` when `time_zone_id` is NULL.
* `capture_datetime` computed once per run.

---

# Persistence & Ordering

* Bulk insert uses `UNNEST` (or an approved alternative); one round-trip.
* The `offline_hhm_conn` upsert is **parameterized**, not string-interpolated.
* The Redis cursor is written **last** and **not retried** on failure.
* A failure on one system logs and continues — it does not abort the run.
* gzip archival writes the correct `bytea` into `log.saved_files`.

---

# Code Quality

* No dead code, no `TODO` placeholders on production paths.
* No `unwrap`/`expect`/`panic!` on runtime-reachable paths (tests excepted).
* Typed errors (`thiserror`) in crates; `anyhow` context at the binary edge.
* Clear naming; structure matches the plan's project layout
  (`rpp_log` / `rpp_db` / `rpp_redis` + `siemens_rpp`).
* No new dependencies without justification.

---

# Logging

* Persisted events match the `util.app_run_logs` contract (verbose + warn/error
  arrays; one end-of-run INSERT; the file write).
* Event `type` / `tag` use the existing uppercase strings.
* `note` JSON shape matches Node's `JSON.stringify` expectation (Open Decision #4).
* No secrets or sensitive data in logs.

---

# Security

* No hardcoded secrets, hosts, ports, or absolute paths — config-driven.
* All SQL parameterized; no injection surface from `sme`/file names.
* Safe filesystem handling; the file-store mount is treated read-only; no path
  traversal from external input.
* `cargo audit` / `cargo deny` clean (when wired up).

---

# Resilience & Performance

* Missing files handled gracefully (warn + continue).
* Bounded memory on large files; the parse loop stays synchronous and serial for
  v1 unless a decision says otherwise.
* No obvious per-line allocation hot spots.

---

# Containerization (phases 0.5+)

* Multi-stage build; binary-only runtime image (`debian:bookworm-slim`).
* `tzdata` present; `gosu` + entrypoint drop privileges via `RUN_USER`.
* `.env` contract matches the plan: file-store root is `ACQU_FILES_ROOT` (renamed
  from Node's `DATA_STORE_DEV`, same value; TD-015), `NODE_MOD_CACHE_DEV` is
  dropped, and the read-only mount `/opt/resources/acqu_files:ro` is present.

---

# Scope Discipline

Verify the change did **not** introduce, unless the active phase calls for it:

* the `win_7` parser path
* `siemens_cv`
* the CT scan-seconds sidecar
* multi-manufacturer abstractions
* concurrent system processing

---

# Final Verdict

Provide:

1. Summary
2. Strengths
3. Concerns (parity risks called out explicitly)
4. Recommended Fixes
5. Approval Status

Status values:

* Approved
* Approved with Minor Changes
* Requires Rework
