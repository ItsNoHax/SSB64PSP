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
| Dynamic stage colour/light tracks | Race to the Finish holds the ROM's only 2 `PrimColor` and 2 light tracks. Glows now blend with their live alpha; the lit fixture lights on the GE under the stage light with live `LIGHT_1`/`LIGHT_2`. Exact against a decomp reference over 600 ticks. `MaterialAnimator` now ticks all 103 entries (64+ were frozen). Pack v34, +0 bytes; 8 goldens refreshed | RE-322 |
| RDP two-tile fractional blend | Dream Land's two ponds draw `TEXEL1` in a second GE pass weighted by the live `PRIM_LOD_FRAC`; within one step of the RDP equation. Pack v33 | RE-321 |
| Donkey Kong attacks and specials | 20 normal attacks and three special families; cargo/grabs remain open | decomp: `ftdonkey*` |

## Verification baseline

- `SSB64_ROM=… cargo test --workspace`: 865 tests pass.
- `romtool matcolors --pack`: 3 stage colour entries, 0 mismatching ticks.
- Both PSP builds pass.
- Pack v34 built twice, byte-identical: 22,224,368 bytes, SHA-256
  `ff5166dded1faa79c07f4aca275a87aa6f35cc23c2483bd5cf0cde72e42921b5`.
- PPSSPPHeadless: `tools/golden.sh verify --twice`: all 68 goldens match and
  repeat pixel-identically.
- PSP-2000 Slim, firmware 6.61: v34 Race to the Finish renders the
  translucent glows; dynamic colour costs +105 µs render CPU, +4 state
  writes per frame; no exceptions. New Donkey gameplay is not
  hardware-validated.

## Blockers

- Cargo requires shared grab ownership, capture states, throw motion data
  and gameplay-facing joint attachment. Existing hitboxes still use root
  offsets instead of joint transforms, so move reach is provisional.
- Fighter map collision is floor-only (no wall/ceiling solver).
