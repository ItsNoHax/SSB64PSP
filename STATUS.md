# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  Order: fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Kirby's normals, specials, grab and throw motion scripts
  (`P2`), continuing the remaining movesets in `FTKind` order.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Captain Falcon moveset | 20 normals, jab finisher and rapid loop; Falcon Punch, Falcon Kick and Falcon Dive with capture/throw; grabs and throws. 35 attack/special slots plus grab clips packed. Host gameplay only | RE-338 |
| Yoshi moveset | 18 normals, jab chain, 14-hit down air; Egg Lay, Egg Throw and Yoshi Bomb; aerial-jump turn and resistance; swallowed-fighter egg escape; thrown egg and Bomb stars; grab and throws. 29 Yoshi slots packed. Host gameplay only | RE-337 |
| `matcolors` texel check | Filter-compensated textures paired by source digest; pack v37 | RE-336 |

## Verification baseline

- `cargo test --workspace`: 963 tests pass, including ROM-backed tests.
- `romtool anims --verify`: 602 finite lengths agree with the decompilation;
  generated fighter-animation table has 268 slots and reproduces byte for byte.
- The previous `romtool matcolors --pack` check covered all 15 tracks,
  145 textures and 33 CLUTs (RE-336).
- Both PSP builds pass.
- Pack v37 rebuilt from the ROM: 23,037,360 bytes, SHA-256
  `2629e02d8bddedf58def1a8b4159a0993b0bc550aba108074d677743504cfe2e`.
- PPSSPP Falcon-fighter golden smoke: 1 of 1 matches. Prior full matrix:
  69 of 69 with the v37 pre-Yoshi pack (RE-336).
- PSP-2000 Slim, firmware 6.61 ARK, PSPLink v3.2.1: stage 40 held
  1.026 ms/tick in each 600-tick window from tick 601 through 3,600 (RE-329).
  Three v35 stages agree with PPSSPP (RE-326). Current Captain Falcon
  gameplay is not hardware-validated.

## Blockers

- Fighter map collision is floor-only (no wall/ceiling solver).
- Falcon Kick wall rebound and Falcon Dive cliff catch await that solver.
- Escape (roll) and hit-status intangibility are unported (RE-333, RE-334).
- The item system is unported; Link's Bomb creates nothing (RE-335).
- Hurtboxes remain one root sphere, including the Egg Lay victim. Some
  same-valued source attack boxes attached to different joints are still
  condensed. Held non-unit scale is deferred (RE-332, RE-337).
- Captain Falcon, Yoshi and several other completed fighters are not
  selectable in `psp-game`; Yoshi's weapons are simulated but not drawn.
