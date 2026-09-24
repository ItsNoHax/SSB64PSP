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
| RDP two-tile fractional blend | Dream Land's two ponds (`SetLFrac`/`TextureIDNext`, the only ROM users) now draw `TEXEL1` in a second GE pass weighted by the live `PRIM_LOD_FRAC`; both proven exact, every pond pixel within one step of the RDP equation. Ponds were invisible before. Pack v33; 3 goldens refreshed, 1 added | RE-321 |
| Donkey Kong attacks and specials | 20 normal attacks and three special families translated from the motion/status modules; 31 ROM animation slots packed. Giant Punch charge, Spinning Kong hit loops and Hand Slap pulses run in portable gameplay. Cargo/grabs remain open. | decomp: `ftdonkey*`, `212_DonkeyMainMotion.c` |
| PSP-2000 texture-row validation | A/B stripes on a 16×400 T4 pillar: old 8-byte rows gave 191 pure-red pixels; v32's 16-byte rows mixed green into 185. Stock v32 pack loaded and rendered, with no PSPLink exceptions | RE-320 |

## Verification baseline

- `SSB64_ROM=… cargo test --workspace`: 848 tests pass.
- `romtool anims --verify`: 27 fighters decoded, 189 decomp lengths agree.
- Both PSP builds pass.
- Pack v33 built twice, byte-identical: 22,224,368 bytes, SHA-256
  `4c7dfc7136d9540293ae0b80bf241414c8298a3698f65d3ac087d4e4440c38f6`.
  `romtool residuals`: 1,624 variants, 0 suspected bugs.
- PPSSPPHeadless: `tools/golden.sh verify --twice` with the rebuilt pack:
  all 68 goldens match and repeat pixel-identically.
- PSP-2000 Slim, firmware 6.61: v33 pack with the two-tile blend (RE-321)
  and RE-320's v32 captures; no exceptions. New Donkey gameplay is not
  hardware-validated.

## Blockers

- Cargo requires shared grab ownership, capture states, throw motion data
  and gameplay-facing joint attachment. Existing hitboxes still use root
  offsets instead of joint transforms, so move reach is provisional.
- Fighter map collision is floor-only (no wall/ceiling solver).
