# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed). Order:
  fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Samus's normals, specials and grab/throw motion scripts
  (`P2`), continuing the remaining fighter movesets.
- **Parallel track:** rendering fidelity (`P5`). Not a gate for gameplay.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Gameplay joint attachment | Mario, Fox and Donkey attack/catch boxes use their posed source joints. Held TopN placement subtracts its child translation through the catcher's heavy-item joint and draws at that rotation. Grab golden refreshed | RE-332 |
| Shared grabs and Donkey cargo | Mario, Fox and Donkey Kong catch, hold, throw and escape through linked fighter statuses; Donkey Kong carries and throws. The pack binds grab clips by their source runtime-joint flags, and the Training golden now shows both fighters on the platform | RE-330, RE-331 |

## Verification baseline

- `cargo test --workspace`: 905 tests pass (including ROM-backed tests).
- `romtool anims --verify`: 414 finite lengths agree with the decompilation;
  generated fighter-animation table is reproducible byte for byte.
- `romtool matcolors --pack`: 103 entries, all 15 tracks and four resolvers
  exact over 600 frames; 145/145 textures match through the recorded tile,
  33/33 CLUTs match, and every animated UV sample lands on the RDP texel.
- Both PSP builds pass.
- Pack v36 rebuilt from the ROM: 22,446,048 bytes, SHA-256
  `b5c86c5efe53030626fac4629561140da0d8596f4c39542ec07173c4ae5e6686`.
- PPSSPPHeadless: updated grab golden matches twice; 10,908 pixels changed
  from the previous held pose (RE-332). The previous 69-scene baseline is
  RE-331; this batch ran the focused scene.
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
- Hurtboxes remain one root sphere; some same-valued source attack boxes
  attached to different joints are still condensed. Held non-unit scale is
  deferred (RE-332).
