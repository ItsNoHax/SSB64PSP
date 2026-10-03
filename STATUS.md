# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Training menu presentation and DAMAGE/COMBO/
  ENEMY/SPEED/held-item stats, with source hit accounting (RE-439).
  Pack rebuilt to v87.
- **Next gameplay batch:** battle player tags, off-screen arrows and the
  magnifier (`ifCommonPlayer*`); wire Training's View visibility state.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Training menu presentation | Panel, labels/options, cursor/arrows/underline, stats and combo counting; 8 regressions, 1 new golden, 84 UI rebaselines | RE-439 |
| CPU modes and Training menu | 11 traits, Rush, menu logic, locked CPUs | RE-438 |

## Verification baseline

- All 1,736 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread); resistance-overflow regression included.
- Clippy with warnings denied, workspace rustfmt, gameplay/ROM no_std
  and docs validation pass. Pack strict check: zero unresolved.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- 89 targeted Training goldens match and repeat exactly; the manifest
  has 179 scenes. Final runs and pixel deltas: RE-439. No full matrix
  run in this batch; previous full baseline is RE-438.
- Production PPSSPPHeadless 20 s software smoke reaches its timeout
  without faults; production requests no screenshot. This and the
  goldens are neither physical-PSP nor N64-equivalence proof.
- Pack v87: 35,778,144 bytes, SHA-256
  `c5727de42ec3b91c2022998c3adc16c7e35a8b748348eec2ca79d5a130f55ac2`.
- Production stack/ELF measurements: RE-439. Physical PSP last checked
  RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Training's menu and stats draw. Original-game captures confirm that
  items and stage effects advance while it is open. Player tags/arrows,
  the magnifier and menu audio remain; VS entry focus and CPU item
  behaviour have no N64 comparison yet (RE-437–439, TODO.md).
- Item colour animations, music, arrows, audio and rumble remain.
  Poké Ball rays/open animation, Pokémon materials/translucency, rock
  textures and FlySparks head-0 state remain (RE-435, RE-436, TODO.md).
- The N64 throws the Poké Ball right where the port throws it left
  (RE-434). Item damage/trajectory comparisons need event alignment.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- The 1P Game, bonus stages, pipe traversal/plant notification, selects'
  spotlight and no-save-data unlocks remain in TODO.md. Stage performance
  and VRAM residency belong to `P5`.
