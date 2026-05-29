# Codex Review Request

You are acting as a Senior Software Engineer and Technical Reviewer for
`siemens_rpp` — a Rust port of the Node app `hhm_rpp_siemens` (a cron-triggered
Siemens CT/MRI log parser).

Review the implementation against:

* `markdown/siemens_rpp_plan.md` (the plan + verified Node behavior / source of truth)
* `markdown/ARCHITECTURE_PRINCIPLES.md`
* `markdown/IMPLEMENTATION_RULES.md`
* `markdown/TECHNICAL_DECISIONS.md`
* `markdown/REVIEW_CHECKLIST.md`

Do not rewrite the implementation. Do not introduce alternative frameworks or
crates. Review the implementation that exists.

The overriding concern is **behavioral parity** with the Node app: any observable
difference from Node (PG rows, Redis cursor, log shapes, datetime strings) is a
defect unless it is an intentional, documented divergence.

---

# Stack

* Rust (tokio runtime)
* `tokio-postgres` + `deadpool-postgres`
* `redis`
* `regex`, `chrono` + `chrono-tz`
* `serde` / `serde_json`, `clap`, `dotenvy`, `flate2`
* Packaged as a multi-stage Docker image (`debian:bookworm-slim` runtime)

---

# Phase Under Review

[PHASE NUMBER + NAME]

---

# Objectives

[PASTE PHASE OBJECTIVES]

---

# Files Changed

[PASTE FILE LIST / git diff instructions, including untracked files]

---

# Commands Run + Results

[cargo build / test / clippy / fmt; parity harness; --dry-run output diffs]

---

# Known Tradeoffs / Approved Divergences

[List intentional differences from Node and where they are documented]

---

# Review Requirements

Evaluate, in priority order:

* Parity (PG tables, Redis cursor, log shapes, datetime/DST)
* Correctness (regex selection, cursor-hit logic, NULL fields, timezone fallback)
* Persistence & ordering (UNNEST insert, parameterized upsert, cursor-written-last)
* Failure isolation (one system failing must not abort the run)
* Security (SQL safety, secrets, filesystem)
* Maintainability and scope adherence

---

# Required Output

Write the full review to `notes/codex_review_phase_X.txt`, then summarize in chat.

## Executive Summary

...

## Strengths

...

## Issues

(call out parity risks explicitly)

## Suggested Fixes

...

## Approval Status

Approved
Approved with Minor Changes
Requires Rework
