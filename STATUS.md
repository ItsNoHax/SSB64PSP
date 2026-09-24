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
| RE-312 manual fixes and visual review | v766 exact-site RGBA8888 override deployed (U2 −47.4%, flips 8→4, +1,728 B); 12 review candidates A/B-captured, all `ACCEPT_CURRENT`; found a large-RGBA8888 state leak and 48 textures over 512 texels (`TODO.md`) | RE-312, [visual review](docs/rendering/three-point-visual-review.md) |
| RE-312 final census and manual-fix triage | 0 deployable 5a rows left; cross-phase triage of 189 hand-edit candidates | RE-312 |
| RE-312 residual deployment | 7 per-primitive UV phases + 4 dense RGBA8888 variants; pack v31 | RE-312 |
| Texture-LUT semantics | Per-task-list RDP/RSP state, `G_MDSFT_TEXTLUT`, stale short TLUTs, every source format compensated | RE-313 |
| Fox moveset | All normals, rapid jab, Blaster, Fire Fox, Reflector | RE-303 |
| Mario moveset | All normals and specials, Fireball weapon, shadows | RE-299, RE-300, RE-302 |

## Verification baseline

- `SSB64_ROM=… cargo test --workspace`: 811 tests pass.
- Both PSP release builds pass.
- Pack v31: 29,427,040 bytes, SHA-256 `de9f6c64…3f825acc08`; four builds
  identical.
- Residual report: 0 suspected bugs, 0 level-0 or padded-addressing
  mismatches, 0 unattempted formats; section 11: 0 recommended, 12 review,
  177 accept.
- PPSSPPHeadless: 14 fighter goldens and all 38 stage goldens pass after
  refreshing `r2-stage-bonus2-mario` (v766). The eight known-failing
  goldens are unchanged; see
  [docs/visual-regression/README.md](docs/visual-regression/README.md#known-failing-goldens).
- No physical-PSP capture of the current pack.

## Blockers

None for gameplay. Known shared gaps that later fighters will hit:

- Fighter map collision is floor-only (no wall/ceiling solver).
- Grabs/throws need per-character throw motion data and gameplay-facing joint
  attachment.
