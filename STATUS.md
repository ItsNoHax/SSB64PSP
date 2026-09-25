# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed). Order:
  fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Link's normals, specials, boomerang/bomb weapons and
  grab/throw motion scripts (`P2`), continuing the remaining fighter movesets
  in `FTKind` order.
- **Parallel track:** rendering fidelity (`P5`). Not a gate for gameplay.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Luigi moveset | 20 normals and Jab3 from `220_LuigiMainMotion.c`; Fireball (attribute row 1), Super Jump Punch (launch frame 7) and Cyclone on Mario's shared special statuses; grabs and throws. 21 Luigi attack slots plus his special, grab and thrown clips are packed. Host-only: Luigi is not selectable in `psp-game` | RE-334 |
| Samus moveset | 22 normals, Charge Shot, Screw Attack, Bomb, Grapple Beam and throws. Host-only; weapons not drawn | RE-333 |

## Verification baseline

- `cargo test --workspace`: 925 tests pass (including ROM-backed tests).
- `romtool anims --verify`: 489 finite lengths agree with the decompilation;
  generated fighter-animation table is reproducible byte for byte.
- `romtool matcolors --pack`: 103 entries, all 15 tracks and four resolvers
  exact over 600 frames; 145/145 textures match through the recorded tile,
  33/33 CLUTs match, and every animated UV sample lands on the RDP texel.
- Both PSP builds pass.
- Pack v36 rebuilt from the ROM: 22,631,056 bytes, SHA-256
  `096a03c923ed6523665a6c09bdec538b3c8b7025c61ba69d49c9c5a4f548a82e`.
- PPSSPPHeadless: the three `f1-training*` goldens match unchanged with the
  rebuilt pack. The previous 69-scene baseline is RE-331.
- PSP-2000 Slim, firmware 6.61 ARK, PSPLink v3.2.1: stage 40 held
  1.026 ms/tick in each 600-tick window from tick 601 through 3,600;
  controller peek cost 7 µs/tick, no dropped ticks or exceptions (RE-329).
- PSP-2000 Slim, firmware 6.61: v35 Mushroom Kingdom, Meta Crystal and
  Final Destination agree with the new PPSSPP renders (RE-326). The rest of
  the golden matrix and the Donkey/Samus/Luigi gameplay are not
  hardware-validated.

## Blockers

- Fighter map collision is floor-only (no wall/ceiling solver).
- Escape (roll) and hit-status intangibility are unported; Samus's
  charge-loop roll, Screw Attack and Luigi's Super Jump Punch intangible
  frames wait on them (RE-333, RE-334).
- Hurtboxes remain one root sphere; some same-valued source attack boxes
  attached to different joints are still condensed. Held non-unit scale is
  deferred (RE-332).
