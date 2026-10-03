# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** every CPU trait and the Rush objective, Training's
  menu logic (CP, Item, Speed, View, Reset, Exit, slowed speeds) and no
  CPU while control is locked (RE-438). Pack unchanged (v86).
- **Next gameplay batch:** Training menu presentation: the panel, labels,
  options, cursor, arrows and the stat display with combo counting
  (`sc1PTrainingModeMake*`, TODO.md). The open menu is invisible today.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| CPU modes and Training menu | 11 traits, Rush, menu logic, locked CPUs; 16 regressions, 1 new golden, 12 VS rebaselines | RE-438 |
| CPU item objectives | Item search/track/use, held-item ranges, item threats, weapon `lr` | RE-437 |

## Verification baseline

- All 1,728 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread).
- Clippy with warnings denied, workspace rustfmt, gameplay/ROM no_std and
  docs validation pass.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- Full golden matrix 178/178 matches twice (run `20261003-220548`).
- Production PPSSPPHeadless 20 s software smoke reaches its timeout
  without faults; production requests no screenshot. This and the goldens
  are neither physical-PSP nor N64-equivalence proof.
- Pack v86: 35,447,872 bytes, SHA-256
  `947177fee3cb714ab5d3e792e72315ad1af675776c207eb1a256116ad31be240`.
- Production `run` frame 29,680; `enter_training` 212,488. Combined
  242,168 of the 256 KiB stack, leaving 19,976 before alignment and
  nested calls. ELF total 3,561,136 bytes. Details: RE-438.
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Training's menu does not draw; the world running while it is open,
  the VS entry focus and CPU item behaviour have no N64 comparison yet
  (RE-437, RE-438, TODO.md).
- Item colour animations (including Hammer's warning flash), music,
  arrows, audio and rumble remain. Poké Ball rays/open animation,
  Pokémon materials/translucency, rock textures and FlySparks head-0
  state remain (RE-435, RE-436, TODO.md).
- The N64 throws the Poké Ball right where the port throws it left
  (RE-434). Item damage/trajectory comparisons need event alignment.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- The 1P Game, bonus stages, pipe traversal/plant notification, selects'
  spotlight and no-save-data unlocks remain in TODO.md. Stage performance
  and VRAM residency belong to `P5`.
