# Detailed Agent Protocol

Load only when task needs rules beyond the root bootstrap.

## Batch state

Work proceeds in subsystem batches (`AGENTS.md` BATCH MODE), not per-task
statuses. `STATUS.md` is a replacement snapshot, never an append-only
journal: milestone, current batch, what finished, next batch, real
blockers. Put detailed results in evidence entries, not `STATUS.md`.

## Evidence and documentation

Routine source ports do not need an evidence record. Evidence is required
for ambiguous original behavior, reverse-engineering discoveries, PSP
deviations, important rendering discrepancies, or major architectural
decisions: decompilation, ROM data, display lists, extracted reports,
tests, screenshots, PPSSPP, physical PSP, comparison ports, or numerical
measurements. Update affected documentation once, at the end of the batch.
Do not duplicate long investigations in STATUS or plan bullets.

## Original behavior and references

Reference hierarchy:

1. Original SSB64 decompilation
2. Original ROM/data
3. BattleShip
4. `sf64-psp`
5. `oot-PSP`
6. `n64psp`
7. Existing implementation
8. Engineering assumptions

Reference projects are technical references, not authorities. Do not copy
Nintendo assets or copyrighted data.

## Verification

During a batch: compile/check plus the cheapest relevant targeted tests
only. Do not run the full workspace suite after every small change. At the
end of the batch: workspace tests, PSP build, and one appropriate PPSSPP
smoke/integration test. Physical PSP validation and full visual-regression
matrices run at milestone boundaries or when investigating PSP-specific
behavior, not every batch. Record PSP model, environment, build, pack
version, configuration and observations for hardware validation.

## Git and safety

Inspect status and diff before committing. A batch commit covering dozens
of files is expected — commit the completed batch as a coherent unit, not
one commit per function. Preserve unrelated work. Before destructive
actions, resolve exact targets and prefer recoverable operations. Never
weaken acceptance criteria or delete failing tests to show progress.

## Continuity

Before ending a batch, update STATUS once with milestone, batch, changes,
verification, remaining work, blockers, evidence and commit. Fresh agents
must continue from repository state alone.
