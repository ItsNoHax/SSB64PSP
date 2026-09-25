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
| Material animation phase | `MaterialJoint` tick `n` is now the decomp's frame `n` (was `n + 2`): the first parse no longer advances the clock and keys subtract `anim_speed`. Stage and effect material tracks shift two frames; `romtool matcolors` exact at phase 0. A pre-roll control reproduces all 68 old goldens, so the 14 refreshed goldens differ only by phase | RE-324 |
| Task-list-1 XLU reset | Stage list-1 primitives take the whole `G_RM_AA_ZB_XLU_SURF` reset; static `TEXEL0 * PRIM` classifies where the mode blends. 15 primitives now blend (Race to the Finish cones, Sector Z engine glows, Board the Platforms bar glow, Zebes, Saffron, staff roll). N64 references agree. 4 goldens refreshed; pack +1,312 bytes | RE-323 |
| RDP two-tile fractional blend | Dream Land's two ponds draw `TEXEL1` in a second GE pass weighted by the live `PRIM_LOD_FRAC`; within one step of the RDP equation. Pack v33 | RE-321 |

## Verification baseline

- `SSB64_ROM=… cargo test --workspace`: 868 tests pass.
- `romtool matcolors --pack`: 103 `MatAnimDesc`s, all 15 material and colour
  tracks bit-exact against the decomp reference over 600 frames.
- Both PSP builds pass.
- Pack v34 built twice, byte-identical: 22,225,680 bytes, SHA-256
  `a40728812aa9e91c7f437899d1763bd0f97e30ab1f31372df470906112ea1818`.
- PPSSPPHeadless: `tools/golden.sh verify --twice`: all 68 goldens match and
  repeat pixel-identically.
- PSP-2000 Slim, firmware 6.61: v34 (`ff5166dd…`) Race to the Finish
  renders the translucent glows; dynamic colour costs +105 µs render CPU,
  +4 state writes per frame. RE-323's pack, RE-324's phase and the new
  Donkey gameplay are not hardware-validated.

## Blockers

- Cargo requires shared grab ownership, capture states, throw motion data
  and gameplay-facing joint attachment. Existing hitboxes still use root
  offsets instead of joint transforms, so move reach is provisional.
- Fighter map collision is floor-only (no wall/ceiling solver).
