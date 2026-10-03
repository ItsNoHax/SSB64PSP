# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Sword, Bat, Fan, Star Rod, Ray Gun, Fire Flower and
  Hammer makers, swing/shoot/Hammer states, projectiles and PSP drawing
  (RE-436, pack v86). All 20 normal item makers now make their items.
- **Next gameplay batch:** CPU item objectives (`TrackItem`, `UseItem`)
  and their shared item/weapon/hazard view.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Held utilities | Seven lifecycles, 16 swings, four shooting and six Hammer states, three projectiles; 5 new goldens | RE-436 |
| Poké Ball Pokémon | All 13 makers/statuses, weapon callbacks, Clefairy selection, Onix feedback and PSP drawing | RE-435 |

## Verification baseline

- All 1,702 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread): 73 romtool tests in the full run, 1,629 remaining tests
  rerun after correcting the rehit fixture. Includes 16 new gameplay
  regressions and expanded ROM attribute/angle checks.
- Clippy with warnings denied, workspace rustfmt, gameplay/ROM no_std and
  docs validation pass.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- 20 targeted game goldens match twice, including 5 additions; no existing
  rebaselines (run `20261003-192249`). Manifest: 176 scenes.
  Last full matrix: 169/169 at RE-434 (`20261003-020526`).
- Production PPSSPPHeadless 20 s software smoke reaches its timeout
  without faults; production requests no screenshot. This and the goldens
  are neither physical-PSP nor N64-equivalence proof.
- Pack v86: 35,447,872 bytes, SHA-256
  `947177fee3cb714ab5d3e792e72315ad1af675776c207eb1a256116ad31be240`.
- Production `run` frame 29,680; `enter_training` 212,448. Combined
  242,128 of the 256 KiB stack, leaving 20,016 before alignment and
  nested calls. ELF total 3,540,680 bytes. Details: RE-436.
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Item colour animations (including Hammer's warning flash), music,
  arrows, audio and rumble remain. Poké Ball rays/open animation,
  Pokémon materials/translucency, rock textures and FlySparks head-0
  state remain (RE-435, RE-436, TODO.md).
- The N64 throws the Poké Ball right where the port throws it left
  (RE-434). Item damage/trajectory comparisons need event alignment.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- CPU item/mode work, bonus stages, pipe traversal/plant notification,
  selects' spotlight and no-save-data unlocks remain in TODO.md.
  Stage performance and VRAM residency belong to `P5`.
