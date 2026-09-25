# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed). Order:
  fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** shared grab/capture/throw system, then Donkey Kong's cargo
  carry and throws (`P2`). His attacks and specials are ported; the full
  moveset remains open until cargo is functional.
- **Parallel track:** rendering fidelity (`P5`). Not a gate for gameplay.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Viewer timing and camera head 1 | Shared PSP input peeks the latest sample instead of waiting per tick; the viewer caches its stage animation descriptor. On PSP-2000, stage 40 held 1.026 ms/tick through tick 3,600 after polling had previously climbed past 9 ms/tick. All packed head-1 graphs start from the camera XLU reset; five goldens refreshed | RE-328, RE-329 |
| Material follow-up | Pack v36 records each texture's render tile; texel verification no longer guesses format or mirror layout. `unk10 == 1` U halving follows the decomp, and manager effects resolve material state on their own clock | RE-327 |

## Verification baseline

- `cargo test --workspace`: 888 tests pass (ROM-backed tests also pass).
- `romtool matcolors --pack`: 103 entries, all 15 tracks and four resolvers
  exact over 600 frames; 145/145 textures match through the recorded tile,
  33/33 CLUTs match, and every animated UV sample lands on the RDP texel.
- Both PSP builds pass.
- Pack v36 rebuilt from the ROM with the camera seed: 22,361,792 bytes,
  SHA-256 `a5eacfa256a5ef6a6ca09728f24b61fd93e47c956a3eb7fe741a0367eedd13a0`.
- PPSSPPHeadless: all 68 goldens match twice; five changed for RE-328.
- PSP-2000 Slim, firmware 6.61 ARK, PSPLink v3.2.1: stage 40 held
  1.026 ms/tick in each 600-tick window from tick 601 through 3,600;
  controller peek cost 7 µs/tick, no dropped ticks or exceptions (RE-329).
- PSP-2000 Slim, firmware 6.61: v35 Mushroom Kingdom, Meta Crystal and
  Final Destination agree with the new PPSSPP renders (RE-326). v34
  (`ff5166dd…`) Race to the Finish renders the translucent glows (+105 µs
  render CPU). The rest of the golden matrix and the Donkey gameplay are not
  hardware-validated.

## Blockers

- Cargo requires shared grab ownership, capture states, throw motion data
  and gameplay-facing joint attachment. Existing hitboxes still use root
  offsets instead of joint transforms, so move reach is provisional.
- Fighter map collision is floor-only (no wall/ceiling solver).
