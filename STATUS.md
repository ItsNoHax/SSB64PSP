# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** CPU item objectives (TrackItem, UseItem), held-item
  attack ranges and item/weapon threats, with `psp-game` reporting items,
  weapons, the Twister, the Zebes acid and own PK Thunder trail to each
  CPU (RE-437). Pack unchanged (v86).
- **Next gameplay batch:** CPU modes: the 1P/team traits, Rush and the
  Training CPU menu (`dSC1PTrainingModeDummyBehaviors`).
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| CPU item objectives | Item search/track/use, held-item ranges, item threats, weapon `lr`; 10 regressions, 1 new golden | RE-437 |
| Held utilities | Seven lifecycles, swings, shooting, Hammer, three projectiles | RE-436 |

## Verification baseline

- All 1,712 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread).
- Clippy with warnings denied, workspace rustfmt, gameplay/ROM no_std and
  docs validation pass.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- All 107 existing `f1-*` goldens match (run `20261003-204834`); new
  `f1-training-cpu-item` matches twice (`20261003-205917`). Manifest: 177
  scenes. Last full matrix: 169/169 at RE-434 (`20261003-020526`).
- Production PPSSPPHeadless 20 s software smoke reaches its timeout
  without faults; production requests no screenshot. This and the goldens
  are neither physical-PSP nor N64-equivalence proof.
- Pack v86: 35,447,872 bytes, SHA-256
  `947177fee3cb714ab5d3e792e72315ad1af675776c207eb1a256116ad31be240`.
- Production `run` frame 29,680; `enter_training` 212,456. Combined
  242,136 of the 256 KiB stack, leaving 20,008 before alignment and
  nested calls. ELF total 3,553,324 bytes. Details: RE-437.
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- CPU item behaviour has no N64 comparison yet (RNG and timing
  dependent; RE-437, TODO.md).
- Item colour animations (including Hammer's warning flash), music,
  arrows, audio and rumble remain. Poké Ball rays/open animation,
  Pokémon materials/translucency, rock textures and FlySparks head-0
  state remain (RE-435, RE-436, TODO.md).
- The N64 throws the Poké Ball right where the port throws it left
  (RE-434). Item damage/trajectory comparisons need event alignment.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- Bonus stages, pipe traversal/plant notification, selects' spotlight and
  no-save-data unlocks remain in TODO.md. Stage performance and VRAM
  residency belong to `P5`.
