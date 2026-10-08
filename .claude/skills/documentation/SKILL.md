---
name: documentation
description: Updating project state or documentation (PLAN.md, STATUS.md, DECISIONS.md, evidence records, architecture and subsystem docs). Activates for "update the docs", "record this in STATUS", or after completing a task that needs a documentation update.
---

# Documentation

Update only the owner of each kind of state and link to it elsewhere.

| State | Owner |
|---|---|
| Roadmap, subsystem state, completed and remaining work | `PLAN.md` (the only task list) |
| Current work, blockers, verification baseline | `STATUS.md` |
| Permanent decisions | `DECISIONS.md` + `docs/decisions/D-NNN.md` |
| Architecture, current and target | `docs/architecture.md` |
| Renderer model per domain | `docs/rendering.md` + `docs/rendering/*.md` |
| Memory layout and constraints | `docs/memory.md` |
| Golden scenes and known failures | `docs/visual-regression/README.md` + `tests/golden/scenes.tsv` |
| Durable technical findings | `docs/evidence/INDEX.md` + `docs/evidence/re/RE-NNN.md` |
| Project overview and legal | `README.md` — no progress lists |

Style: short sentences, tables over prose, present tense, current model only.
History belongs in git, not in docs.

Rules:

- `STATUS.md` is replaced, never appended: current, current work,
  blockers, verification. Under 4 KiB. No roadmap or history.
- `PLAN.md` is the only roadmap and task list. Tick or delete a
  "Remaining Work" item when a batch closes it; add new work there, never in
  another file.
- New evidence: only for a durable technical finding. Next unused
  `RE-NNN.md` (check the index and `docs/evidence/retired.tsv`), then
  `python3 tools/docs/gen_evidence_index.py`.
- Retire a record that holds no durable fact: move its fact to its
  successor, delete the file, add a `retired.tsv` row, regenerate the index.
- New decision: next unused `D-NNN.md` plus a one-line `DECISIONS.md` entry.
  Amend an existing decision in its own file; mark a superseded one
  "Superseded by D-NNN" rather than deleting it.
- Architecture docs describe the current and target state, not how they
  evolved.
- Never weaken an acceptance criterion silently; state the change and why.
- Finish with `python3 tools/docs/validate_docs.py`.
