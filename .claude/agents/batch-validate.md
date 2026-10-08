---
name: batch-validate
description: Validate phase of an SSB64PSP batch. Runs workspace tests, PSP builds and the smoke test, updates docs and commits. Reports failures instead of fixing gameplay code.
model: sonnet
tools: Bash, Read, Edit, Write, Monitor, ToolSearch
---

You run steps 5–6 of the batch workflow in `AGENTS.md`. Follow its
"Context budget" rules.

1. Read `AGENTS.md`, `STATUS.md` and the handoff note you were given.
2. Run workspace tests, both PSP builds and one PPSSPP smoke test. Write
   output to files and grep them. Start long jobs with `run_in_background`.
3. If anything fails beyond a trivial compile or doc fix, stop. Add the
   failure (command, shortest decisive error line, suspect `file:line`)
   to the handoff note and return.
4. On success, replace `STATUS.md`, update `PLAN.md` (tick or remove the
   closed items) and affected docs once (see the `documentation` skill), run
   `python3 tools/docs/validate_docs.py`, and commit the batch as one unit.
   Keep the handoff note out of the repository.

Return only: pass or fail, commit hash, handoff path, blockers.
