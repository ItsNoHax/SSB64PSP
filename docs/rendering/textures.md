# Textures

Part of [rendering.md](../rendering.md).

## Formats

| N64 | PSP | Notes |
|---|---|---|
| CI4 / CI8 | `PsmT4` / `PsmT8` + CLUT | Dominant format; native CLUT support |
| I4 / I8 | `PsmT4` / `PsmT8` + grey CLUT | Exact; intensity also drives alpha |
| RGBA16 (5551) | `Psm5551` | Channel order differs |
| RGBA32 | `Psm8888` | Direct |
| IA4 / IA8 / IA16 | `Psm8888` (or `PsmT8`) | No PSP IA format |

- CI4 dominates; TLUT loads almost always 16 entries.
- Paletted formats stay paletted to fit VRAM ([D-003](../decisions/D-003.md)).
- `ssb-rom::texture` decodes to RGBA8888.
- `ssb-rom::psp_texture` packs to the chosen PSM, swizzles rows of at least 16 bytes (RE-022).

### Lossless palettization (RE-491)

- Pack writer stores any `Psm8888`/`Psm5551` texture whose level 0 has at most 256 colours as `PsmT4`/`PsmT8` with a CLUT of exactly those colours.
  - Level 0 only.
  - Code: `psp_texture::palettize_lossless`, `TextureDesc::TEXELS_PALETTIZED`.
- Pixels unchanged: GE looks texels up before it filters.
- Most compensated and IA textures qualify; GE reads a quarter or an eighth of the bytes.

### TLUT mode

- RDP's `G_MDSFT_TEXTLUT`, not the texture format, decides palette lookup (RE-313).
- Any 4- or 8-bit texel drawn with TLUT on = index; IA16 TLUT entries act as intensity/alpha.
- CI drawn with TLUT off: raw index read as grey.
- `mesh.rs` tracks RDP/RSP state per task display list; drops the palette of TLUT-off draws.
- Index beyond a short `G_LOADTLUT` reads stale TMEM; measured values: `romtool`'s `STALE_TLUT_ENTRIES`.
- `G_SETTILE.palette` selects a 16-entry bank within a larger TLUT (RE-224).

### Tile loads

- `G_LOADTILE` copies a rectangle of the `G_SETTIMG` image into TMEM.
- Texture drawn from that TMEM address reads from the rectangle's corner at the image's own width.
  - Not from the image's start at the tile's width (RE-423).
- Example: Yoshi's Island's fruit panel = five such strips of one 66 × 64 RGBA32 image.

### Texture references

- Texture named by file + offset.
- Stage display lists often draw texels from another archive file via an extern relocation → address word reads zero in the list.
- Converter keys on the address word's own offset.
- `TextureRef` names texel file and palette file independently (RE-037).
- Not packed (`romtool textures` reports; RE-057–060, RE-067, RE-070):
  - Runtime-framebuffer references (RE-055).
  - One missing palette (`ITCommonObject`).
- Archive-wide texture total exceeds the ~700 KiB VRAM pool → per-scene residency required ([memory.md](../memory.md)).

## Addressing

- Verified by host tests (RE-220–224).
- `crates/ssb-rom/src/n64_addressing.rs`: reference model of RDP tile addressing, transcribed from `angrylion-rdp-plus`.

| Topic | Rule | Evidence |
|---|---|---|
| Mirror | GE has no mirror wrap → mirrored axes pre-baked for every period the drawn rectangle spans; 0 of 810 mirror+clamp axes diverge from reference | RE-067, RE-221 |
| Tile origin | Subtract `uls`/`ult` on every axis before clamp, mask, mirror; repeating axes also use tile-relative phase (omitting it hid Mario/Luigi's overall buttons) | RE-359 |
| Clamp | Native `sceGuTexWrap(Clamp)` after tile-origin rebasing | RE-102, RE-152, RE-359 |
| Power-of-two padding | Repeated edge texels, not zeros → linear filtering at the logical edge matches | RE-222 |
| 512 limit | See below | RE-314, RE-318 |
| 16-byte row minimum | See below | RE-319, RE-320 |
| Unused fields | `mask == 0`, `shift_s`/`shift_t`, `tmem` never occur; `line` unused (texels read straight from ROM) | RE-223 |

### 512 limit

- GE addresses at most 512 texels per axis; rust-psp's `sceGuTexImage` encodes 1024 as log2 15.
- `psp_texture::ge_texture_dims` caps the declared size at 512 (RE-314).
- No packed texture exceeds it.
  - `wide_tile::lower` rewrites the 88 primitives whose clamped, mask-narrowed tile would bake past 512.
  - Replacement: equivalent repeating period plus a clamped far window; splits 34 triangles (RE-318).

### 16-byte row minimum

- GE reads a buffer row under 16 bytes at a 16-byte pitch.
- `psp_texture::ge_buffer_stride` stores every level at least 16 bytes wide (32 texels for T4).
- Declared size stays the power of two of the logical width → UVs and wrapping unchanged.
- PSP-2000 stripe captures confirm the T4 pitch and repair (RE-319, RE-320).

## LOD and mipmaps

- SSB64 never enables RDP LOD: all `TEXTLOD` = `G_TL_TILE`, all `TEXTDETAIL` = `G_TD_CLAMP` (RE-127).
- GE binds level 0 only (`sceGuTexLevelMode(Const, 0.0)`, RE-213).
- Generated lower levels stay in the pack, unused.

## Filtering

- Sampling alignment exact.
- N64 3-point filtering approximated by build-time compensation.

### Sampling alignment

Measured on PPSSPP software and a PSP Slim (RE-304):

- Point: N64 texel coordinate `n` submitted as `n`.
- Linear: submit `n + 0.5` (16 S10.5 units, or `0.5 / uploaded_dim`).
- GE truncates bilinear weights to 4 bits (`31/32 → 15/16`) and truncates the result.
  - Modelled by `n64_filter::sample_bilinear`.

### 3-point compensation

- All `TEXTFILT` = `G_TF_BILERP`: the RDP's 3-point triangle filter (RE-124).
- GE has only 4-tap bilinear.
- `crates/ssb-rom/src/n64_filter.rs`: 3-point reference sampler.
  - Uncompensated: 5.6% of samples differ by ≥ 8/255 (RE-219).
- `filter_compensation` solves at build time for texels that bring GE bilinear output closer to the N64 reference on each primitive's real UV coverage.
- Runtime still uses plain `GU_LINEAR`.

### 2D sprites (RE-517)

- `sprite::psp_image`: alpha-0 texels with a visible 8-neighbour take that neighbour's RGB.
  - One ring; orthogonal, highest alpha first.
  - Baked at pack time → Linear filtering at a sprite edge pulls in no black.
- Filters stay Linear; 5551 unchanged.

### Compensation stages

| Stage | What it does | Evidence |
|---|---|---|
| Coverage | Real S10.5 barycentric UVs, plus critical probes at diagonals, half-texels, GE boundaries, extrema and seams | RE-305, RE-310 |
| Alpha policy | Opaque (alpha forced 255), Cutout (alpha moves only if pass/fail never flips), Translucent (gated on premultiplied visible error) | RE-306 |
| CI4/CI8 | Coordinate descent over palette indices; each sample informs only its dominant bilinear corner | RE-307 |
| Animated CI | One shared index field optimized over all reachable `PaletteID` states | RE-308 |
| Direct colour | GE-exact integer refinement of stored values | RE-309 |
| Texgen | Trains on real vertex normals over 360 poses inside a pose-independent S10.5 bound | RE-311 |
| All formats | RGBA16 stays `Psm5551`, RGBA32/IA stay `Psm8888` | RE-313 |
| UV phase | Per-primitive S10.5 phase shift stored in the pack | RE-312 |
| Exact-site override | Immutable RGBA8888 cross-phase fit at listed use sites only (`residual_fixes::Kind::Direct`) | RE-312 |

### Compensation bookkeeping

- `TextureDesc::TEXELS_COMPENSATED` marks each compensated texture.
- Pack records a digest of its source tile bytes → `romtool matcolors` pairs it with its ROM image without a texel comparison (RE-336).
- Candidate kept only if it does not regress, on an independent holdout:
  - SSE
  - Maximum error
  - ≥ 8 and ≥ 32 counts
- CI textures promoted to RGBA8888 only for ≥ 25% SSE gain within 64 KiB of level-0 cost.

### Residual error

- Most remaining error is a limit of any bilinear texture (`BILINEAR_SURFACE_LIMIT`).
- Per-variant census: `romtool residuals` → `assets/generated/three-point-residuals.md` (gitignored, ROM-derived).
- Known gaps:
  - Metal reflection map (file 302 `0x30`) fails the solver gate.
  - Material-animated texgen uses full-tile coverage.

### Exact-site overrides: why only one ships

- Candidates: `residuals::cross_phase_fit`; review set `residual_fixes::REVIEW_CASES`; packed with `romtool pack --residual-candidates`.
- A/B-captured on PPSSPP against the shipped pack (`tools/residual-ab-capture.sh`; RE-312).
- Shipped: Board the Platforms Mario silhouette fit (all changed pixels inside its own site).
- Other 12: stay `ACCEPT_CURRENT`.
- Why:
  - Texture-space gain does not reach the frame.
    - Textures minified or near one texel per pixel; GE bilinear and RDP 3-point nearly agree.
    - 27–47% cross-phase gain = a few hundred pixels, mostly under 8/255.
  - No N64 reference frame exists for those scenes.
    - No candidate can show a frame-level move toward N64; the deploy bar requires one.
- A/B side effects came from unrelated bugs, not the candidates:
  - `TSIZE` stride overflow on a 576-texel texture (RE-314).
  - Pack size shifting the animation pose at capture (RE-315).

## Texture coordinate generation

- Measured PSP deviation: no pixel-exact comparison with N64 output claimed (RE-214, RE-215, RE-225–239).
- Users: Meta Crystal (file 117); Metal Mario and Polygon models (files 300, 301, 303).
  - Most triangles ordinary generation, a few linear.
  - No metal item; `MMario` is a separate `FTKind` (RE-216).
- Only `G_TEXTURE_GEN` enables generation; `G_TEXTURE_GEN_LINEAR` selects the `acos` curve.
  - Walker keeps the raw geometry-mode word.
- Coordinates generated at `G_VTX` time.
  - No archive triangle loads a vertex under a different texgen mode or scale than it is drawn with → primitive-level state exact ([D-039](../decisions/D-039.md)).
- Look-at basis: camera's world-space right (S) and up (T) (`syMatrixLookAtReflectF`), quantized to signed bytes (RE-227).

Source formula:

```text
dot       = clamp((n · l) / 127, -1, 1)
ordinary: u = (dot + 1) / 4
linear:   u = acos(-dot) / (2π)
S10.5     = trunc(u * gSPTexture_scale)      texels = S10.5 / 32
```

PSP lowering:

- **Ordinary**: GE texture-matrix generator, fed the raw unnormalized normal ([D-038](../decisions/D-038.md), RE-226).
  - GE divides by 128:
    - `a = scale / (128 · dim) · (128/127)`
    - `b = scale / (128 · dim) + origin_shift`
    - Code: `regular_texgen_matrix_coeffs` (RE-228).
  - Per-node transform folded in as `normalize(Mᵀ · lookat)`.
  - `EnvironmentMap` unused: ignores texture scale and offset.
- **Linear**: generated per vertex on CPU (`linear_texgen_uv`) into the authored-UV path ([D-040](../decisions/D-040.md)).
  - Conversion truncates, matching BattleShip and n64psp (RE-229).
- `PrimDesc` carries the `gSPTexture` scale and tile origin.
  - Origin applies on every axis before wrap/mirror/clamp (RE-359).
- Cross-node vertex reuse with differing transforms exists (measured, non-blocking; RE-225).
