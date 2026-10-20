---
name: documentation
description: Updating project state or documentation (PLAN.md, STATUS.md, DECISIONS.md, evidence records, architecture and subsystem docs). Activates for "update the docs", "record this in STATUS", or after completing a task that needs a documentation update.
---

# Documentation

- Owner table and rules: AGENTS.md § Documentation
- Update the owner, link elsewhere
- Update once per batch, in the batch commit (§ Git)

Procedures:

| Task | Steps |
|---|---|
| New evidence (AGENTS.md § Evidence) | next unused `RE-NNN.md` (check `docs/evidence/INDEX.md`, `docs/evidence/retired.tsv`) → `python3 tools/docs/gen_evidence_index.py` |
| Retire a record | move fact to successor → delete file → add `retired.tsv` row (ID, title, where the fact lives, reason) → regenerate index |
| New decision | next unused `D-NNN.md` + one-line `DECISIONS.md` entry in an existing category |
| Amend decision | edit its own file; superseded → "Superseded by D-NNN", never delete |

- Acceptance criterion change: state change and why, never silent
- Finish: `python3 tools/docs/validate_docs.py`
- Validator checks:
  - pack version and golden count against code
  - no `[x]` under PLAN Remaining Work
  - decision categories
  - inline paths
  - no ROM-derived files
