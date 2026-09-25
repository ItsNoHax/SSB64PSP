# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed). Order:
  fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** gameplay joint attachment for hit/catch boxes and held
  fighters (`P2`), then continue the remaining fighter movesets.
- **Parallel track:** rendering fidelity (`P5`). Not a gate for gameplay.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Shared grabs and Donkey cargo | Mario, Fox and Donkey Kong catch, hold, throw and escape through linked fighter statuses; Donkey Kong carries and throws. The pack binds grab clips by their source runtime-joint flags, and the Training golden now shows both fighters on the platform | RE-330, RE-331 |
| Viewer timing and camera head 1 | Shared PSP input peeks the latest sample instead of waiting per tick; the viewer caches its stage animation descriptor. On PSP-2000, stage 40 held 1.026 ms/tick through tick 3,600 after polling had previously climbed past 9 ms/tick. All packed head-1 graphs start from the camera XLU reset; five goldens refreshed | RE-328, RE-329 |
| Material follow-up | Pack v36 records each texture's render tile; texel verification no longer guesses format or mirror layout. `unk10 == 1` U halving follows the decomp, and manager effects resolve material state on their own clock | RE-327 |

## Verification baseline

- `cargo test --workspace`: 901 tests pass (ROM-backed tests also pass).
- `romtool anims --verify`: 414 finite lengths agree with the decompilation;
  generated fighter-animation table is reproducible byte for byte.
- `romtool matcolors --pack`: 103 entries, all 15 tracks and four resolvers
  exact over 600 frames; 145/145 textures match through the recorded tile,
  33/33 CLUTs match, and every animated UV sample lands on the RDP texel.
- Both PSP builds pass.
- Pack v36 rebuilt from the ROM: 22,446,048 bytes, SHA-256
  `b5c86c5efe53030626fac4629561140da0d8596f4c39542ec07173c4ae5e6686`.
- PPSSPPHeadless: corrected grab golden and all 69 scenes match twice
  (RE-331).
- PSP-2000 Slim, firmware 6.61 ARK, PSPLink v3.2.1: stage 40 held
  1.026 ms/tick in each 600-tick window from tick 601 through 3,600;
  controller peek cost 7 µs/tick, no dropped ticks or exceptions (RE-329).
- PSP-2000 Slim, firmware 6.61: v35 Mushroom Kingdom, Meta Crystal and
  Final Destination agree with the new PPSSPP renders (RE-326). v34
  (`ff5166dd…`) Race to the Finish renders the translucent glows (+105 µs
  render CPU). The rest of the golden matrix and the Donkey gameplay are not
  hardware-validated.

## Blockers

- Gameplay-facing hit/catch boxes still use root offsets for hand joints;
  held fighters omit their own TopN offset and attachment rotation. Reach and
  held pose remain provisional.
- Fighter map collision is floor-only (no wall/ceiling solver).
