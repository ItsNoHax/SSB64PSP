# Rendering: N64 → PSP

## The central decision

Smash 64 stores its geometry as **ready-made F3DEX2 display lists in the ROM**.
`objdisplay.c` never builds a mesh; it walks the DObj hierarchy, pushes
matrices, sets material state, then calls `gSPDisplayList(head++, dobj->dl)`
into ROM data.

That single fact determines the whole strategy:

* Star Fox 64 generates display lists dynamically, so `sf64-psp` has to
  translate them **at runtime**.
* Smash's are **static data**. We can translate them **at build time**, once,
  and ship PSP-native vertex buffers.

On a 333 MHz MIPS CPU that difference is large. Preconversion is the plan's
§9 instruction ("prefer preconversion over runtime conversion") and here it is
not merely preferable, it is nearly free.

We do **not** emulate the RDP.

## Rendering status

Per-area status, evidence and remaining work. This table is the rendering
counterpart to `docs/porting-status.md`'s per-crate subsystem table — this one
is scoped to exactly the rendering-correctness categories `PLAN.md` R0
tracks (§6.0's hierarchy cross-reference), not the whole engine. Update it
in the same work cycle as any change to the areas below (`AGENTS.md` §11).

| Area | Status | Evidence/Test | Remaining work |
| --- | --- | --- | --- |
| Geometry | COMPLETE | `PLAN.md` R0.8 and R0.12 `COMPLETE` (transform kinds 44/46/48/50, RE-062/RE-063); RE-143 reproduces signed animated billboard scale; RE-144 corrects billboard Z to the original X/Y/X scale rule; RE-145 individually reviews all 109 billboard ordinals; `romtool mesh` converts every root display list, 0 failures archive-wide | Physical PSP validation remains part of the later rendering gate |
| Projection | COMPLETE | `PLAN.md` R0.14: FOV, viewport/aspect and depth sourced and checked (RE-034/082/084/085); RE-131/150 port and correct the default battle camera; RE-151 traces the original ROM's live two-fighter camera state and pins distance/look-at/eye/FOV numerically, with byte-stable PPSSPP captures | Special entry/dead/pause modes remain future gameplay-system work; physical PSP validation remains part of R2 |
| Texture decode | 🟢 85% | `PLAN.md` R0.3 `COMPLETE`: RGBA16/32, IA4/8/16, I4/8, CI4/8 decoded and unit-tested; archive census and pack metrics are recorded in the relevant RE entries | Unconverted runtime-framebuffer references remain owned by R0.13 |
| CI4 | COMPLETE | `PLAN.md` R0.4: unit-tested decode; dominant format (1192/3483 `G_SETTILE`) | None |
| CI8 | COMPLETE | `PLAN.md` R0.4: unit-tested decode | None |
| TLUT | COMPLETE for ROM-backed assets | `PLAN.md` R0.4: loading verified; cross-node palette inheritance pinned by a unit test confirmed capable of failing (RE-064); palette pointers resolved through extern relocations (RE-037); RE-162 resolves the final N-Bumper material-table palette gap | Only the 26 runtime-framebuffer references remain; they do not name a ROM TLUT and belong to R0.13 |
| Texture filtering | `ACCEPTED_DEVIATION` (RE-219) | RE-124 measured all 151/151 real `G_MDSFT_TEXTFILT` commands as `G_TF_BILERP`, matching the PSP path's `Linear` filter *mode* only; RE-219 built a host-side 3-point reference sampler (`crates/ssb-rom/src/n64_filter.rs`, transcribed from `angrylion-rdp-plus`) and measured it against PSP's symmetric four-tap bilinear archive-wide: 684 real textures, 5.73% of interior sample points differ by ≥8/255, 1.19% by ≥32/255; the already-flagged Dream Land canopy highlight texture (file 103 offset `0x5F0`) reaches the maximum 128/255 diff with a 6.4/255 mean across its own interior — a real, material difference, not a theoretical one | The PSP GE's fixed-function texture unit has only `Nearest`/`Linear`, no third mode and no programmable shader stage to implement a custom 3-point blend; exact reproduction would require abandoning hardware texturing entirely. `Linear` remains the closest available approximation; no fix is implemented or planned |
| Texture addressing | VERIFIED — four measured gaps fixed (RE-221, RE-222, RE-224); host-test evidence, no PPSSPP visual confirmation (RE-224) | RE-220 built a host-side N64 tile-addressing reference model (`crates/ssb-rom/src/n64_addressing.rs`, transcribed from `angrylion-rdp-plus`'s `tcshift_cycle`/`TRELATIVE`/`tcclamp_cycle`/`tcmask_coupled`) and measured all three questions RE-218 reopened, archive-wide, 2,484 real authored-UV primitives: (1) mirror+clamp beyond the first mirrored period — 810 real axis instances, 176 reaching a third+ mask period, 99 (12.22%) measurably diverging from the then-current PSP lowering; (2) `mask == 0` — zero real occurrences archive-wide, invariant pinned with a test, no fix needed; (3) PSP power-of-two padding vs the N64 logical clamp boundary — 456 real clamped-non-mirrored-non-POT axis instances, 347 (71.4%) with a UV sample reaching the last logical texel where `Linear` blends into zero-filled padding. RE-221 (`PLAN.md` R2.0/P0c) fixed gap (1): `texture::mirror_extend` now bakes every mirrored period a mirror+clamp axis's drawn rect spans (`TextureRef::drawn_width`/`drawn_height`) instead of always exactly two; re-measured archive-wide divergence is **0/810**, now asserted in the census test. RE-222 (`PLAN.md` R2.0/P0d) fixed gap (3): `pad_edge_repeat`/`pad_edge_repeat_nibbles` (`crates/ssb-rom/src/psp_texture.rs`) fill the padding region with the repeated edge row/column instead of zeros, applied at the actual production padding site (`encode_level`, via `pack_mipped`) and, for consistency, `pack_rgba`/`pack_indexed` (the particle-frame path) too — a no-op on an already-POT texture, never touching a mirrored axis by construction (mirror-doubling always lands on a power of two, RE-220). RE-223 (`PLAN.md` R2.0/P1) then censused `G_SETTILE`'s remaining unconsumed fields (`palette`/`line`/`tmem`/`shift_s`/`shift_t`, 2,238 real render-tile-0 instances): `shift_s`/`shift_t`/`tmem` pinned zero-archive-wide invariants, `line` pinned as structurally unconsumed (TMEM staging a converter that reads texels straight from ROM never needs); `palette` was a real, material gap — 7/1,948 CI4 instances (file 86, `ITCommonObject`) requesting bank 1 of a 48-entry loaded TLUT that `mesh.rs` always resolved as bank 0. RE-224 (`PLAN.md` R2.0/P2) fixed gap (4): `SetTile.palette` now threads through `mesh.rs`'s `State`/`TextureRef`, and `tools/romtool`'s `palette_bank_offset` shifts the TLUT read by `palette * 16` entries at pack time — a no-op by construction when `palette == 0`, guarded when the loaded TLUT is smaller than the requested bank. Confirmed by two host tests built from RE-223's own measured real TLUT shape; a PPSSPP TEXVIEW before/after was attempted but not obtained (interactive-only viewer, no reliable unattended automation in this environment — see RE-224), so this row is verified by host-test evidence, not an on-device screenshot | R0.5's wrap/clamp/mirror acceptance item and `PLAN.md` R2.0 are both closed; R2.1/T6/T7 consume the reference model rather than duplicating it; a PPSSPP TEXVIEW screenshot of file 86 (RE-224 records the exact texture/object indices) remains a manual follow-up; physical PSP validation remains part of R2 |
| LOD/mipmaps | COMPLETE (original behavior identified and reproduced) | RE-127 measured 131/131 `TEXTLOD` commands as `G_TL_TILE` and 121/121 `TEXTDETAIL` commands as `G_TD_CLAMP`; SSB64 never enables traditional RDP LOD/mipmap blending. RE-213 stopped exposing levels above zero (`sceGuTexMode` max-mip 0, `sceGuTexLevelMode(Const, 0.0)`, bilinear); RE-214 revalidated that after the texgen refactor — `bind_texture` still binds level zero only, and Dream Land is byte-identical to RE-213's own level-zero capture, SHA-256 `08cc25cc...` | Generated lower levels stay in the pack but inert; removing them is a separate pack-format decision |
| Texture coordinate generation | VERIFYING (ordinary and linear) | RE-214/215 establish raw-bit handling, the ROM census and PPSSPP/physical-PSP captures. The final T1–T10 queue in `PLAN.md` still requires load-space/normal-transform proof, raw signed-byte normal semantics, LookAt quantization, linear integer conversion, tile shifts/addressing and an original-N64 Metal comparison. Linear is source-formula exact, not yet proven bit-exact to original hardware | RE-216 corrected the nonexistent VS-Mode “Metal Box item” route; the real stage-8 route or a faithful RAM-level warp remains open |
| Combiner | COMPLETE for classified static paths | `PLAN.md` R0.6: general `(A-B)*C+D` evaluator (RE-039/043), texture blend (RE-073/074), flat colour (RE-080), and shade-scale consumption (RE-106). RE-168's post-RE-163 census accepts 65,000/65,199 source-attributed emitted-triangle visits (99.695%) and source-identifies every missing-constant case | The 186 unsupported-equation visits are catalogued; runtime shield colours belong to future effect/gameplay integration, not static material conversion |
| Lighting | VERIFYING | RE-103/105 and RE-164–167 establish the current path and Dream Land comparison, but R2.2/C1–C2 must prove single-source PRIM ownership and `G_VTX` load-time normal/colour provenance; `looks_like_unit_normal` remains a fallback to quantify |
| Alpha | COMPLETE for both classified gates, including their overlap | `PLAN.md` R0.6: `CVG_X_ALPHA \| ALPHA_CVG_SEL` decoded and wired to `sceGuAlphaFunc` (RE-069), matching `sf64-psp`'s own validated real-hardware approximation. RE-195 additionally decodes `G_MDSFT_ALPHACOMPARE` (a second, independent real discard gate, 29.8% `G_AC_THRESHOLD` archive-wide). RE-214 resolves both onto the GE's one alpha-test unit in `pack::alpha_gate`, with host regressions for every combination | The cutout gate remains an approximation of multisampled coverage (`alpha > 0`), as it always has been; the overlap itself is no longer a gap |
| Blending | COMPLETE for classified single-cycle formulas | RE-129/130 decoded alpha combiners, classified nine archive-wide shapes, and enable real blending for `TEXEL0_ALPHA` and `TEXEL0_ALPHA * SHADE_ALPHA`; PPSSPP-verified on Dream Land | Rare `PRIM_ALPHA` multiply (~43) and two-cycle (~93) primitives are measured and deliberately declined under R0.6 |
| Depth | VERIFYING | RE-068/085 establish the RDP default, depth-test mapping and inverted PSP range, but R2.2/C3 must separate RDP `Z_CMP` from `Z_UPD`/`ZMODE` and map independent depth writes |
| Primitive submission order | VERIFYING | RE-122 fixed texture-cache key state loss, but RE-217 found `merge_by_material` still globally groups primitives; R2.2/C4 must preserve adjacent runs and measure draw-call impact |
| PSP GE state cache | VERIFYING | RE-118 fixed the known overlay texture invalidation; RE-217 identifies a broader direct-GU inventory and `invalidate_all` regression still required by R2.2/C5 |
| Culling | COMPLETE | `PLAN.md` R0.6: RDP per-frame default (`CULL_BACK` on) fixed, measured 86.3% of packed primitives post-fix (RE-068) | None |
| Transparency | COMPLETE for classified formulas | RE-135 measured 25/35 translucent billboard primitives already carry RE-130's real `ALPHA_BLEND` path | The remaining 10 are the same documented `PRIM_ALPHA`/two-cycle long tail, not a billboard-specific gap |
| Runtime MObj display state | COMPLETE for measured static content | RE-194 measures all 665 real `MObjMaterial`s and implements default substitution, texture scale, and tile-0 window state. `MOBJ_FLAG_FRAC` has 0 real static occurrences; tile-1 scroll is inert because all 12 inputs equal tile 0 and no `TEXEL1` consumer exists | Future runtime gameplay can revisit dynamic-only state if a real caller requires it; no R1 renderer gap remains |
| Effects / particles | COMPLETE for R1 renderer scope | RE-172–189 cover manager descriptors, transforms, material/texture/colour animation, all 160 particle scripts, pack serialization, host/device interpretation, spawn-tree execution, `LBGenerator`, PSP drawing, and one real manager-effect runtime spawn event | Remaining manager-effect gameplay call sites belong to later gameplay integration, not renderer completeness |
| Shadows | NOT STARTED | Only `shadow_size` — a fighter attribute constant extracted from `FTAttributes` — exists; nothing renders a shadow | No design exists; not yet a numbered R0.x task |
| Framebuffer effects | COMPLETE for R1 renderer scope | R0.13/RE-146–149 cover LB-transition capture and all 11 wipes. RE-190–193 census all remaining framebuffer references, implement 1P wallpaper capture plus its real `SObj` 2D-sprite draw, and device-verify bounded output | Real results-screen and match-transition triggers belong to G2 gameplay integration |
| UI | NOT STARTED | Debug overlay only, via `sceGuDebugFlush` (software-rasterizer-dependent, RE-014) — not real GE geometry | "Renderer 3" below is explicitly not started |
| Physical PSP | IN_PROGRESS | RE-201–215 provide representative Slim/6.61 captures for stages, fighters, materials, framebuffer effects and texgen; the formal matrix still lacks exhaustive coverage and the original-N64 texgen comparison; RE-218 reopens filtering/addressing correctness ahead of the texgen queue | R2.0/P0–P1, then R2.1/T1–T10 and R2.2/C1–C6 |

## Pipeline

```
ROM (relocData file)
   │  ssb-rom::archive          ← VPK0 + relocation      [DONE, verified]
   ▼
decompressed asset bytes
   │  ssb-rom::dl               ← F3DEX2 command decode  [DONE, unit-tested]
   ▼
command stream + Vtx arrays
   │  converter (build time)    ← state tracking          [IMPLEMENTED, partial coverage]
   ▼
intermediate representation: mesh + material
   │  packer                    ← swizzle, PSM, index     [IMPLEMENTED, partial coverage]
   ▼
PSP vertex/index buffers + material records
   │  psp::gu                   ← sceGumDrawArray         [IMPLEMENTED, partial coverage]
   ▼
GE
```

## Measured usage (`romtool scan`)

Plan §8: *"Measure actual Smash usage from the decompilation. Do not assume all
N64 rendering features are needed."*

Display lists are discovered by scanning every aligned offset and keeping the
ones that **convert cleanly** — a real list fills its own vertex cache before
drawing, so garbage fails almost immediately. Using relocation targets as
candidate list starts does *not* work: most reloc targets are the vertex-array
pointers carried by `G_VTX`, not list starts.

Result: **135 files, 1,864 display lists, 28,089 triangles, 0 conversion
failures.**

**Opcodes actually emitted** (re-measured for R0.16/RE-119; the previous
table predated several conversion-fidelity fixes — e.g. RE-093/RE-094's
texture-state corrections — that changed how many triangles the same
1,864 lists parse into, and predated RE-105's discovery that
`G_MOVEWORD` is genuinely load-bearing, not unused):

| Opcode | Count | | Opcode | Count |
|---|---:|---|---|---:|
| `G_TRI2` | 13523 | | `G_LOADTLUT` | 1558 |
| `G_RDPPIPESYNC` | 9015 | | `G_NOOP` | 1484 |
| `G_VTX` | 4756 | | `G_SETOTHERMODE_L` | 1444 |
| `G_SETTILE` | 4327 | | `G_TRI1` | 1043 |
| `G_MOVEWORD` | 3722 | | `G_RDPTILESYNC` | 1006 |
| `G_RDPLOADSYNC` | 3445 | | `G_DL` | 826 |
| `G_SETTIMG` | 3074 | | `G_SETBLENDCOLOR` | 366 |
| `G_SETTILESIZE` | 2218 | | `G_SETPRIMCOLOR` | 246 |
| `G_SETOTHERMODE_H` | 1954 | | `G_SETENVCOLOR` | 95 |
| `G_GEOMETRYMODE` | 1894 | | `G_RDPFULLSYNC` | 16 |
| `G_LOADBLOCK` | 1886 | | `G_SETFOGCOLOR` | 3 |
| `G_ENDDL` | 1864 | | | |
| `G_SETCOMBINE` | 1660 | | | |
| `G_TEXTURE` | 1596 | | | |

`G_MOVEWORD` is real and load-bearing (RE-105): its `G_MW_LIGHTCOL`
index is the one unambiguous, ROM-verified signal that a segment relies
on externally-enabled `G_LIGHTING`, used to decide per-vertex lit/literal
classification in `mesh.rs` (R0.6). It was wrongly listed as "never
emitted" until this re-measurement.

**Never emitted** — not worth implementing: `G_QUAD`, `G_CULLDL`, `G_BRANCH_Z`,
`G_MODIFYVTX`, `G_TEXRECT`, `G_FILLRECT`, `G_LOADTILE`, `G_SETSCISSOR`,
`G_MOVEMEM`.

`G_TRI2` outnumbers `G_TRI1` 18:1, so geometry is overwhelmingly paired
triangles. `G_SETFOGCOLOR` appears twice in the entire game — **fog is
effectively unused** and should not cost anything at runtime. Verified
more than just rare (RE-072): no `gSPFogPosition` call exists anywhere in
the decompilation to give a fog range meaning, and the one real stage
that sets a fog colour never references it from its own render mode.

Of the 1360 `G_SETCOMBINE` commands, 79 (5.8%) read `ENVIRONMENT`, and
91% of those match one shape the converter's `combiner_shade_scale`
cannot fold into a vertex-shade scale: `(PRIM-ENV)*TEXEL+ENV`, a
texture-driven blend with no shade dependence, on 28 files including
three fighters' own base models (RE-073). Detected
(`combiner_texture_blend`, packed via `pack.rs`'s `TEXTURE_BLEND` flag),
consumed by `psp/src/meshdraw.rs` via the GE's native `TextureEffect::Blend`,
and visually confirmed against Link's own model (RE-074) — see `PLAN.md`
R0.6.

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

**Geometry modes set:** `G_LIGHTING` (539), `G_SHADING_SMOOTH` (449),
`G_CULL_BACK` (399), `G_TEXTURE_GEN` (156), `G_SHADE` (60),
`G_ZBUFFER` (19), `G_TEXTURE_GEN_LINEAR` (13), `G_CULL_FRONT` (5). No
`G_FOG`. Counts are `set`-mask occurrences (`romtool scan`), not net
per-primitive state.

`G_SHADE` and `G_TEXTURE_GEN`/`G_TEXTURE_GEN_LINEAR` were not previously
listed — `romtool scan`'s own `geometry_mode_name` had `G_SHADE` mapped
to the wrong bit (`0x2`, disagreeing with `refs/ssb-decomp-re`'s own
`gbi.h`, which defines it as `0x4`), so its 60 real occurrences showed as
an unlabelled bit rather than under its real name; fixed (R0.16/RE-119).
Neither category has any explicit handling in `mesh.rs`'s `GeometryMode`
match arm, which currently reads only `G_CULL_BACK`/`G_CULL_FRONT`/
`G_LIGHTING`/`G_SHADING_SMOOTH`/`G_ZBUFFER`:

* **`G_SHADE`** ("enable Gouraud interp" per `gbi.h`; a real display list
  clearing it needs `PRIMITIVE` colour to show anything, not vertex
  shade) is always cleared together with `G_LIGHTING`/`G_SHADING_SMOOTH`
  in the same command, never re-set in that same command (checked
  archive-wide, RE-119) — consistent with a deliberate switch to flat,
  unlit, `PRIMITIVE`-driven rendering that this project's existing
  combiner-shape detection (`combiner_flat_color`/`combiner_texture_blend`,
  R0.6) likely already reproduces correctly for most cases, since those
  shapes don't read `SHADE` regardless of `G_SHADE`'s own state.
  Cross-referenced per-primitive against which specific primitives clear
  `G_SHADE` *and* have a combiner that still reads `SHADE` (RE-120): 31
  real occurrences archive-wide, 29 in content this project does not
  render yet (items, special-move effects, the opening movie), 2 live in
  Yoshi's Island's two stage variants — a narrow, single-primitive-per-stage
  gap left undecided rather than guessed at, since real hardware's actual
  output for this combination is not documented.
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

**`G_SETOTHERMODE_H`/`L` carry several independent sub-fields per command,
not just the cycle-type/render-mode ones `mesh.rs` originally read.** RE-124/
127 measured three (`TEXTFILT`/`TEXTLOD`/`TEXTDETAIL`); RE-195 measured the
remaining six `H` fields and both remaining `L` fields archive-wide. Six
match the RDP's own reset default exactly (`ALPHADITHER`, `RGBDITHER`,
`COMBKEY`, `TEXTCONV`, `TEXTPERSP`, `ZSRCSEL`); `TEXTLUT` is redundant with
`G_SETTILE`'s own format data already read; `PIPELINE` deviates from its
default but is an RDP scheduling hint with no visible effect. `G_MDSFT_
ALPHACOMPARE` is the one genuinely new, non-default field: 29.8% of real
commands request `G_AC_THRESHOLD`, a second, independent alpha-discard gate
from the existing `alpha_test` approximation. Decoded
(`MeshMaterial::alpha_compare_threshold`, `flags::ALPHA_COMPARE_THRESHOLD`)
and resolved onto the GE's single alpha-test unit by `pack::alpha_gate`
(RE-214). The two gates compose without a priority decision because the
cutout approximation is exactly `>= 1`: a threshold at a nonzero reference
already implies it, so `alpha >= reference` satisfies both; a threshold of
zero alongside the cutout stays `alpha > 0`, since `alpha >= 0` would pass
everything and silently drop the cutout; and a threshold of zero on its own
is a real no-op gate, expressed as one rather than strengthened. See RE-195
and RE-214.

Two hardware invariants are used as validity tests, and both earn their keep:
the vertex cache holds at most 32 entries, and triangle indices must fall
within it. Before those checks the scan reported an impossible 160-vertex
`G_VTX`.

## Geometry conversion results

`romtool mesh` converts every root display list into indexed meshes:

```
display lists converted  1768      (0 failures)
triangles                25562
triangle corners         76686
unique vertices          36693
vertex reuse             2.09x
draw calls after merge   2483
textured draws           1280

geometry memory
  triangle soup, float     1797.3 KiB
  triangle soup, 16-bit     898.7 KiB
  indexed, 16-bit           579.8 KiB
  saving vs float soup         67.7%
```

Three compounding wins, all paid for at build time:

1. **Indexing** — the RSP re-uploads shared vertices because its cache holds
   only 32. Undoing that gives 2.09x reuse.
2. **16-bit vertex components** — N64 positions are already `i16` and UVs are
   S10.5, so they map onto `GU_VERTEX_16BIT` / `GU_TEXTURE_16BIT` directly.
   12 bytes per vertex instead of 24. Converting to `f32` would double vertex
   bandwidth for no fidelity gain, on a machine that is bandwidth-bound.
3. **Material merging** — primitives sharing a material are merged into single
   draws, because GE state changes cost far more than draw calls.

All the game's geometry fits in **580 KiB**, comfortable against 32 MiB of main
RAM.

These are *root-list* figures and are deliberately smaller than the pack's
(2722 meshes, 47,696 triangles). This command converts each root display list
on its own; the packer walks each scene graph in draw order through one shared
vertex cache, which reaches the continuation lists a standalone conversion
cannot enter (RE-025, RE-026). Neither number is wrong — they measure different
traversals, and the gap between them is the scene-graph work.

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

## Display list translation

F3DEX2's model maps cleanly onto an indexed draw:

| F3DEX2 | Meaning | PSP |
|---|---|---|
| `G_VTX(count, dest, addr)` | load `count` vertices into the 32-entry cache at `dest` | append to a vertex buffer, remember the cache mapping |
| `G_TRI1(a,b,c)` | triangle from cache indices | three indices |
| `G_TRI2` | two triangles | six indices |
| `G_DL` (call/branch) | invoke another list | inline during conversion |
| `G_MTX` / `G_POPMTX` | modelview stack | becomes the DObj hierarchy transform |
| `G_SETTIMG`/`G_SETTILE`/`G_LOADBLOCK` | texture load | one converted texture, referenced by id |
| `G_SETPRIMCOLOR`/`G_SETENVCOLOR` | combiner inputs | material fields |
| `G_GEOMETRYMODE` | lighting/cull/fog bits | material flags |

Two F3DEX2 encoding traps, both handled in `ssb-rom/src/dl.rs`:

* **Indices are stored doubled.** `G_TRI1` holds `index * 2`; likewise `G_VTX`'s
  destination. Halve on decode.
* **`G_MTX`'s parameter byte is inverted** relative to F3DEX.

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
  rebasing. **RE-218 reopens both of the above as universal claims**: neither
  was checked against a full N64 tile-addressing reference model for
  coordinates beyond the first mirrored period (for authored UVs, not just
  texgen), for `mask == 0` tiles specifically, or against PSP's zero-filled
  power-of-two texture padding — see `PLAN.md` R2.0/P0b.

* Texgen is implemented and self-validated, but not closed: the ordered
  T1–T10 queue in `PLAN.md` has raw normal semantics (T2, RE-226), LookAt
  quantization (T3, RE-227), shared regular/linear reference math (T4,
  RE-228 — which also found and fixed a real overcorrected matrix constant)
  and linear integer conversion (T5, RE-229 — truncation, not rounding,
  against two independent HLE references, fixing a dormant rounding bug)
  measured and fixed; load-space provenance (T1's own cross-node gap),
  tile-shift/addressing proof and an original-N64 Metal comparison remain
  open (T6–T9). The current `G_TEXTURE_GEN_LINEAR` path is source-formula
  exact, not claimed bit-exact to N64.

* Renderer model corrections remain open under `PLAN.md` R2.2/C1–C7:
  single-source primitive-colour ownership, load-time lighting provenance,
  independent RDP depth compare/write state, adjacent-only primitive merging,
  and systematic PSP GE cache invalidation.

## Coordinate handling

See `docs/reverse-engineering.md` RE-004 and RE-005. Summary:

* **No handedness flip.** Both systems are right-handed, `+Y` up, view down `-Z`.
* **No matrix transpose.** N64 row-major/row-vector and PSP
  column-major/column-vector cancel out; only the s15.16 → `f32` widening is
  real work.
* **UVs** are S10.5 fixed point: divide by 32 for texels, then by the texture
  dimension to normalize.
* **Aspect ratio.** The game renders 320x240 (4:3); the PSP is 480x272.
  Stretching would distort every character, so the default is a pillarboxed
  362x272 viewport, centred. `coord::pillarboxed_viewport()` computes it, and
  `Gpu::init` must apply it to **both** `sceGuViewport` and `sceGuScissor` --
  feeding its aspect to the projection while leaving the GE viewport at the
  full 480 stretches the image by 480/362 = 1.33x, which is exactly the
  distortion the pillarbox exists to prevent (RE-034).

## Depth

The PSP's depth buffer is **inverted** relative to the intuitive setup: near
maps to 65535 and far to 0, so `sceGuDepthRange(65535, 0)` pairs with
`DepthFunc::GreaterOrEqual`. This is already set up in `psp/src/gu.rs` and is a
classic source of "everything renders in the wrong order" bugs.

## Renderer evolution

Per plan §32:

* **Renderer 0** — triangle testbed. ✅ Done; runs at a locked 60 FPS.
* **Renderer 1** — SSB stage geometry. ✅ Done. Stages render from the pack
  with their collision polylines overlaid (`docs/images/m4-stage-collision.png`).
* **Renderer 2** — complete materials/textures. 🟢 Mostly. Textures, CLUTs,
  per-node baked matrices and recovered `MObj` palettes all draw on device,
  stages included (`docs/images/m4-stage-textured.png`). Outstanding: the
  runtime-lighting deviation, the `MObj` fields listed in RE-010, and the
  26 runtime-framebuffer references that R0.13 owns.
* **Renderer 3** — transparency, fog, particles, shadows, UI. Not started. The
  debug overlay is still `sceGuDebugFlush` rather than GE geometry, which is
  why it needs the software rasteriser (RE-014).
* **Renderer 4** — batching, state sorting, caching. **Not before the game is
  visibly running**, and not before the state being merged/sorted/cached has
  passed its own correctness gate (D-036, `PLAN.md` R0.16/R2.2). The current
  build-time `merge_by_material` globally groups primitives, so submission
  order is not yet a safe correctness baseline; adjacent-run merging is owned
  by R2.2/C4. Further batching/state sorting remains an R3 optimization.

`PLAN.md` R0.18 tracks a systematic comparison against `sf64-psp` and
`oot-PSP` (both PSP targets, so their `sceGu` usage is directly comparable)
beyond the ad hoc BattleShip cross-references already scattered through this
document and `docs/reverse-engineering.md`. Treat all three as technical
references, not authorities, per D-037.

## Vertex layout

The GE reads vertex attributes in a fixed order and the `VertexType` flags must
describe exactly that order. `GuVertex` is therefore
`{ u, v, color, x, y, z }` — texture coords, then colour, then position.
Reordering those fields renders garbage with no error. It must also be
16-byte aligned for GE DMA, which is why vertex buffers are wrapped in
`psp::Align16`.


### Billboard rest-pose spin (RE-141)

The pack's billboard flags distinguish camera basis and spin independently.
`FLAG_BILLBOARD_SPIN_Z` selects the authored Z angle only for ROM Kind46;
Kind44/48/50 ignore authored rotation. Case45's X-angle convention belongs
to runtime-created transforms, outside the ROM descriptor path. Pack version
19 is required. `NodeDesc::billboard_rest_spin` supplies the PSP draw call;
posed matrices do not supply animated spin angles yet. All shipped selected
rest spin angles are zero; a synthetic nonzero-angle round-trip regression
pins the distinction without claiming an observed visual improvement.

### Animated billboard hierarchy and scale (RE-142–143)

Every packed animation was intersected with the 109 billboard nodes. Six are
direct stage-animation joints and none changes rotation in 240 frames; fighter
animations reference none. Another six have null scripts but descend from an
animated joint. `StageAnimator::compose` now propagates the parent transform
through those rest-local children, matching the original DObj hierarchy.

The persistent `billboard_animation_inventory` example records the affected
source graph/node, dynamic ranges, final scale, and first negative frame. All
six direct joints animate scale. Three Kind44 nodes reach small negative
uniform scales, which exposed the old unsigned matrix-column-length path.

RE-143 ports the original scale traversal directly. `gcPrepDObjMatrix`
computes billboard X as `ancestor_x * node.scale.x`, billboard Y as
`ancestor_x * node.scale.y`, then carries the new signed X value into children;
the tree walker restores it before visiting siblings.
`StageAnimator::billboard_scales` reproduces that calculation from current
local poses. The PSP animated-stage draw path uses the signed pair with the
model base scale, while static objects retain the already-verified
composed-column path. A regression with a negative animated parent proves that
both child axes inherit signed X, not the parent's Y. The scale acceptance item
is closed; per-node visual and physical PSP checks remain open.
