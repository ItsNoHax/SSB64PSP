# Detailed Agent Protocol

Load only when task needs rules beyond the root bootstrap.

## Task state

Statuses: `TODO`, `IN_PROGRESS`, `BLOCKED`, `VERIFYING`, `COMPLETE`,
`ACCEPTED_DEVIATION`. Completion requires PLAN acceptance criteria plus
recorded evidence; compilation or plausible screenshots alone are insufficient.

`STATUS.md` is a replacement snapshot, never an append-only journal. Keep it
short: task, status, last complete, next task, blockers, verification, evidence,
documentation, and commit. Put detailed results in evidence entries.

## Evidence and documentation

Meaningful implementation needs evidence: decompilation, ROM data, display
lists, extracted reports, tests, screenshots, PPSSPP, physical PSP, comparison
ports, or numerical measurements. Update affected documentation in the same
work cycle. Do not duplicate long investigations in STATUS or plan bullets.

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

Use smallest relevant proof first: targeted test, crate tests, workspace tests,
asset/ROM verification, PPSSPP, physical PSP, then broader CI-equivalent
checks. Record PSP model, environment, build, pack version, configuration and
observations for hardware validation.

## Git and safety

Inspect status and diff before committing. Prefer focused commits. Preserve
unrelated work. Before destructive actions, resolve exact targets and prefer
recoverable operations. Never weaken acceptance criteria or delete failing
tests to show progress.

## Continuity

Before ending active work, update STATUS with current task, changes,
verification, remaining work, blockers, evidence and commit. Fresh agents must
continue from repository state alone.
