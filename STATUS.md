# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the Poké Ball's thirteen Pokémon, their eight
  weapons, Chansey's eggs and PSP drawing (RE-435, pack v85).
- **Next gameplay batch:** Sword, Bat, Fan, Star Rod, Ray Gun, Fire Flower
  and Hammer: makers and the related swing/shoot/Hammer machinery.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Poké Ball Pokémon | All 13 makers/statuses, weapon callbacks and owner credit, Clefairy selection, Onix rock feedback, joint/material animation; 2 new goldens | RE-435 |
| Throwable utilities | Six makers/statuses, surface attachment, landing root squaring and Green Shell palette mesh | RE-434 |

## Verification baseline

- All 1,685 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread), including 20 Pokémon gameplay tests and 3 ROM-backed tests.
  Clippy, workspace rustfmt, gameplay/ROM no_std and docs validation pass.
- Both production PSP release builds pass. Game has no warnings; viewer
  retains five existing warnings. Production binaries are restored.
- 18 targeted game goldens match twice, including 2 additions, with no
  existing rebaselines (run `20261003-180306`). Manifest: 171 scenes.
  Last full matrix: 169/169 at RE-434 (`20261003-020526`).
- Production PPSSPPHeadless 20 s software smoke finishes without faults.
  Two N64 Training references support qualitative checks; later damage
  and fighter positions diverge. No physical-PSP or N64-equivalence proof.
- Pack v85: 35,447,056 bytes, SHA-256
  `44b9a27f0a6b2dac4914332ae913ced6443ee1717f1b3d5999acd7eae2dadef3`.
- Production `run` frame 29,360; `enter_training` 212,416. Combined
  241,776 of the 256 KiB stack, leaving 20,368 before alignment and
  nested calls. ELF total 3,508,708 bytes. Details: RE-435.
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Seven swing/shoot/Hammer makers make nothing yet. Poké Ball rays and
  open animation, Pokémon status materials/translucency, rock textures,
  FlySparks head-0 state, audio and rumble remain (RE-435, TODO.md).
- The N64 throws the Poké Ball right where the port throws it left
  (RE-434). Pokémon damage/trajectory comparisons need event alignment.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- Bonus stages, pipe traversal/plant notification, selects' spotlight,
  no-save-data unlocks and outstanding stage/rendering comparisons remain
  in TODO.md. Stage performance and VRAM residency belong to `P5`.
