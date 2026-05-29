# siemens_rpp Phase Template

Copy this into [PHASE_LOG.md](PHASE_LOG.md) for each phase.

## Phase Information

### Phase Number / Name

Phase X — [name]  (maps to a Migration-plan step in the plan)

### Status

* Planned
* In Progress
* Under Review
* Complete

### Date Started

YYYY-MM-DD

### Date Completed

YYYY-MM-DD

### Branch

`phase-X-short-name`

---

# Objective

The primary purpose of this phase. Specific, measurable, independently reviewable.

---

# Scope

## Included

List everything intentionally included.

## Excluded

List everything intentionally deferred (e.g. `win_7`, `siemens_cv`, scan-seconds
sidecar, concurrency).

---

# Open Decisions Resolved / Depended On

| Decision | Status this phase | Resolution |
| -------- | ----------------- | ---------- |
| #X       |                   |            |

---

# Deliverables

Expected outputs (crates, modules, subcommands, tests, Dockerfile, etc.).

---

# Architectural Decisions

## Decision

Describe it.

### Reasoning

Why selected.

### Alternatives Considered

* Alternative A
* Alternative B

(Promote durable decisions into [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md).)

---

# Files Created

| File | Purpose     |
| ---- | ----------- |
| file | description |

# Files Modified

| File | Purpose     |
| ---- | ----------- |
| file | description |

---

# Data Contract Impact

Describe any effect on:

* PG tables / columns (`log.siemens_ct`, `log.siemens_mri`,
  `alert.offline_hhm_conn`, `log.saved_files`, `util.app_run_logs`)
* Redis cursor key/value
* `host_datetime` / `capture_datetime` formatting
* the `util.app_run_logs` event JSON shape

If none:

```txt
No data-contract impact.
```

---

# Parity Notes

* What was checked against the Node app, and how.
* Any intentional divergence from Node (with the TECHNICAL_DECISIONS.md / Open
  Decision reference).

---

# CLI / Subcommand Changes

Describe new/modified/removed subcommands or flags. If none:

```txt
No CLI changes.
```

---

# Testing Performed

## Automated

* `cargo build` / `test` / `clippy --all-targets -- -D warnings` / `fmt --check`
* unit / round-trip / golden tests
* datetime parity test (phase 2+)

## Manual / Verification

* `siemens_rpp parse --dry-run` output diff vs Node (phase 1+)
* shadow-table row diff (phase 3)
* container smoke test (phase 0.5+)

---

# Known Issues

Document remaining issues. If none:

```txt
No known issues.
```

---

# Codex Review

## Review Status

* Approved
* Approved with Minor Changes
* Requires Rework

## Summary

Paste Codex's summary.

## Handoff / Review Artifacts

* `notes/codex_handoff_phase_X.txt`
* `notes/codex_review_phase_X.txt`

## Action Items

List review findings and their disposition.

---

# Lessons Learned

Anything future phases should know.

---

# Next Phase Recommendations

Recommended focus for the next phase.

---

# Final Outcome

Summarize what was accomplished.
