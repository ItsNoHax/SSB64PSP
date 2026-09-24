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

By `G_SETTILE` count: CI4 1,192, RGBA16 92, IA16 83, others few. TLUT loads
are almost always 16 entries. Paletted formats stay paletted to fit VRAM
([D-003](../decisions/D-003.md)).

`ssb-rom::texture` decodes to RGBA8888; `ssb-rom::psp_texture` packs to the
chosen PSM and swizzles rows of at least 16 bytes (RE-022).

### TLUT mode

The RDP's `G_MDSFT_TEXTLUT`, not the texture format, decides palette lookup
(RE-313):

- Any 4- or 8-bit texel drawn with the TLUT on is an index. IA16 TLUT entries
  act as intensity/alpha.
- CI drawn with the TLUT off reads the raw index as grey.
- `mesh.rs` tracks RDP/RSP state per task display list and drops the palette
  of TLUT-off draws.
- An index beyond a short `G_LOADTLUT` reads stale TMEM; measured values are
  in `romtool`'s `STALE_TLUT_ENTRIES`.
- `G_SETTILE.palette` selects a 16-entry bank within a larger TLUT (RE-224).

### Texture references

A texture is named by a file and an offset. Stage display lists often draw
texels from another archive file through an extern relocation, so the address
word reads zero in the list. The converter keys on the address word's own
offset, and `TextureRef` names the texel file and palette file independently
(RE-037).

## Conversion results

`romtool textures` (RE-057–060, RE-067, RE-070):

| Metric | Value |
|---|---|
| Unique textures bound | 665 |
| Packed | 638 (549 `PsmT4`, 22 `PsmT8`, 67 `Psm8888`) |
| Not packed | 26 runtime-framebuffer references (RE-055), 1 missing palette (`ITCommonObject`) |
| Packed size | 1,170.9 KiB (51.9% below all-RGBA8888) |

These figures predate the per-variant filter compensation below, which adds
texture variants. The archive-wide total exceeds the ~700 KiB VRAM pool; per-scene
residency is required ([memory.md](../memory.md)).

## Addressing

Status: verified by host tests (RE-220–224).

`crates/ssb-rom/src/n64_addressing.rs` is a reference model of RDP tile
addressing, transcribed from `angrylion-rdp-plus`.

- **Mirror**: the GE has no mirror wrap, so mirrored axes are pre-baked for
  every period the drawn rectangle spans (RE-067, RE-221). 0 of 810 mirror+clamp
  axes diverge from the reference.
- **Clamp**: native `sceGuTexWrap(Clamp)` after per-axis tile-origin rebasing
  (RE-102, RE-152).
- **Power-of-two padding**: filled with repeated edge texels, not zeros, so
  linear filtering at the logical edge matches (RE-222).
- `mask == 0`, `shift_s`/`shift_t` and `tmem` never occur in the archive;
  `line` is unused because texels are read straight from ROM (RE-223).

## LOD and mipmaps

Status: complete. SSB64 never enables RDP LOD: all `TEXTLOD` commands are
`G_TL_TILE` and all `TEXTDETAIL` are `G_TD_CLAMP` (RE-127). The GE binds level
0 only (`sceGuTexLevelMode(Const, 0.0)`, RE-213). Generated lower levels stay
in the pack, unused.

## Filtering

Status: sampling alignment exact; N64 3-point filtering approximated by
build-time compensation.

### Sampling alignment

Measured on PPSSPP software and a PSP Slim (RE-304):

- Point: N64 texel coordinate `n` is submitted as `n`.
- Linear: submit `n + 0.5` (16 S10.5 units, or `0.5 / uploaded_dim`).
- The GE truncates bilinear weights to 4 bits (`31/32 → 15/16`) and truncates
  the result. `n64_filter::sample_bilinear` models this.

### 3-point compensation

All `TEXTFILT` commands are `G_TF_BILERP`, the RDP's 3-point triangle filter
(RE-124). The GE only has 4-tap bilinear. `crates/ssb-rom/src/n64_filter.rs`
is a 3-point reference sampler; uncompensated, 5.6% of samples differ by
≥ 8/255 (RE-219).

`filter_compensation` solves, at build time, for texels that make GE bilinear
output closer to the N64 reference on each primitive's real UV coverage.
Runtime still uses plain `GU_LINEAR`.

| Stage | What it does | Evidence |
|---|---|---|
| Coverage | Real S10.5 barycentric UVs, plus critical probes at diagonals, half-texels, GE boundaries, extrema and seams | RE-305, RE-310 |
| Alpha policy | Opaque (alpha forced 255), Cutout (alpha moves only if pass/fail never flips), Translucent (gated on premultiplied visible error) | RE-306 |
| CI4/CI8 | Coordinate descent over palette indices; each sample informs only its dominant bilinear corner | RE-307 |
| Animated CI | One shared index field optimized over all reachable `PaletteID` states | RE-308 |
| Direct colour | GE-exact integer refinement of stored values | RE-309 |
| Texgen | Trains on real vertex normals over 360 poses inside a pose-independent S10.5 bound | RE-311 |
| All formats | RGBA16 stays `Psm5551`, RGBA32/IA stay `Psm8888` | RE-313 |
| UV phase | Per-primitive S10.5 phase shift stored in pack v31 | RE-312 |

A candidate is kept only if it does not regress SSE, maximum error, or the
≥ 8 and ≥ 32 counts on an independent holdout. CI textures are promoted to
RGBA8888 only for ≥ 25% SSE gain within 64 KiB of level-0 cost.

The per-variant residual report is
[three-point-residuals.md](three-point-residuals.md), generated by
`romtool residuals`: 1,607 variants, 0 suspected bugs, 0 level-0 mismatches.
Most remaining error is proven to be a limit of any bilinear texture
(`BILINEAR_SURFACE_LIMIT`).

Remaining: the metal reflection map (file 302 `0x30`) fails the solver gate;
material-animated texgen uses full-tile coverage; no physical-PSP check of
compensated packs.

## Texture coordinate generation

Status: complete, with a measured PSP deviation (RE-214, RE-215,
RE-225–239).

Used by Meta Crystal (file 117) and the Metal Mario and Polygon models (files
300, 301, 303): 3,012 triangles, 2,743 ordinary and 269 linear. No metal item
exists; `MMario` is a separate `FTKind` (RE-216).

- Only `G_TEXTURE_GEN` enables generation; `G_TEXTURE_GEN_LINEAR` selects the
  `acos` curve. The walker keeps the raw geometry-mode word.
- Coordinates are generated at `G_VTX` time. No archive triangle loads a
  vertex under a different texgen mode or scale than it is drawn with, so
  primitive-level state is exact ([D-039](../decisions/D-039.md)).
- The look-at basis is the camera's world-space right (S) and up (T)
  (`syMatrixLookAtReflectF`), quantized to signed bytes (RE-227).

Source formula:

```text
dot       = clamp((n · l) / 127, -1, 1)
ordinary: u = (dot + 1) / 4
linear:   u = acos(-dot) / (2π)
S10.5     = trunc(u * gSPTexture_scale)      texels = S10.5 / 32
```

PSP lowering:

- **Ordinary**: the GE texture-matrix generator, fed the raw unnormalized
  normal ([D-038](../decisions/D-038.md), RE-226). The GE divides by 128, so
  `a = scale / (128 · dim) · (128/127)` and
  `b = scale / (128 · dim) + origin_shift` (`regular_texgen_matrix_coeffs`,
  RE-228). The per-node transform is folded in as `normalize(Mᵀ · lookat)`.
  `EnvironmentMap` is not used: it ignores texture scale and offset.
- **Linear**: generated per vertex on the CPU (`linear_texgen_uv`) into the
  authored-UV path ([D-040](../decisions/D-040.md)). Conversion truncates,
  matching BattleShip and n64psp (RE-229).
- `PrimDesc` carries the `gSPTexture` scale and tile origin; the origin
  applies on clamped axes only.

Remaining: 164 cross-node vertex reuses with differing transforms (measured,
non-blocking). No pixel-exact comparison with N64 output is claimed.
