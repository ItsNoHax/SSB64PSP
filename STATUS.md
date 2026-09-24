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
| Donkey Kong attacks and specials | 20 normal attacks and three special families translated from the motion/status modules; 31 ROM animation slots packed. Giant Punch charge, Spinning Kong hit loops and Hand Slap pulses run in portable gameplay. Cargo/grabs remain open. | decomp: `ftdonkey*`, `212_DonkeyMainMotion.c` |
| PSP-2000 texture-row validation | A/B stripes on a 16×400 T4 pillar: old 8-byte rows gave 191 pure-red pixels; v32's 16-byte rows mixed green into 185. Stock v32 pack loaded and rendered, with no PSPLink exceptions | RE-320 |
| Texture rows under 16 bytes | Measured: the GE read 304 T4 textures with 4- or 8-byte rows at a 16-byte pitch, showing only even rows and sampling past their data. Every row is now stored at least 16 bytes wide; the declared size is unchanged. Pack v32; 44 goldens refreshed | RE-319 |

## Verification baseline

- `SSB64_ROM=… cargo test --workspace`: 832 tests pass.
- `romtool anims --verify`: 27 fighters decoded, 189 decomp lengths agree.
- Both PSP builds pass.
- Pack v32 rebuilt with Donkey animations: 22,200,288 bytes, SHA-256
  `08734c021218bdf4f395bb6a3c5257a20ffde08dd73b32ff0861d63f31ea8552`.
- PPSSPPHeadless: `tools/golden.sh verify --twice` with the rebuilt pack:
  all 67 goldens match and repeat pixel-identically.
- PSP-2000 Slim, firmware 6.61: RE-320 stripe A/B and stock v32 native
  captures; no exceptions. New Donkey gameplay is not hardware-validated.

## Blockers

- Cargo requires shared grab ownership, capture states, throw motion data
  and gameplay-facing joint attachment. Existing hitboxes still use root
  offsets instead of joint transforms, so move reach is provisional.
- Fighter map collision is floor-only (no wall/ceiling solver).
