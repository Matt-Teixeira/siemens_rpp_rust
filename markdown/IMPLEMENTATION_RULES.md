# siemens_rpp Implementation Rules

## Purpose

This document defines how implementation work should be performed on `siemens_rpp`.

**Claude Code is the implementation tool for this project.** Claude should read
this file, [ARCHITECTURE_PRINCIPLES.md](ARCHITECTURE_PRINCIPLES.md), and the
relevant section of [siemens_rpp_plan.md](siemens_rpp_plan.md) before making code
changes. Codex performs review (see [FLOW.md](FLOW.md)).

The goal is a maintainable Rust port that is **bit-identical in behavior** to the
Node app `hhm_rpp_siemens` for CT and MRI.

---

# Core Rules

## 1. Parity Is the Default

When in doubt, do what the Node app does. The verified Node behavior is documented
in [siemens_rpp_plan.md](siemens_rpp_plan.md) ("Verified current behavior") with
file:line citations into `~/apps/hhm_rpp_siemens`. Re-read the source rather than
guessing.

Any intentional divergence (e.g. fixing the bad-match crash, parameterizing the
`offline_hhm_conn` upsert) must be:

* called out in the phase handoff,
* recorded in [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md),
* covered by the parity harness or an explicit note on why parity can't apply.

## 2. Preserve the Data Contract

Do not change, without an explicit decision:

* Postgres tables / column sets (`log.siemens_ct`, `log.siemens_mri`,
  `alert.offline_hhm_conn`, `log.saved_files`, `util.app_run_logs`).
* Redis key shape `${sme}.${file_name}` and the raw first-line cursor value.
* The `util.app_run_logs` event JSON shape and `host_datetime` / `capture_datetime`
  string formats.

Fields that are NULL in Node today (`domain_group`, `id_group`, `month`, `day`,
`year`) stay NULL — replicate `mapDataToSchema` filling missing keys with null.

## 3. No Silent Scope Expansion

Implement only the current phase (see [PROMPTS.md](PROMPTS.md)). Do not add:

* the `win_7` parser path (deferred),
* `siemens_cv` (deprecated),
* the CT scan-seconds sidecar (v1.1),
* multi-manufacturer abstractions,

unless the active phase calls for it. Discovered future work gets documented in
[PHASE_LOG.md](PHASE_LOG.md), not implemented.

## 4. Prefer Small, Reviewable Changes

One phase per branch. Each change must be independently reviewable by Codex. Do
not bundle unrelated changes or "while we're here" refactors.

## 5. Dependencies Require Justification

The crate stack is fixed in [siemens_rpp_plan.md](siemens_rpp_plan.md) ("Crate
stack") and [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md). Adding a crate
requires documenting: what it solves, why a chosen crate or std doesn't, and that
it is maintained. Prefer the already-selected crates (`tokio`, `tokio-postgres` +
`deadpool-postgres`, `redis`, `regex`, `chrono` + `chrono-tz`, `serde`/`serde_json`,
`clap`, `dotenvy`, `flate2`, `uuid`, `thiserror`/`anyhow`, `tracing`).

## 6. Errors: Typed in Libraries, Contextual at the Edge

Use `thiserror` for per-stage error types in the crates; use `anyhow` for
top-level context in the binary. A failure on one system logs and continues — it
must not abort the run. Never `unwrap()`/`expect()`/`panic!` on a
runtime-reachable path (tests excepted).

## 7. Logging Goes Through `rpp_log`

Persisted logs are the source of truth and must match the Node `util.app_run_logs`
contract: in-memory event array → one end-of-run INSERT (verbose + warn/error)
plus the file write to `/opt/run-logs/${APP_NAME}/...`. Event `type` and `tag`
stay the existing uppercase strings (`INFO`/`WARN`/`ERROR`,
`CALL`/`DETAILS`/`CATCH`/...). A `tracing` layer may sit on top for dev
ergonomics, but the buffered events are authoritative.

## 8. File & Path Safety

* No hardcoded absolute paths — build paths from config under
  `/opt/resources/acqu_files/` via the `PathBuilder` strategy (Open Decision #1).
* Handle missing files gracefully (warn + continue, as Node does).
* Mounts are read-only for the file store; do not write back into it.

## 9. SQL Safety

Parameterize all SQL. Do **not** string-interpolate `sme` or other values into
queries (the Node `offline_hhm_conn` upsert does this; the Rust port fixes it —
this is an approved, documented divergence). Bulk insert via `UNNEST` for v1.

## 10. Cursor Handling Is the Top Parity Risk

CRLF vs LF on the cursor compare drives correctness (Open Decision #5). Whatever
the resolution, apply it **symmetrically** on read and write. Store the cursor
value raw — no trim/normalize/re-encode beyond the agreed CRLF handling.

## 11. Time Correctness Is Centralized

All datetime construction goes through one `time::host_datetime(...)` function so
the format string, timezone fallback (`America/New_York`), and DST policy live in
one place. Match luxon output byte-for-byte; the datetime parity test gates
cutover.

## 12. Do Not Leave TODO-Driven Code

No unfinished `TODO`s on production paths. Intentionally deferred work is recorded
in [PHASE_LOG.md](PHASE_LOG.md) (Technical Debt Register), not left as a comment.

## 13. Verify Before Handoff

Before handing a phase to Codex, run and record results for:

* `cargo build`
* `cargo test`
* `cargo clippy --all-targets -- -D warnings`
* `cargo fmt --check`

plus any phase-specific verification (parity harness, `--dry-run` against a dev
SME). Record what was run and what failed in the handoff and
[PHASE_LOG.md](PHASE_LOG.md).
