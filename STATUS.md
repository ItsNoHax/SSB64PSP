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
| Fast golden captures | One `golden_capture` build per crate; scene read from `capture_scene.txt`; exit after screenshot; parallel `tools/golden.sh verify` in 17 s vs 14 min 51 s. All 67 captures byte-identical to the old pipeline; manifest `tests/golden/scenes.tsv` | RE-316 |
| Viewer animation per sim tick | `psp-asset-viewer` stage, material and effect-spawn animation now tick per 60 Hz simulation tick, not per render frame. Zero-padded packs no longer change any of the 67 golden captures; 27 animated goldens refreshed | RE-315 |
| GE texture size cap | RE-312's "large RGBA8888 state leak" was no leak: 1024-padded textures overflowed rust-psp's `TSIZE` encoding and sampled neighbouring pack bytes. Declared size now capped at 512; 11 stage goldens refreshed. Stage-35 pixels traced to per-render-frame animation ticks (fixed in RE-315) | RE-314 |
| RE-312 manual fixes and visual review | v766 exact-site RGBA8888 override deployed (U2 −47.4%, flips 8→4, +1,728 B); 12 review candidates A/B-captured, all `ACCEPT_CURRENT`; found a large-RGBA8888 state leak and 48 textures over 512 texels (`TODO.md`) | RE-312, [visual review](docs/rendering/three-point-visual-review.md) |
| RE-312 final census and manual-fix triage | 0 deployable 5a rows left; cross-phase triage of 189 hand-edit candidates | RE-312 |
| RE-312 residual deployment | 7 per-primitive UV phases + 4 dense RGBA8888 variants; pack v31 | RE-312 |
| Texture-LUT semantics | Per-task-list RDP/RSP state, `G_MDSFT_TEXTLUT`, stale short TLUTs, every source format compensated | RE-313 |
| Fox moveset | All normals, rapid jab, Blaster, Fire Fox, Reflector | RE-303 |
| Mario moveset | All normals and specials, Fireball weapon, shadows | RE-299, RE-300, RE-302 |

## Verification baseline

- `SSB64_ROM=… cargo test --workspace`: 820 tests pass.
- Both PSP release builds pass.
- Pack v31: 29,427,040 bytes, SHA-256 `de9f6c64…3f825acc08`; four builds
  identical.
- Residual report: 0 suspected bugs, 0 level-0 or padded-addressing
  mismatches, 0 unattempted formats; section 11: 0 recommended, 12 review,
  177 accept.
- PPSSPPHeadless: `tools/golden.sh verify --twice` passes: 59 goldens match,
  the eight `known-failing` rows of
  [tests/golden/scenes.tsv](tests/golden/scenes.tsv) differ as before, and
  every capture repeats exactly (RE-316).
- No physical-PSP capture of the current pack.

## Blockers

None for gameplay. Known shared gaps that later fighters will hit:

- Fighter map collision is floor-only (no wall/ceiling solver).
- Grabs/throws need per-character throw motion data and gameplay-facing joint
  attachment.
