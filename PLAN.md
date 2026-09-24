# Roadmap

Source-port Super Smash Bros. 64 to PSP as a native Rust game. Gameplay is
translated from `ssb-decomp-re` in large subsystem batches. Crate ownership,
reference order and batch rules are in [`AGENTS.md`](AGENTS.md).

This file lists milestones only. Current work is in [`STATUS.md`](STATUS.md);
per-subsystem state is in [`docs/porting-status.md`](docs/porting-status.md).

## Milestones

| ID | Scope | State |
|---|---|---|
| `P0` | Architecture: shared `psp-runtime`, thin `psp-game`, separate `psp-asset-viewer` ([D-044](docs/decisions/D-044.md)) | Done |
| `P1` | Decomp compatibility: `GObj`/`DObj`/fighter state, callbacks and status tables, animation hooks, collision interfaces, globals, math/RNG/timing, asset references | In progress, merged with `P2` |
| `P2` | Fighters: fighter-common code, then complete fighters — movement, attacks, specials, grabs/throws, shield, damage/hitstun/hitlag, knockback, ledges, tech/roll, death/respawn | In progress (Mario, Fox done) |
| `P3` | Match: stage loading, spawning, stocks, blast zones, KO, timer, character/stage select, results/restart | Not started |
| `P4` | Remaining systems: items, CPU AI, effects integration, menus, UI, audio, other modes | Not started |
| `P5` | Fidelity and performance: physical-PSP profiling, measured optimization (VFPU, GU batching, memory), final visual regression, hardware acceptance matrix | Runs in parallel |

`P5` never gates `P1`–`P4`. Optimize only against measured full-game
workloads ([D-032](docs/decisions/D-032.md), [D-036](docs/decisions/D-036.md)).

## Archived process

The earlier `R0`–`R3` rendering gate, `M0`–`M3` foundation and `F1`/`G0`–`G5`
gameplay milestones are kept in `plans/rendering/` and `plans/gameplay/` as
history. Evidence records and code comments still cite those IDs. Do not add
new specs there.
