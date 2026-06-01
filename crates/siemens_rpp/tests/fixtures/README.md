# Test fixtures

Captured/crafted log inputs for offline, deterministic parser tests. The live
Siemens `Application.log` files rotate (turn over ~daily and accumulate during the
day), so tests must not read them directly — they use these committed fixtures.

## `sample_ct.log`

A small **CRLF** win_10 CT log crafted to exercise every Phase 1 branch:

- 3 valid `re_v1` lines → 3 rows,
- 1 blank line → skipped silently (Node `blankLineTest`),
- 1 non-blank unmatched line → warn-and-skip (TD-017), not a crash.

Line shape matches the real `SME21862/Application.log` format observed on the dev
host (e.g. `I⇥2026-06-01⇥08:44:29⇥CT_MCU⇥3119⇥Control info MCU (...)`).
