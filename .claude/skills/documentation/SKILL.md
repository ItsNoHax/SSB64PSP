---
name: documentation
description: Updating project state or documentation (STATUS.md, PLAN.md, TODO.md, DECISIONS.md, evidence records, porting-status.md, subsystem docs). Activates for "update the docs", "record this in STATUS", or after completing a task that needs a documentation update.
---

# Documentation

Canonical ownership — update only the owner, plus links/references that
genuinely need to change:

| Kind of state | Canonical owner |
|---|---|
| Current mutable execution snapshot | `STATUS.md` (replace, never append) |
| Ordered roadmap/index | `PLAN.md` |
| Task specifications/acceptance criteria | `plans/rendering/*.md`, `plans/gameplay/*.md` |
| Inactive discovered future work | `TODO.md` |
| Permanent architectural decisions | `DECISIONS.md` (index) + `docs/decisions/D-XXX.md` |
| Investigation evidence/history | `docs/evidence/INDEX.md` + `docs/evidence/re/RE-XXX.md` |
| Concise subsystem status | `docs/porting-status.md` |
| Current technical model per rendering domain | `docs/rendering/*.md` |
| User-facing project overview | `README.md` |

Rules:

- Never duplicate state across files — link to the owner instead.
- `STATUS.md` is a replacement snapshot: task, status, last complete, next
  task, blockers, verification baseline, evidence, docs, commit. Target
  1-3 KB. Detailed narrative belongs in an evidence record, not here.
- New evidence: create `docs/evidence/re/RE-NNN.md` (next unused ID, check
  `docs/evidence/INDEX.md`), then run
  `python3 tools/docs/gen_evidence_index.py` to refresh the index.
- New decision: create `docs/decisions/D-NNN.md` (next unused ID) and add a
  one-line entry to `DECISIONS.md`'s index. If new evidence amends an old
  decision, add the amendment to that same `D-XXX.md` — do not reinterpret
  or delete the original decision.
- Do not move completed work from `TODO.md` into a "done" archive there —
  once it's covered by a task spec or evidence record, delete the `TODO.md`
  entry.
- Never weaken or silently rewrite an existing acceptance criterion; if it
  needs to change, say so explicitly and record why.
