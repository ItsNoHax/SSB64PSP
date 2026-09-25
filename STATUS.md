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
| Material sampling | Texture, palette and window tracks apply only where the `MObj` still owns that state (33 Mushroom Kingdom primitives drew its sprite); palettes resolve per primitive; UV affine decomp-exact. All bound textures and CLUTs match the ROM. Pack v35; 13 goldens rebaselined; hardware agrees on 3 stages | RE-326 |
| Material resolvers | `romtool matcolors` checks the texture, palette, tile-0 and two-tile-blend resolvers against `gcDrawMObjForDObj`. `PaletteID` resolves for every kind, truncated. Stale pack rebuilt. `MaterialJoint` follows the decomp for `SetTargetRate` and step rates | RE-325 |
| Material animation phase | `MaterialJoint` tick `n` is now the decomp's frame `n` (was `n + 2`): the first parse no longer advances the clock and keys subtract `anim_speed`. Stage and effect material tracks shift two frames; `romtool matcolors` exact at phase 0. A pre-roll control reproduces all 68 old goldens, so the 14 refreshed goldens differ only by phase | RE-324 |

## Verification baseline

- `SSB64_ROM=… cargo test --workspace`: 882 tests pass.
- `romtool matcolors --pack`: 103 `MatAnimDesc`s, all 15 tracks bit-exact
  and all four resolvers exact against the decomp over 600 frames; every
  bound texture and owned CLUT matches the ROM, and every material-animated
  vertex lands on the RDP's texel (RE-326).
- Both PSP builds pass.
- Pack v35 built twice, byte-identical: 22,231,104 bytes, SHA-256
  `5e529bbf1e9ccfeeb19ab4a688fde566e3389fb23957343bb3cb7923f7434cac`.
- PPSSPPHeadless: `tools/golden.sh verify --twice`: all 68 goldens match and
  repeat pixel-identically.
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
