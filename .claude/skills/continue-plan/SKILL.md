---
name: continue-plan
description: Resume or continue SSB64PSP development from repository state. Activates for "continue with the plan", "continue development", "resume current work", "what's next", or /continue-plan.
---

# Continue plan

1. Read `AGENTS.md` and `STATUS.md`.
2. Take the current work from `STATUS.md`. If it names none, pick the next
   unchecked item under the in-progress milestone in `PLAN.md`'s
   "Remaining Work".
3. Read only the `RE-NNN`/`D-NNN` records that item cites, via
   `docs/evidence/INDEX.md` and `DECISIONS.md`.
4. Check `git status` and recent commits for work in progress.
5. If `STATUS.md` lists a blocker, confirm it still holds before resuming.
6. Run the batch workflow in `AGENTS.md`.
7. Before ending: replace `STATUS.md`, update `PLAN.md` (tick or remove the
   closed items, add any new ones) and the affected docs once, and commit
   the batch.

## Orchestrating

When asked to orchestrate, do no batch work yourself; only dispatch and
review. Per batch, run the phase agents in order: `batch-research`,
`batch-implement` (repeat while the handoff lists remaining work),
`batch-validate`. Each prompt gives the batch and the handoff note path
(`<scratchpad>/<batch>-handoff.md`, outside the repository). On a validate
failure, send the note back to `batch-implement`. Run
`tools/token-report.py` after each batch.

Never hand batch work to `general-purpose`. A fix or investigation outside
the plan (crash, hardware fault, performance) uses the same phases:
`batch-research` to locate the cause, `batch-implement` for the fix,
`batch-validate` to check and commit. When one agent reports more work,
start a new agent with its handoff note rather than continuing it with
`SendMessage`; continuing re-bills its whole context.

A running session keeps the skill text it already loaded and the
settings it started with (`autoCompactWindow` included). After a change
to `.claude/settings.json` or a skill, tell the user to restart the
orchestrating session.

Do not scan unrelated `PLAN.md` sections or evidence the batch does not
need.
