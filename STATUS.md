# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed). Order:
  fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Yoshi's normals, specials (Egg Lay, Egg Throw, Egg Roll,
  Yoshi Bomb) and grab/throw motion scripts (`P2`), continuing the remaining
  fighter movesets in `FTKind` order.
- **Parallel track:** rendering fidelity (`P5`). Not a gate for gameplay.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Link moveset | 14 normals, jab finisher on mid-animation flag 1, five-edge rapid jab, down-air rehit bounce; Boomerang weapon with owner catch; Spin Attack with its fighter-driven weapon; Bomb pull (item unported); Hookshot and throws. 29 Link slots packed. Per-fighter `Attack11` follow-up windows. Host-only | RE-335 |
| Luigi moveset | Normals, jab finisher, shared Mario specials, Fireball row 1, grabs and throws. Host-only | RE-334 |

## Verification baseline

- `cargo test --workspace`: 943 tests pass (including ROM-backed tests).
- `romtool anims --verify`: 525 finite lengths agree with the decompilation;
  generated fighter-animation table is reproducible byte for byte.
- `romtool matcolors --pack`: all 15 tracks and the four resolvers exact
  over 600 frames and 33/33 CLUTs match, but textures 160 and 161 differ in
  the alpha bit of 307 texels and the command exits with an error. The HEAD
  pack before this batch fails the same way (TODO).
- Both PSP builds pass.
- Pack v36 rebuilt from the ROM: 22,738,640 bytes, SHA-256
  `153b4b1574e86d7e12e923864aa06a0ad95f307743fb7f18be552ed4c3ba4465`.
- PPSSPPHeadless: the three `f1-training*` goldens match unchanged with the
  rebuilt pack. The previous 69-scene baseline is RE-331.
- PSP-2000 Slim, firmware 6.61 ARK, PSPLink v3.2.1: stage 40 held
  1.026 ms/tick in each 600-tick window from tick 601 through 3,600 (RE-329).
- PSP-2000 Slim, firmware 6.61: v35 Mushroom Kingdom, Meta Crystal and
  Final Destination agree with PPSSPP (RE-326). The rest of the golden
  matrix and the Donkey/Samus/Luigi/Link gameplay are not hardware-validated.

## Blockers

- Fighter map collision is floor-only (no wall/ceiling solver).
- Escape (roll) and hit-status intangibility are unported (RE-333, RE-334).
- The item system is unported; Link's Bomb creates nothing (RE-335).
- Hurtboxes remain one root sphere; some same-valued source attack boxes
  attached to different joints are still condensed. Held non-unit scale is
  deferred (RE-332).
