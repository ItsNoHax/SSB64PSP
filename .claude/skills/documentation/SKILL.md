---
name: documentation
description: Updating project state or documentation (STATUS.md, PLAN.md, TODO.md, DECISIONS.md, evidence records, porting-status.md, subsystem docs). Activates for "update the docs", "record this in STATUS", or after completing a task that needs a documentation update.
---

# Documentation

Update only the owner of each kind of state and link to it elsewhere.

| State | Owner |
|---|---|
| Current batch, blockers, verification baseline | `STATUS.md` |
| Milestones | `PLAN.md` |
| Per-subsystem status | `docs/porting-status.md` |
| Deferred work | `TODO.md` |
| Investigation evidence | `docs/evidence/INDEX.md` + `docs/evidence/re/RE-NNN.md` |
| Permanent decisions | `DECISIONS.md` + `docs/decisions/D-NNN.md` |
| Renderer model per domain | `docs/rendering/*.md` |
| Golden scenes and known failures | `docs/visual-regression/README.md` |
| Project overview | `README.md` — no progress lists |

Style: short sentences, tables over prose, present tense, current model only.
History belongs in evidence records and git.

Rules:

- `STATUS.md` is replaced, never appended: current batch, last completed,
  verification baseline, blockers. Target 1–3 KB.
- New evidence: next unused `RE-NNN.md` (check the index), then
  `python3 tools/docs/gen_evidence_index.py`.
- New decision: next unused `D-NNN.md` plus a one-line `DECISIONS.md` entry.
  Amend an existing decision in its own file; never rewrite or delete it.
- Remove a `TODO.md` entry once a batch or evidence record covers it.
- `plans/` is archived. Do not add or edit specs there.
- Never weaken an acceptance criterion silently; state the change and why.
- Finish with `python3 tools/docs/validate_docs.py`.
