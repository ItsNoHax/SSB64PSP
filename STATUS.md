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
| RE-312 final census and manual-fix triage | 3 remaining dense rows fail packer gates (0 deployable left); out-of-sample cross-phase triage of 189 hand-edit candidates: 1 recommended, 12 visual review, 176 accept; pack unchanged | RE-312 |
| RE-312 residual deployment | 7 per-primitive UV phases + 4 dense RGBA8888 variants; pack v31; 0 suspected bugs, 0 level-0 mismatches | RE-312 |
| Texture-LUT semantics | Per-task-list RDP/RSP state, `G_MDSFT_TEXTLUT`, stale short TLUTs, every source format compensated | RE-313 |
| Build-time 3-point filter compensation | Alpha-aware, palette-index, animated-CI, GE-exact refinement, critical-UV and texgen coverage | RE-305–311 |
| Fox moveset | All normals, rapid jab, Blaster, Fire Fox, Reflector | RE-303 |
| Mario moveset | All normals and specials, Fireball weapon, shadows | RE-299, RE-300, RE-302 |

## Verification baseline

- `SSB64_ROM=… cargo test --workspace`: 809 tests pass.
- Both PSP release builds pass.
- Pack v31: 29,425,312 bytes, SHA-256 `97b3f4d8…5707f216`.
- PPSSPPHeadless: `tools/verify-fighter-goldens.sh` passed at RE-313; RE-312
  refreshed or added the six goldens it changed. Eight non-fighter goldens
  are known to differ and predate RE-313; see
  [docs/visual-regression/README.md](docs/visual-regression/README.md#known-failing-goldens).
- No physical-PSP capture of pack v31.

## Blockers

None for gameplay. Known shared gaps that later fighters will hit:

- Fighter map collision is floor-only (no wall/ceiling solver).
- Grabs/throws need per-character throw motion data and gameplay-facing joint
  attachment.
