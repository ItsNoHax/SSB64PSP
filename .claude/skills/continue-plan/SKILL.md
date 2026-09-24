---
name: continue-plan
description: Resume or continue SSB64PSP development from repository state. Activates for "continue with the plan", "continue development", "resume current work", "what's next", or /continue-plan.
---

# Continue plan

1. Read `AGENTS.md` and `STATUS.md`.
2. Take the next batch from `STATUS.md`. If it names none, pick the next
   subsystem under the current milestone in `PLAN.md`.
3. Read only the `RE-XXX`/`D-XXX` records `STATUS.md` cites, via
   `docs/evidence/INDEX.md` and `DECISIONS.md`.
4. Check `git status` and recent commits for work in progress.
5. If `STATUS.md` lists a blocker, confirm it still holds before resuming.
6. Run the batch workflow in `AGENTS.md`.
7. Before ending: replace `STATUS.md`, update the affected docs once, and
   commit the batch.

Do not scan unrelated milestones, `plans/` (archived) or evidence the batch
does not need.
