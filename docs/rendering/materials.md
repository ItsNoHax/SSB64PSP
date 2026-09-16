# Materials

Part of [docs/rendering.md](../rendering.md). Describes the current model; see `docs/evidence/re/` for how it was established.

`G_MOVEWORD` is real and load-bearing (RE-105): its `G_MW_LIGHTCOL`
index is the one unambiguous, ROM-verified signal that a segment relies
on externally-enabled `G_LIGHTING`, used to decide per-vertex lit/literal
classification in `mesh.rs` (R0.6). It was wrongly listed as "never
emitted" until this re-measurement.


Of the 1360 `G_SETCOMBINE` commands, 79 (5.8%) read `ENVIRONMENT`, and
91% of those match one shape the converter's `combiner_shade_scale`
cannot fold into a vertex-shade scale: `(PRIM-ENV)*TEXEL+ENV`, a
texture-driven blend with no shade dependence, on 28 files including
three fighters' own base models (RE-073). Detected
(`combiner_texture_blend`, packed via `pack.rs`'s `TEXTURE_BLEND` flag),
consumed by `psp/src/meshdraw.rs` via the GE's native `TextureEffect::Blend`,
and visually confirmed against Link's own model (RE-074) — see `PLAN.md`
R0.6.


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

### Combiner

Status: COMPLETE for classified static paths

`PLAN.md` R0.6: general `(A-B)*C+D` evaluator (RE-039/043), texture blend (RE-073/074), flat colour (RE-080), and shade-scale consumption (RE-106). RE-168's post-RE-163 census accepts 65,000/65,199 source-attributed emitted-triangle visits (99.695%) and source-identifies every missing-constant case

**Remaining work:** The 186 unsupported-equation visits are catalogued; runtime shield colours belong to future effect/gameplay integration, not static material conversion


### Runtime MObj display state

Status: COMPLETE for measured static content

RE-194 measures all 665 real `MObjMaterial`s and implements default substitution, texture scale, and tile-0 window state. `MOBJ_FLAG_FRAC` has 0 real static occurrences; tile-1 scroll is inert because all 12 inputs equal tile 0 and no `TEXEL1` consumer exists

**Remaining work:** Future runtime gameplay can revisit dynamic-only state if a real caller requires it; no R1 renderer gap remains
