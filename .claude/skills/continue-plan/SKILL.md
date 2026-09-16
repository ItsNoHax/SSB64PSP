---
name: continue-plan
description: Resume or continue SSB64PSP development from repository state. Activates for "continue with the plan", "continue development", "resume current work", "what's next", or /continue-plan.
---

# Continue plan

Progressive-disclosure resume flow for this repository. Follow in order —
do not skip ahead to reading the whole roadmap or evidence corpus.

1. Read `AGENTS.md`.
2. Read `STATUS.md`.
3. Identify the active task ID from `STATUS.md` ("Relevant PLAN task").
4. Read only that task's spec file (`plans/rendering/*.md` or `plans/gameplay/*.md`).
5. Read only the evidence IDs `STATUS.md`/the task spec actually reference —
   look them up in `docs/evidence/INDEX.md`, then open the specific
   `docs/evidence/re/RE-XXX.md` file(s).
6. Load additional individual evidence only if a specific claim needs its
   derivation — never the whole `docs/evidence/re/` directory.
7. Inspect `git status` and recent commits.
8. Resume the `IN_PROGRESS` task if one exists; otherwise select the first
   eligible `TODO` item from the active task's spec or `TODO.md`.
9. Do not scan unrelated roadmap sections, other milestones, or historical
   evidence not referenced by the current task.
10. Before ending: update `STATUS.md` (replace, not append), update the
    affected evidence/task-spec files, and commit focused completed work.

If `STATUS.md` names a blocker, verify it's still real before resuming — do
not silently re-attempt a task recorded as blocked without addressing the
blocker.
