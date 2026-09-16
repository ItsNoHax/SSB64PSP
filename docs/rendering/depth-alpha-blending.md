# Depth, Alpha and Blending

Part of [docs/rendering.md](../rendering.md). Describes the current model; see `docs/evidence/re/` for how it was established.

## Depth

The PSP's depth buffer is **inverted** relative to the intuitive setup: near
maps to 65535 and far to 0, so `sceGuDepthRange(65535, 0)` pairs with
`DepthFunc::GreaterOrEqual`. This is already set up in `psp/src/gu.rs` and is a
classic source of "everything renders in the wrong order" bugs.


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


### Alpha

Status: COMPLETE for both classified gates, including their overlap

`PLAN.md` R0.6: `CVG_X_ALPHA \

**Remaining work:** ALPHA_CVG_SEL` decoded and wired to `sceGuAlphaFunc` (RE-069), matching `sf64-psp`'s own validated real-hardware approximation. RE-195 additionally decodes `G_MDSFT_ALPHACOMPARE` (a second, independent real discard gate, 29.8% `G_AC_THRESHOLD` archive-wide). RE-214 resolves both onto the GE's one alpha-test unit in `pack::alpha_gate`, with host regressions for every combination


### Blending

Status: COMPLETE for classified single-cycle formulas

RE-129/130 decoded alpha combiners, classified nine archive-wide shapes, and enable real blending for `TEXEL0_ALPHA` and `TEXEL0_ALPHA * SHADE_ALPHA`; PPSSPP-verified on Dream Land

**Remaining work:** Rare `PRIM_ALPHA` multiply (~43) and two-cycle (~93) primitives are measured and deliberately declined under R0.6


### Depth

Status: COMPLETE

RE-068/085 establish defaults and inverted PSP range; R2.2/C3 (RE-244–251) preserves `Z_CMP`, `Z_UPD` and `ZMODE` independently and drives `sceGuDepthMask` directly. The synthetic depth diagnostic pins ON→OFF→ON writes

**Remaining work:** Physical-PSP capture of the synthetic diagnostic remains a hardware-matrix follow-up, not a model gap
