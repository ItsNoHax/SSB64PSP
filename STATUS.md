# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed). Order:
  fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Donkey Kong full moveset (`P2`).
- **Parallel track:** rendering fidelity (`P5`). Not a gate for gameplay.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| PSP-2000 texture-row validation | A/B stripes on a 16×400 T4 pillar: old 8-byte rows gave 191 pure-red pixels; v32's 16-byte rows mixed green into 185. Stock v32 pack loaded and rendered, with no PSPLink exceptions | RE-320 |
| Texture rows under 16 bytes | Measured: the GE read 304 T4 textures with 4- or 8-byte rows at a 16-byte pitch, showing only even rows and sampling past their data. Every row is now stored at least 16 bytes wide; the declared size is unchanged. Pack v32; 44 goldens refreshed | RE-319 |
| Wide-tile lowering | 48 textures over the GE 512 limit came from clamped, mask-narrowed tiles baked to their 576–960-texel drawn rect. 88 primitives now draw an equivalent repeating period plus a clamped far window (34 triangles split); 0 textures over 512, pack 29.4 → 22.0 MB, 10 stage goldens rebaselined | RE-318 |
| Known-failing goldens | All eight attributed commit by commit (RE-304–RE-310 sampling and compensation, pillarbox clear) and rebaselined; all 67 goldens pass | RE-317 |
| Fast golden captures | One `golden_capture` build per crate; scene read from `capture_scene.txt`; exit after screenshot; parallel `tools/golden.sh verify` in 17 s vs 14 min 51 s. All 67 captures byte-identical to the old pipeline; manifest `tests/golden/scenes.tsv` | RE-316 |

## Verification baseline

- `SSB64_ROM=… cargo test --workspace`: 828 tests pass.
- Both PSP builds pass.
- Pack v32: 22,109,232 bytes, SHA-256
  `4a972a600fe5bec0edc3fcea14d1ce580f72426cce1af6df2f9692055098763e`
  (RE-319). Compared with RE-318, only 304 short-row textures change; all
  level-0 texels and palettes match.
- Residual report (v32): 1,620 variants, 0 suspected bugs, 0 level-0 or
  padded-addressing mismatches, 0 unattempted formats.
- PPSSPPHeadless: `tools/golden.sh verify --twice` passes: all 67 goldens
  match and repeat pixel-identically; no `known-failing` rows
  (RE-316–RE-319).
- PSP-2000 Slim, firmware 6.61: RE-320 stripe A/B and stock v32 native
  captures; no exceptions. Full hardware golden matrix remains open.

## Blockers

None for gameplay. Known shared gaps that later fighters will hit:

- Fighter map collision is floor-only (no wall/ceiling solver).
- Grabs/throws need per-character throw motion data and gameplay-facing joint
  attachment.
