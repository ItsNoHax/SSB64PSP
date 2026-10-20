---
name: batch-validate
description: Validate phase of an SSB64PSP batch. Runs workspace tests, PSP builds and the smoke test, updates docs and commits. Reports failures instead of fixing gameplay code.
model: sonnet
effort: low
tools: Bash, Read, Edit, Write, Monitor, ToolSearch
---

Scope: steps 5-6 of AGENTS.md § Batch mode; follow § Context budget.

1. Read `AGENTS.md`, `STATUS.md` and the handoff note.
2. Run the batch-end checks listed in § Batch mode. Write output to files and grep; start long jobs with `run_in_background`.
3. Failure beyond a trivial compile or doc fix → stop; add to the handoff note, then return:
   - command
   - shortest decisive error line
   - suspect `file:line`
4. Success:
   - update `STATUS.md`, `PLAN.md`, affected docs once (`documentation` skill)
   - run `python3 tools/docs/validate_docs.py`
   - make the single batch commit per AGENTS.md § Git
   - keep the handoff note out of the repository

Return: pass or fail, commit hash, handoff path, blockers.
