# Current State

Milestone: `P0 — Architecture cleanup` (closing out) → `P1 — Decomp
compatibility layer` (next)

Current subsystem/batch: none open. Development process just switched to
batch mode (`AGENTS.md`); no batch has run under the new process yet.

## What was completed

- `P0`: `psp-runtime` split out as the shared PSP-specific library; both
  `psp-game` and `psp-asset-viewer` depend on it instead of duplicating a
  PSP backend. Verified with workspace tests, both EBOOTs, PPSSPP regression
  captures, and a physical-hardware smoke test (`docs/evidence/re/RE-298.md`).
- Pre-batch-mode work (rendering pipeline, asset pipeline, animation,
  collision, physics, movement-state machine, one scoped training-mode
  attack) exists and is tracked per-subsystem in `docs/porting-status.md`.
  It predates `P1`'s formal decomp-type mapping and was not produced under
  batch mode, but is usable foundation for `P1`/`P2`, not throwaway work.

## Immediate next batch

Start `P1`: audit/formalize decomp-type mappings (`GObj`/`DObj`/fighter
state, callbacks/state tables, animation hooks, collision interfaces, game
globals/resources, math/random/timing, asset/data references) against what
`crates/ssb-game`/`crates/ssb-engine` already have, so `P2`'s fighter bulk
port is cheap. Follow the source-port workflow in `AGENTS.md`.

## Real blockers

None. Rendering performance (`P5`) is not a blocker for this or any
gameplay batch.

---

Detailed per-subsystem status: `docs/porting-status.md`. Roadmap:
`PLAN.md`. Evidence index: `docs/evidence/INDEX.md`.
