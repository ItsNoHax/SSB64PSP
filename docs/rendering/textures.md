# Textures

Part of [docs/rendering.md](../rendering.md). Describes the current model; see `docs/evidence/re/` for how it was established.

## Texture conversion results

`romtool textures` extracts every texture the display lists actually bind and
packs it for the GE. Current output (`cargo run --release -p romtool --
textures "rom/Super Smash Bros. (USA).z64"`):

```
unique textures bound  665
packed                 638
failed                  27
  decode: MissingPalette          1
  segmented addr (seg 0x01)      26
note: CI texture, no TLUT recorded  1  (packs successfully; informational)
swizzled               432 (68%)

by PSP format:
  Psm8888     67 textures      641.1 KiB
  PsmT4      549 textures      442.5 KiB
  PsmT8       22 textures       87.4 KiB

VRAM budget
  packed (chosen formats)     1170.9 KiB
  naive, all RGBA8888         2432.8 KiB
  saving                        51.9%
  fits in ~700 KiB texture VRAM? no — needs streaming (1.7x over)
```

**51.9% saved** by keeping paletted textures paletted, down from 68.6%
before RE-067's mirror-texture fix (763.2 KiB) and 56.5% before RE-070's
targeted dither-blur (1059.0 KiB) — both spend real bytes converting
specific textures to `Psm8888` for correctness, on top of the format
choice below. `PsmT4` still carries 549 of 638 packed textures; expanding
those to RGBA8888 would cost eight times as much and blow the VRAM budget
far worse.

`unique textures bound` rose from 647 to 665 and `packed` from 617 to 638
this session (R0.7, RE-059/RE-060): resolving two files' `MObj` material
tables didn't just fix palettes on textures that were already bound —
several primitives had no texture binding at all before (their `SetTimg`
was wiped by an unresolved `forget_texture()` call), and now correctly
resolve one.

Two rules drive the packing:

* **Keep CI4/CI8 paletted.** The PSP has native CLUT support, so the dominant
  N64 format converts at 4 bits per texel with a 16-entry CLUT.
* **Keep I4/I8 paletted too**, against a greyscale CLUT. They are intensity
  ramps, so a palette is exact rather than lossy, and avoids an 8x expansion.

IA and RGBA32 have no PSP equivalent and expand to `Psm8888`.


### Swizzling

57% of packed textures are swizzled. The GE reads through a cache organised in
16-byte by 8-row blocks; storing texels linearly makes each cache line span one
row, so vertical locality is lost. Swizzling reorders texels so each block is
contiguous. Textures whose rows are under 16 bytes cannot be swizzled and are
left linear rather than padded, which would waste more than it saves.


### A texture is named by a file *and* an offset

A display list does not always draw from its own file. A stage's geometry is in
one archive file and its texels in another, reached by a pointer the archive
records as an extern relocation rather than applying — so the address word in
the list reads as zero.

For a long time that was indistinguishable from "this primitive has no
texture", and every stage in the game rendered as a white silhouette. It is
resolved by keying on the address word's own offset, which is what the
relocation is filed under (RE-037): `Cmd::SetTimg` carries that offset,
`mesh::Source` carries the relocations, and `TextureRef` names a file for its
texels and its palette independently — a fighter's palette is in its own file
while a stage's texels are not.


### The 763 KiB figure needs streaming

Only ~700 KiB of VRAM remains after the two framebuffers and the depth buffer
(`docs/memory.md`), so the full texture set does **not** fit at once — it is
1.1x over. This is not a problem in practice: a match needs one stage and up to
four fighters, not every texture in the game. But it does mean texture
residency must be **per-scene**, and that is a known requirement to be
addressed before rendering completeness (`PLAN.md` R0.3/R1), not a surprise
discovered late.


### Remaining unconverted (27 of 665, per current `romtool textures`)

| Reason | Count |
|---|---:|
| segmented address (segment 0x01) | 26 |
| `MissingPalette` at decode | 1 |

Separately, 1 more texture packs successfully but is flagged "CI texture, no
TLUT recorded" — informational, not a failure.

The 26 segment-0x01 entries are not missing texture data at all: RE-055
(`docs/reverse-engineering.md`) traces them to `sLBTransitionPhotoHeap`, a
runtime per-frame copy of the framebuffer that the loading-break ("LB")
transition system binds to RSP segment 1 once per frame
(`refs/ssb-decomp-re/src/lb/lbtransition.c`). That data never exists in any
ROM file, so no texture converter can produce it; a real implementation
belongs to framebuffer effects (`PLAN.md` R0.13), not this converter. The
`MissingPalette` case is not a decode bug either: RE-057 originally traced 4
such failures to three files (`MVCommon`, `ITCommonObject`, `LinkSpecial2`)
whose scene graphs get no — or only partial — `MObj` material-table
pairing, which causes a real, present palette load elsewhere in the same
file to get dropped when an unrelated, unresolvable material call
intervenes. `LinkSpecial2` (RE-059, a third record shape, `EFDesc`, living
outside the archive and hand-entered) and `MVCommon` (RE-060, a fourth
mechanism — no struct at all, just a code call sequence, also hand-entered)
are now fully fixed. Only `ITCommonObject`'s one remaining graph (a fifth
mechanism — a byte-offset delta from a runtime pointer, not yet traced) and
`LinkSpecial2`'s already-fixed file's third graph (a `WPAttributes`-named
Spin Attack model, RE-058, untyped in the decompilation) are still open.
All of it belongs to `PLAN.md` R0.7 (missing material tables), not this
converter. Earlier passes over this data also reported null-address and
out-of-file failure classes; neither appears in the current `romtool
textures` output, so treat them as resolved until a re-run shows otherwise.


## Textures

N64 formats and their PSP destinations:

| N64 | Bits | PSP `TexturePixelFormat` | Notes |
|---|---|---|---|
| RGBA16 (5551) | 16 | `Psm5551` | direct, channel order differs |
| RGBA32 | 32 | `Psm8888` | direct |
| IA16 | 16 | `Psm8888` | expand; PSP has no IA format |
| IA8 / IA4 | 8 / 4 | `Psm8888` or `PsmT8` | expand or palettize |
| I8 / I4 | 8 / 4 | `PsmT8` / `PsmT4` + grey CLUT | intensity drives alpha too |
| CI8 / CI4 | 8 / 4 | `PsmT8` / `PsmT4` | **best case** — PSP has native CLUT support |

CI4/CI8 are the important row: Smash uses them heavily (confirmed by the
`gDPLoadTLUTCmd(..., siz == 8b ? 0xFF : 0xF)` path in `objdisplay.c`), and the
PSP supports paletted textures natively. Those convert almost 1:1 and stay
small in VRAM.

`ssb-rom::texture` decodes to RGBA8888 as a neutral intermediate, and
`ssb-rom::psp_texture` packs from there to the PSM chosen above — including
the swizzle. Both are unit-tested and confirmed on device (RE-022).


### Remaining rendering work

* Mipmap chains are generated at build time for 151+ textures
  (`psp_texture::pack_mipped`), but SSB64 never enables traditional RDP LOD /
  mip blending: RE-127 measured `G_TL_TILE` and `G_TD_CLAMP` throughout the
  real commands. RE-201's direct PSP capture closes the Dream Land canopy
  acceptance item; generated lower levels remain an intentional
  anti-aliasing resource and are inert in the N64-equivalent level-zero draw.
* `G_TX_MIRROR` is reproduced exactly rather than approximated (RE-067):
  since the PSP GE has no native mirror wrap mode (`sceGuTexWrap` is
  `Repeat`/`Clamp` only), `romtool`'s texture conversion pre-bakes a
  mirrored copy of the decoded image on each mirrored axis before packing
  — a real fix, not a heuristic, since pack-time conversion has full
  control of the pixel data and `sceGuTexScale` already renormalises UVs
  against whatever dimensions a packed texture reports. Traced to Dream
  Land's canopy specifically (`file 104` offset `0x798` sets `cm_s=3 cm_t=3
  mask_s=6 mask_t=6` — mirror+clamp, 64-texel period) and confirmed via a
  reversible on-device experiment that the un-mirrored repeat boundary was
  visibly wrong. Costs real VRAM: 187 of 638 packed textures (29%) carry
  the flag on at least one axis, raising packed texture VRAM from 763.2 KiB
  to 1059.0 KiB (+39%). `G_TX_CLAMP`, by contrast, is *not* a gap:
  RE-102 corrected the earlier mask-only conclusion: ordinary clamped axes
  now use native `sceGuTexWrap(Clamp, ...)` after per-axis tile-origin
  rebasing. RE-218 reopened those universal claims; R2.0 (RE-219–224) then
  built the full reference model, fixed third-period mirror+clamp, repeated
  logical edge texels into PSP power-of-two padding, and preserved CI4 palette
  banks. The measured addressing gaps are closed.

* Texgen's ordered T1–T10 queue is complete (RE-225–239): raw normal
  semantics, LookAt quantization, shared regular/linear reference math,
  integer conversion, tile/lighting interaction, addressing, original-ROM
  Metal comparison, and physical-PSP captures are all recorded. The current
  path is source-formula/reference-port exact; this is not a claim of
  pixel-exact original-N64 output.

* The renderer corrective gate is closed (R2.2/C1–C7, RE-240–261): primitive
  colour has one owner; load-time lighting provenance is retained; depth
  compare/write state is independent; primitive runs preserve submission
  order; and out-of-band GU draws invalidate the GE state cache. Remaining
  work is the explicitly separate physical-hardware matrix and R3 profiling.


**Texture formats**, by `G_SETTILE` count:

| Format | Count | PSP destination |
|---|---:|---|
| **CI4** | 1192 | `PsmT4` + 16-entry CLUT — the dominant case |
| RGBA16 | 92 | `Psm5551` |
| IA16 | 83 | expand to `Psm8888` |
| CI8 / I4 / I8 / IA8 / RGBA32 | few | `PsmT8` / `Psm8888` |

TLUT loads are overwhelmingly **16 entries**. So the common case is a CI4
texture with a 16-colour palette — 4 bits per texel, natively supported by the
PSP. That matters because only ~700 KiB of VRAM is left after framebuffers
(`docs/memory.md`).

Counts against `Ci 16bpp` and `Rgba 4bpp` are tile *descriptors* used to stage
TLUT loads, not real texture formats — CI is only ever 4- or 8-bit.


* **`G_TEXTURE_GEN`/`G_TEXTURE_GEN_LINEAR`** (RSP-computed
  reflection-mapped UVs, not the display list's own baked UVs) is used by
  file 117 (`StageMetalFile2`, Meta Crystal) and files 300/301/303
  (`MMarioModel`/`NMarioModel`/`NFoxModel`) among 16 files in total — the
  well-known "Metal [Character]" transformation's signature reflective look
  (RE-119). **Not an item effect** — RE-216 found no metal-type item exists
  in the decomp; `MMarioModel`/`NMarioModel`/`NFoxModel` back a separate,
  permanent `FTKind` the original only constructs for the 1P-mode stage-8
  boss fight. Archive-wide there are 3,012 texgen triangles, 2,743 ordinary
  and 269 linear (RE-214's `romtool texgen` census).

  The two bits are **independent state, and only `G_TEXTURE_GEN` enables
  generation**. `G_TEXTURE_GEN_LINEAR` is a *modifier* selecting the `acos`
  curve; it generates nothing on its own. The walker keeps the raw
  geometry-mode word and derives the effective mode from it, so a list that
  clears only `G_TEXTURE_GEN` and later re-sets it correctly resumes in linear
  mode. (An earlier three-state enum could not represent that, and let the
  linear bit enable generation by itself. On this ROM the difference is
  unobservable — the raw pair `(GEN=0, LINEAR=1)` never occurs — but it is
  measured rather than assumed.)

  Source semantics, from `refs/BattleShip`'s F3DEX interpreter:

  ```text
  dot = clamp((n · l) / 127, -1, 1)     l = look-at basis, n = object normal
  ordinary:  u = (dot + 1) / 4
  linear:    u = acos(-dot) / (2*pi)
  S10.5      = u * gSPTexture_scale     texels = S10.5 / 32
  ```

  **Vertex-load semantics.** F3DEX generates these coordinates during `G_VTX`
  processing, not at draw time, so primitive-level state is only equivalent if
  no list loads a vertex under one state and draws it under another. Measured
  archive-wide (RE-214): zero texgen triangles have vertices loaded under a
  different effective mode or a different `G_TEXTURE` scale than the draw.
  Primitive granularity is therefore a proven optimisation, not an assumption.
  203 texgen triangles do reuse a vertex across a node or list boundary, and
  every one of them agrees on both.

  **The look-at basis is the camera's world-space right and up.**
  `syMatrixLookAtReflectF` writes `right` into `l[0]` (drives S) and `up` into
  `l[1]` (T); `gmCameraPrepLookAtFuncMatrix` emits them as
  `gSPLookAtX`/`gSPLookAtY`. World space, not eye space, because SSB64
  concatenates the view matrix into the *projection* matrix and leaves the
  modelview stack model-only.

  **PSP translation.** The GE's `EnvironmentMap` generator computes the right
  dot product but ignores `sceGuTexScale`/`sceGuTexOffset` (measured, RE-214),
  so it can only sweep the whole uploaded texture — wrong for the 32x8 tile
  that sweeps 16 of its 32 texels and for the 48x42 tile padded to 64x64.
  Coordinates are generated through the GE's texture-**matrix** generator
  instead, with the projection source set to the raw, un-normalised vertex
  normal (RE-226: the RSP's own `G_TEXTURE_GEN` never normalises the
  quantised normal either, only scales it, and the previously-shipped
  `NormalizedNormal` mode was measured collapsing every normal to the same
  output regardless of magnitude) and the matrix carrying the affine term
  `u = dot * a + b`, via `regular_texgen_matrix_coeffs`
  (`crates/ssb-rom/src/psp_texture.rs`). `a` and `b` need *different*
  corrections for the GE's own measured internal divisor (`/128`) against
  the original hardware's `/127` (RE-226): `a = gSPTexture_scale / (128 *
  uploaded_dim) * (128/127)` — it multiplies the normal-dependent dot
  product, which *is* read through that divisor — but `b`, the curve's
  zero-crossing constant plus the tile-origin shift, is not, so it must
  **not** carry the same `128/127` factor: `b = gSPTexture_scale / (128 *
  uploaded_dim) + origin_shift`. RE-228 (`PLAN.md` R2.1/T4) found and fixed
  a real, if sub-texel, bug here — an earlier version used the
  `128/127`-compensated `a` for `b` too, overcorrecting by up to
  `scale/127 - scale/128` S10.5 units, caught by a 20,000-case random
  property test against an independent source-formula reference. The
  generator reads the object-space normal, so each node's world transform is
  folded into the matrix rows as `normalize(M^T · lookat)` — the RSP's own
  `CalculateNormalDir`, reproduced by `ssb_engine::math::
  transform_lookat_basis` — while the vertex normal itself is fed in
  unscaled. No GE light is involved, so the fighter's light 0 cannot reach
  the reflection.

  `PrimDesc` carries the `gSPTexture` scale and the render tile's origin for
  exactly this path (pack `VERSION` 27): under `G_TEXTURE_GEN` the RSP never
  reads the authored UVs those values were already baked into. The origin is
  applied on clamped axes only, matching `push_vertex`'s own rule (RE-152);
  57 texgen triangles bind a nonzero origin, all clamped, up to 3 texels.

  `G_TEXTURE_GEN_LINEAR` cannot go through the generator above at all: the
  generated coordinate is affine in the dot product either way and `acos` is
  not. RE-215 generates it exactly instead, per vertex on the CPU
  (`ssb_rom::psp_texture::linear_texgen_uv`, cross-checked against
  `refs/BattleShip` and `refs/n64psp`, neither of which renormalises the
  vertex normal — both divide by the constant `127`), into the pack's
  existing raw S10.5 authored-UV unit, and submits it through the ordinary
  authored-UV draw path rather than a second GE mode. The final float-to-S10.5
  conversion this shares with the reference-only ordinary curve
  (`texgen_s10_5_addressed`) **truncates**, it does not round (RE-229, `PLAN.md`
  R2.1/T5) — both `refs/BattleShip` and `refs/n64psp` cast straight to an
  integer with no `+ 0.5`; an earlier version of this project added `0.5`
  before casting, a dormant rounding bug fixed by T5 and pinned by boundary
  tests at `N+0.49`/`N+0.50`/`N+0.51` for every real ROM scale. A lookup table was
  considered and rejected: the vertex normal is quantised, and the look-at
  basis it is dotted against varies every frame a camera rotates — also
  quantised to a signed byte (RE-227), but still not a fixed input domain the
  way the normal alone is — so a LUT keyed on the normal alone cannot be exact
  either. 257 triangles archive-wide (12 packed primitives) makes the direct
  polynomial cheap enough that this was not measured as a bottleneck.


### Texture decode

Status: 🟢 85%

`PLAN.md` R0.3 `COMPLETE`: RGBA16/32, IA4/8/16, I4/8, CI4/8 decoded and unit-tested; archive census and pack metrics are recorded in the relevant RE entries

**Remaining work:** Unconverted runtime-framebuffer references remain owned by R0.13


### CI4

Status: COMPLETE

`PLAN.md` R0.4: unit-tested decode; dominant format (1192/3483 `G_SETTILE`)


### CI8

Status: COMPLETE

`PLAN.md` R0.4: unit-tested decode


### TLUT

Status: COMPLETE for ROM-backed assets

`PLAN.md` R0.4: loading verified; cross-node palette inheritance pinned by a unit test confirmed capable of failing (RE-064); palette pointers resolved through extern relocations (RE-037); RE-162 resolves the final N-Bumper material-table palette gap

**Remaining work:** Only the 26 runtime-framebuffer references remain; they do not name a ROM TLUT and belong to R0.13


### Texture filtering

Status: COMPENSATED where measured-material, now including material-aware alpha (RE-306, RE-305); exact fixed-function deviation remains (RE-219); sampling alignment verified (RE-304)

RE-304 separates coordinate alignment from the 3-point
reconstruction problem. A deterministic GE rig samples unique-colour 2x2 and
4x4 RGBA textures at centres, halves, odd 1/32 steps, diagonals, and
clamp/repeat/pre-baked-mirror boundaries. Both PPSSPP software and a PSP Slim
produce the same interior probe values. The measured convention is:

* point: N64 texel coordinate `n` is submitted as `n` (no bias);
* linear: submit `n + 0.5`, equivalently add 16 S10.5 units or
  `0.5 / uploaded_dim` to the normalized GE coordinate;
* the GE truncates bilinear fractions to four bits (`31/32 -> 15/16`) and
  truncates the final channel result.

This explains the `filter_bias = 16.0f` used by the SM64-derived PSP renderer
also present in `refs/oot-PSP`, but the value is adopted here from the SSB64
S10.5 derivation and direct measurements, not by copying that renderer.
`sample_bilinear()` now models the measured GE precision. The runtime adds the
linear correction after authored-UV normalization, after live MObj UV affine
state, and in the regular texgen texture matrix; CPU-generated linear texgen
uses the authored path and therefore receives the same correction once.

RE-124 measured all 151/151 real `G_MDSFT_TEXTFILT` commands as `G_TF_BILERP`, matching the PSP path's `Linear` filter *mode* only; RE-219 built a host-side 3-point reference sampler (`crates/ssb-rom/src/n64_filter.rs`, transcribed from `angrylion-rdp-plus`) and measured it against PSP's symmetric four-tap bilinear archive-wide. The post-RE-304 census covers 684 textures and 11,288,880 samples: 5.647% differ by >=8/255 and 1.057% by >=32/255.

RE-305 replaces the four old hand-authored blur exceptions with one measured build-time solve. Authored triangles supply real S10.5 barycentric UV coverage; texgen conservatively uses dense tile coverage. Candidates must reduce exact integer-sampler SSE and threshold counts without increasing max error. CI4/CI8 first retain their existing palette; promotion to RGBA8888 requires >=25% SSE improvement and <=64 KiB level-0 cost. Animated indexed textures are deliberately unchanged so duplicate palette entries cannot silently acquire different meanings in another animation frame. RE-305's own pack selected 406 variants (274 CI4, 132 RGBA8888), reducing their average mean error 1.947 -> 0.876/255 for 494,368 bytes level-zero cost and 783,280 bytes total pack growth -- holding alpha exact throughout, which RE-305 itself named as the source of the remaining error on alpha-bearing textures.

RE-306 makes alpha compensation material-aware instead of always-exact, classifying each texture's real runtime alpha state (`filter_compensation::AlphaPolicy`, derived from `pack::material_alpha_state`/`pack::alpha_gate`, the same logic the GE draw path itself uses) into Opaque (alpha forced to 255, no solver effort spent, and excluded from the acceptance gate since nothing samples it), Cutout (alpha optimized only where the real runtime threshold's pass/fail classification is proven unchanged at every measured sample, re-checked again after any palette/5551 quantization), or Translucent (alpha optimized through the same objective as RGB, gated additionally on a premultiplied "visible" SSE -- `rgb * alpha / 255` -- that cannot regress, so invisible near-zero-alpha texels' raw RGB error can never dominate the decision). The texture-variant cache now keys on this policy, since a shared physical texture used both opaquely and translucently needs two immutable variants, not one shared guess. Rebuilding the same US ROM: 404 variants (267 CI4, 137 RGBA8888; 364 Opaque, 27 Cutout, 13 Translucent), a *smaller* pack (30,158,688 vs 30,172,496 bytes) since excluding alpha from the Opaque gate is net-cheaper than the old always-4-channel gate. All 13 Translucent variants improve, including the Dream Land canopy blossoms RE-305 could only partially fix (`103:0x5F0`'s three variants: 5.154/7.995/4.309 -> 0.974/1.930/1.037 mean, versus RE-305's 2.062-5.195 floor). All 30 baseline-alpha-dominated variants improve with no max regression. See RE-306 for the full per-policy design, test coverage, and a flagged pre-existing PPSSPP-capture-timing drift on stage-animation scenes found (not fixed) while validating this batch.

**Remaining work:** The PSP GE has only `Nearest`/`Linear` and no programmable shader stage, so a texture-only inverse cannot equal the RDP's piecewise triangular surface everywhere. Alpha preservation, animated palette semantics, and conservative texgen coverage intentionally leave residual error. RE-305 has PPSSPP/host validation but no physical-PSP spot check.


### Texture addressing

Status: VERIFIED — four measured gaps fixed (RE-221, RE-222, RE-224); host-test evidence, no PPSSPP visual confirmation (RE-224)

RE-220 built a host-side N64 tile-addressing reference model (`crates/ssb-rom/src/n64_addressing.rs`, transcribed from `angrylion-rdp-plus`'s `tcshift_cycle`/`TRELATIVE`/`tcclamp_cycle`/`tcmask_coupled`) and measured all three questions RE-218 reopened, archive-wide, 2,484 real authored-UV primitives: (1) mirror+clamp beyond the first mirrored period — 810 real axis instances, 176 reaching a third+ mask period, 99 (12.22%) measurably diverging from the then-current PSP lowering; (2) `mask == 0` — zero real occurrences archive-wide, invariant pinned with a test, no fix needed; (3) PSP power-of-two padding vs the N64 logical clamp boundary — 456 real clamped-non-mirrored-non-POT axis instances, 347 (71.4%) with a UV sample reaching the last logical texel where `Linear` blends into zero-filled padding. RE-221 (`PLAN.md` R2.0/P0c) fixed gap (1): `texture::mirror_extend` now bakes every mirrored period a mirror+clamp axis's drawn rect spans (`TextureRef::drawn_width`/`drawn_height`) instead of always exactly two; re-measured archive-wide divergence is **0/810**, now asserted in the census test. RE-222 (`PLAN.md` R2.0/P0d) fixed gap (3): `pad_edge_repeat`/`pad_edge_repeat_nibbles` (`crates/ssb-rom/src/psp_texture.rs`) fill the padding region with the repeated edge row/column instead of zeros, applied at the actual production padding site (`encode_level`, via `pack_mipped`) and, for consistency, `pack_rgba`/`pack_indexed` (the particle-frame path) too — a no-op on an already-POT texture, never touching a mirrored axis by construction (mirror-doubling always lands on a power of two, RE-220). RE-223 (`PLAN.md` R2.0/P1) then censused `G_SETTILE`'s remaining unconsumed fields (`palette`/`line`/`tmem`/`shift_s`/`shift_t`, 2,238 real render-tile-0 instances): `shift_s`/`shift_t`/`tmem` pinned zero-archive-wide invariants, `line` pinned as structurally unconsumed (TMEM staging a converter that reads texels straight from ROM never needs); `palette` was a real, material gap — 7/1,948 CI4 instances (file 86, `ITCommonObject`) requesting bank 1 of a 48-entry loaded TLUT that `mesh.rs` always resolved as bank 0. RE-224 (`PLAN.md` R2.0/P2) fixed gap (4): `SetTile.palette` now threads through `mesh.rs`'s `State`/`TextureRef`, and `tools/romtool`'s `palette_bank_offset` shifts the TLUT read by `palette * 16` entries at pack time — a no-op by construction when `palette == 0`, guarded when the loaded TLUT is smaller than the requested bank. Confirmed by two host tests built from RE-223's own measured real TLUT shape; a PPSSPP TEXVIEW before/after was attempted but not obtained (interactive-only viewer, no reliable unattended automation in this environment — see RE-224), so this row is verified by host-test evidence, not an on-device screenshot

**Remaining work:** R0.5's wrap/clamp/mirror acceptance item and `PLAN.md` R2.0 are both closed; R2.1/T6/T7 consume the reference model rather than duplicating it; a PPSSPP TEXVIEW screenshot of file 86 (RE-224 records the exact texture/object indices) remains a manual follow-up; physical PSP validation remains part of R2


### LOD/mipmaps

Status: COMPLETE (original behavior identified and reproduced)

RE-127 measured 131/131 `TEXTLOD` commands as `G_TL_TILE` and 121/121 `TEXTDETAIL` commands as `G_TD_CLAMP`; SSB64 never enables traditional RDP LOD/mipmap blending. RE-213 stopped exposing levels above zero (`sceGuTexMode` max-mip 0, `sceGuTexLevelMode(Const, 0.0)`, bilinear); RE-214 revalidated that after the texgen refactor — `bind_texture` still binds level zero only, and Dream Land is byte-identical to RE-213's own level-zero capture, SHA-256 `08cc25cc...`

**Remaining work:** Generated lower levels stay in the pack but inert; removing them is a separate pack-format decision


### Texture coordinate generation

Status: COMPLETE with measured PSP deviation

RE-214/215 establish raw-bit handling and captures; R2.1/T1–T10 (RE-225–239) completes load-space provenance, signed normals, quantized LookAt, shared regular/linear math, integer conversion, tile addressing, original-ROM comparison and physical-PSP validation. `romtool texgen --verify` passes in RE-261

**Remaining work:** T1's 164 cross-node differing-transform vertex reuses remain a measured non-blocking follow-up; no pixel-exact original-N64 claim is made
