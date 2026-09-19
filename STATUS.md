# Current State

Milestone: `P0 — Architecture cleanup` (closing out) → `P1 — Decomp
compatibility layer` (next)

Current subsystem/batch: none open. One pre-`P1` cleanup batch (ownership
fix + stale-doc fix) just ran; see below.

## What was completed

- `P0`: `psp-runtime` split out as the shared PSP-specific library; both
  `psp-game` and `psp-asset-viewer` depend on it instead of duplicating a
  PSP backend. Verified with workspace tests, both EBOOTs, PPSSPP regression
  captures, and a physical-hardware smoke test (`docs/evidence/re/RE-298.md`).
- Cleanup batch: `Dummy::apply_hit_from`'s hit detection, damage, knockback,
  hitstun and hit-suppression logic moved out of `psp-game` into
  `ssb_game::attack::apply_hit_from` (`crates/ssb-game/src/attack.rs`);
  `psp-game`'s `Dummy::apply_hit_from` is now a thin call-through, matching
  the crate-ownership rule in `AGENTS.md`. Training-mode Mario-jab behavior
  preserved exactly (workspace tests, `psp-game` release build, PPSSPP
  headless Training capture all pass). Also corrected `README.md` and
  `docs/porting-status.md` claims that rendering blocks gameplay — the
  active process is `P0`–`P5` (parallel tracks), not the archived
  `R0`–`R3` rendering-gate model; archived plans/evidence left untouched.
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
