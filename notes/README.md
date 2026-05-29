# notes/

Per-phase AI handoff and review artifacts. This directory is tracked project
memory; it is **not** for durable workflow/reference docs (those live in
`markdown/`).

On this project **Claude implements and Codex reviews** (see
[../markdown/FLOW.md](../markdown/FLOW.md)). Each phase produces two files:

| File | Written by | Purpose |
| ---- | ---------- | ------- |
| `codex_handoff_phase_X.txt` | Claude (implementer) | Everything Codex needs to review the phase: git diff instructions (incl. untracked files), the phase prompt / plan section, architecture + decision references, relevant PHASE_LOG entries, commands run and results, known tradeoffs / approved Node divergences, and the explicit review output path. |
| `codex_review_phase_X.txt`  | Codex (reviewer)     | The full review: critical issues, suggested improvements, acceptable tradeoffs, questions/assumptions, and commit readiness. |

Use [../markdown/CODEX_REVIEW_TEMPLATE.md](../markdown/CODEX_REVIEW_TEMPLATE.md)
when preparing the handoff. After review, summarize durable outcomes in
[../markdown/PHASE_LOG.md](../markdown/PHASE_LOG.md) before committing the phase.

`X` is the migration-plan phase number (0, 0.5, 1, 2, …) — see
[../markdown/PROMPTS.md](../markdown/PROMPTS.md).
