# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed). Order:
  fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Luigi's normals, specials and grab/throw motion scripts
  (`P2`), continuing the remaining fighter movesets in `FTKind` order.
- **Parallel track:** rendering fidelity (`P5`). Not a gate for gameplay.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Samus moveset | 22 normals, Charge Shot, Screw Attack, Bomb, Grapple Beam catch and both throws from the decomp. Charge Shot and Bomb run in the weapon pool. 31 Samus animation slots and her grab clips are packed. Host-only: Samus is not selectable in `psp-game`, and her weapons are not drawn | RE-333 |
| Gameplay joint attachment | Attack/catch boxes use posed source joints; held TopN follows the catcher's heavy-item joint | RE-332 |

## Verification baseline

- `cargo test --workspace`: 917 tests pass (including ROM-backed tests).
- `romtool anims --verify`: 454 finite lengths agree with the decompilation;
  generated fighter-animation table is reproducible byte for byte.
- `romtool matcolors --pack`: 103 entries, all 15 tracks and four resolvers
  exact over 600 frames; 145/145 textures match through the recorded tile,
  33/33 CLUTs match, and every animated UV sample lands on the RDP texel.
- Both PSP builds pass.
- Pack v36 rebuilt from the ROM: 22,557,024 bytes, SHA-256
  `deda8cfebcc7d876e04a449358a5fa252e1d464d2b08d1b1c71befecb2126dc2`.
- PPSSPPHeadless: the three `f1-training*` goldens match unchanged with the
  rebuilt pack. The previous 69-scene baseline is RE-331.
- PSP-2000 Slim, firmware 6.61 ARK, PSPLink v3.2.1: stage 40 held
  1.026 ms/tick in each 600-tick window from tick 601 through 3,600;
  controller peek cost 7 µs/tick, no dropped ticks or exceptions (RE-329).
- PSP-2000 Slim, firmware 6.61: v35 Mushroom Kingdom, Meta Crystal and
  Final Destination agree with the new PPSSPP renders (RE-326). v34
  (`ff5166dd…`) Race to the Finish renders the translucent glows (+105 µs
  render CPU). The rest of the golden matrix and the Donkey gameplay are not
  hardware-validated.

## Blockers

- Fighter map collision is floor-only (no wall/ceiling solver).
- Escape (roll) and hit-status intangibility are unported; Samus's
  charge-loop roll and Screw Attack start frames wait on them (RE-333).
- Hurtboxes remain one root sphere; some same-valued source attack boxes
  attached to different joints are still condensed. Held non-unit scale is
  deferred (RE-332).
