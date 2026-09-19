# SSB64PSP Development Plan

## Mission

Source-port Super Smash Bros. 64 to PSP as a native Rust game, not an
emulator.

- Gameplay comes primarily from `ssb-decomp-re`, translated in large
  coherent subsystems rather than recreated mechanic-by-mechanic.
- BattleShip informs SSB-specific native-port adaptations.
- `oot-PSP`, `sf64-psp` and `n64psp` inform PSP platform patterns (GU
  rendering, memory, input, timing, audio, cache, asset loading,
  optimization).
- `ssb-game` owns portable gameplay; `ssb-engine` owns reusable
  engine/runtime-independent systems; `psp-runtime` owns PSP-specific
  implementation; `psp-game` stays a thin application; `psp-asset-viewer`
  stays a debugging/render-validation tool, never the game.

See `AGENTS.md` for crate ownership, batch mode rules and the source-port
workflow. This file is an index/roadmap, not a task-by-task journal —
current work lives in `STATUS.md`, per-subsystem detail in
`docs/porting-status.md`.

Rendering performance is **not** a hard blocker for gameplay development.
It is milestone `P5`, profiled against real full-game workloads.

---

## How to use this plan

Development proceeds in batches (`AGENTS.md` BATCH MODE), one coherent
subsystem at a time, within the current milestone below. `STATUS.md` says
which milestone and batch are active now. When resuming work, read
`STATUS.md`, not this whole file end to end.

---

## Reference hierarchy

1. `ssb-decomp-re` (primary gameplay source)
2. Original ROM/data
3. BattleShip — `https://github.com/JRickey/BattleShip`
4. `sf64-psp` — `https://github.com/TheMrIron2/sf64-psp`
5. `oot-PSP` — `https://github.com/z2442/oot-PSP`
6. `n64psp` — `https://github.com/TheMrIron2/n64psp`
7. Existing SSB64PSP implementation
8. Engineering assumptions

References 3–6 are technical references, not authorities (`DECISIONS.md`
D-037). Disagreements get investigated. Do not copy Nintendo assets or
copyrighted game data from reference projects.

---

## Milestones

### P0 — Architecture cleanup

Finalize the `psp-runtime` / `psp-game` / `psp-asset-viewer` split. Ensure
shared PSP code exists only in `psp-runtime`, and gameplay never lives in a
PSP-specific crate.

Status: substantially complete — the shared `psp-runtime` library exists
and both PSP applications depend on it; see `STATUS.md` for the current
state of this milestone.

### P1 — Decomp compatibility layer

Establish mappings for the major SSB decomp types and runtime concepts, so
subsequent decomp translation is cheap:

- `GObj`/`DObj`/fighter state integration
- callbacks/state tables
- animation hooks
- collision interfaces
- game globals/resources
- math/random/timing
- asset/data references

### P2 — Fighter/gameplay bulk port

Port fighter-common code as one subsystem, then port complete fighters (not
isolated moves): movement, attacks, specials, grabs/throws, shield,
damage/hitstun/hitlag, knockback, ledges, tech/roll, death/respawn.

### P3 — Match

Real stage loading, player spawning, stocks, blast zones, KO, match state,
timer, character select, stage select, result/restart loop.

### P4 — Remaining game systems

Items, CPU AI, effects integration, menus, UI, audio, remaining game modes.

### P5 — Fidelity/performance

Profile real matches on physical PSP; optimize measured bottlenecks; VFPU
where justified; GU batching/state optimization; memory optimization; final
visual regression; physical PSP acceptance matrix.

---

## Archived process

The pre-batch-mode process (`R0`–`R3` rendering-correctness gate, `M0`–`M3`
foundation, `F1`/`G0`–`G5` gameplay milestones) is retained as history in
`plans/rendering/*.md`, `plans/gameplay/*.md` and `docs/porting-status.md`'s
Task column. It is not the active tracker — `P0`–`P5` above is. Consult it
only when a specific investigation needs that history.
