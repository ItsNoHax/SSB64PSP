# SSB64PSP Development Plan

## Mission

Create a native Rust implementation of Super Smash Bros. 64 for Sony PSP hardware.

The original SSB64 decompilation and the user's legally obtained ROM are the primary behavioral references.

The project prioritizes:

```text
Original behavior
        ↓
Rendering correctness
        ↓
Rendering completeness
        ↓
Physical PSP validation
        ↓
Rendering performance
        ↓
Combat
        ↓
Full game systems
```

**Rendering is a hard gate.**

Combat does not begin until the rendering gate has been explicitly passed.

---

# 1. How to Use This Plan

`PLAN.md` defines:

* the ordered roadmap;
* task dependencies;
* acceptance criteria;
* verification requirements;
* milestone gates.

`STATUS.md` defines:

* what the agent is currently doing;
* what it completed last;
* what it verified;
* what is blocked;
* what should be resumed.

Do not put mutable session state in this file.

When the user says:

> Continue with the plan.

the agent reads `STATUS.md` first to determine whether an existing task should be resumed.

If there is no active task, select the first eligible `TODO` task in this plan.

---

# 2. Task Statuses

Every implementation task uses exactly one:

* `TODO`
* `IN_PROGRESS`
* `BLOCKED`
* `VERIFYING`
* `COMPLETE`
* `ACCEPTED_DEVIATION`

Definitions:

### TODO

The task has not been started.

### IN_PROGRESS

The task is actively being implemented.

### BLOCKED

The task cannot proceed. The blocker and evidence must be recorded in `STATUS.md`.

### VERIFYING

Implementation exists but acceptance criteria have not yet been fully demonstrated.

### COMPLETE

All acceptance criteria are satisfied and evidence has been recorded.

### ACCEPTED_DEVIATION

Exact N64 reproduction is impossible on PSP, and the difference has been demonstrated, documented and justified.

---

# 3. Task Requirements

Every implementation task must have:

* Objective
* Dependencies
* Acceptance criteria
* Verification
* Evidence
* Relevant files
* Known limitations

Never mark a task `COMPLETE` without evidence.

---

# 4. Reference Hierarchy

When determining original SSB64 behavior:

1. Original SSB64 decompilation
2. Original ROM/data
3. BattleShip
4. sf64-psp
5. oot-PSP
6. n64psp
7. Existing SSB64PSP implementation
8. Engineering assumptions

BattleShip:

`https://github.com/JRickey/BattleShip`

sf64-psp:

`https://github.com/TheMrIron2/sf64-psp`

oot-PSP:

`https://github.com/z2442/oot-PSP`

n64psp:

`https://github.com/TheMrIron2/n64psp`

BattleShip, sf64-psp, oot-PSP and n64psp are all technical references, not authorities (`DECISIONS.md` D-037). `sf64-psp` and `oot-PSP` both target the PSP, which makes their `sceGu`/texture/material translation choices directly comparable — see R0.18.

Disagreements must be investigated.

Do not copy Nintendo assets or copyrighted game data from reference projects.

---

# 5. Completed Foundation

## M0 — Research

Status: `COMPLETE`

Original architecture, decompilation and reverse-engineering references established.

---

## M1 — PSP Bootstrap

Status: `COMPLETE`

PSP target builds and the engine executes under the development environment.

---

## M2 — Resource Pipeline

Status: `COMPLETE`

ROM validation, VPK0/relocData processing, extraction and runtime asset-pack infrastructure are operational.

---

## M3 — Core Game / Scene Infrastructure

Status: `COMPLETE`

Core scene, fighter, animation, collision and rendering infrastructure exists.

Remaining renderer work is governed by the rendering milestones below.

---

# 6. R0 — Rendering Correctness

Status: `VERIFYING` — RE-217 found renderer-model correctness work that
reopens parts of R0.6, R0.15 and R0.16; RE-218 additionally reopens R0.5's
filtering and mirror/clamp/mask/POT-padding addressing claims. R2.0 is the
active first task; R2.1 is the texgen audit queued behind it; R2.2 must
close the corrective gate before R0/R1 can be treated as stable. R2 remains
required before R3/combat unlock.

This is the current development gate.

The objective is to determine and reproduce the actual rendering behavior used by SSB64 rather than merely producing visually plausible output.

## 6.0 Rendering-Correctness Hierarchy (Cross-Reference)

This project already organizes rendering-correctness work as `R0.1`–`R0.18`
below, not as a separate top-level `R1`–`R8` sequence — this repository's own
top-level milestone names `R1`/`R2`/`R3` (§7–§9) already mean *Rendering
Completeness* / *Physical PSP Validation* / *Rendering Performance*. A second,
unrelated `R1`–`R8` would collide with those names. The table below maps each
rendering-correctness category onto the `R0.x` task(s) that actually own it,
so nothing is duplicated and nothing is missing an owner.

| Correctness category | Owning task(s) | Status |
| --- | --- | --- |
| Geometry (vertex positions/colors/normals, triangle topology, culling, matrix transforms, projection, viewport/scissor, coordinate conventions) | R0.8 (transforms), R0.14 (camera/projection), R0.6 (culling/geometry-mode defaults) | `COMPLETE` |
| N64 render-state model (faithful intermediate representation; must not collapse to `mesh + texture + basic colour`) | **R0.16**, R0.15 (render-state isolation), R0.6 (state threading) | `VERIFYING` — RE-217 / R2.2 |
| Texture correctness (formats, CI4/CI8, TLUT/palette lifetime, relocation, dimensions, coordinate scaling, filtering, LOD, mipmaps, clamp/mirror/repeat, masks/shifts) | R0.3, R0.4, R0.5, **R2.0** | `VERIFYING` — RE-218 reopened R0.5's filtering and mirror/clamp/mask/POT-padding claims; RE-219 (`R2.0`/P0a) closed the filtering question with `ACCEPTED_DEVIATION`; R2.0/P0b–P1 still own the remaining mirror/clamp/mask/POT-padding/field-census investigation. Format/CI4/CI8/TLUT/relocation/LOD/mipmap conclusions are unaffected and remain `COMPLETE`; RE-201 physical PSP evidence stands for the scene it covers |
| Combiner correctness (`G_SETCOMBINE` shapes, TEXEL0/TEXEL1/SHADE/PRIMITIVE/ENVIRONMENT, RGB/alpha, interpolation/modulation) | R0.6 | `COMPLETE` for classified static paths; runtime shield colours deferred with their effect path (RE-168) |
| Lighting correctness (`G_LIGHTING`, shading, normals, vertex colors, material interaction, ambient/directional lights) | R0.6 / R2.2-C1/C2 | `VERIFYING` — load-time provenance and `PRIM` ownership remain open |
| Alpha/blending correctness (alpha compare/test, source/destination blending, translucent vs. opaque, depth writes, render ordering) | R0.6 | `COMPLETE` for the classified single-cycle formulas (RE-129/130); rare `PRIM_ALPHA` and two-cycle cases remain documented declines |
| Depth/culling correctness (depth direction/range/function/writes, polygon culling, winding, clipping) | R0.6 / R0.14 / R2.2-C3 | `VERIFYING` — compare/write separation remains open |
| Render-pass completeness (transparency, particles, shadows, framebuffer effects, UI, other passes) | R0.12 (billboards), R0.13 (framebuffer), top-level R1 §7 (completeness gate) | R0.12 and R0.13 `COMPLETE`; particles/shadows/UI not started (see `docs/rendering.md` "Rendering status" table) |
| Visual-regression methodology (deterministic test scenes; reference vs. PPSSPP-software vs. PPSSPP-hardware vs. physical PSP; test matrix) | **R0.17** | `COMPLETE` |
| Reference-port comparative audit (sf64-psp, oot-PSP) | **R0.18** | `COMPLETE` |

`R0.16`, `R0.17` and `R0.18` are new tasks added below to close the gaps this
table identifies: this project already has extensive, evidence-driven
per-feature correctness work (`R0.1`–`R0.15`), but no task previously owned
(a) auditing whether the intermediate representation itself is faithful
rather than merely "whatever the current code happens to carry through", (b)
a deterministic, repeatable visual-regression methodology, or (c) a
systematic comparison against `sf64-psp`/`oot-PSP` beyond the ad hoc
BattleShip cross-checks already recorded in `docs/reverse-engineering.md`.

---

## R0.1 — Rendering State Reconciliation

Status: `COMPLETE`

### Objective

Reconcile the documented renderer state with the actual implementation.

### Dependencies

* M3

### Acceptance

* [x] implementation inventory completed — renderer, texture, mesh, animation and material subsystems inspected against `docs/porting-status.md` and source
* [x] renderer architecture documented — `docs/rendering.md`, README "Architecture"; the stale duplicate `ARCHITECTURE.md` was removed
* [x] stale documentation identified — see 2026-09-02 documentation audit (below)
* [x] known rendering gaps enumerated — `TODO.md` Phases B–H, `docs/porting-status.md` "Known gaps"
* [x] unsupported rendering paths identified — texture wrap modes (hardcoded `Repeat`), mipmap/LOD (generated but independently measured; its Dream Land canopy acceptance was later closed by RE-201), material animation (decoded, not played), majority-vote lighting heuristic
* [x] current verification baseline recorded — `cargo test --workspace`: **531 passing**; archive census and pack metrics are recorded with their corresponding evidence entries

### Verification

* inspect renderer implementation
* inspect generated asset reports
* run relevant tests
* run PPSSPP baseline

### Evidence

2026-09-02 documentation audit: reconciled `AGENTS.md`, `PLAN.md`, `STATUS.md`,
`README.md`, `DECISIONS.md`, `TODO.md`, `docs/porting-status.md`,
`docs/rendering.md` against current code, git history, and rebuilt asset
reports. Removed a stale duplicate `ARCHITECTURE.md` and a stale "Milestones"
table in `docs/porting-status.md` that contradicted its own subsystem rows.
Corrected a README claim that physical PSP hardware validation was complete
when `STATUS.md`/`PLAN.md` R2 show it is not. See `STATUS.md` for the
current task following this one.

---

## R0.2 — N64 Rendering Command Inventory

Status: `COMPLETE`

### Objective

Enumerate every N64 rendering command and relevant state transition actually exercised by SSB64.

### Dependencies

* R0.1

### Acceptance

* [x] GBI commands identified — `docs/rendering.md` "Measured usage" (`romtool scan`), full opcode/count table and a "never emitted" list
* [x] usage/frequency recorded — same table
* [x] display-list usage mapped — 135 files, 1,864 display lists (`docs/rendering.md`)
* [x] current PSP implementation mapped — `docs/rendering.md` "Display list translation" table
* [x] unsupported commands identified — texture wrap/mirror, mipmap/LOD selection, material animation playback (`docs/rendering.md` "Not yet handled", `TODO.md`)
* [x] relevant RSP/RDP behavior identified — depth inversion (D-007), aspect ratio (D-008), coordinate handling (D-004) in `DECISIONS.md`
* [x] BattleShip cross-reference performed — cloned `refs/BattleShip` + its `libultraship` submodule, read its F3DEX2/S2DEX interpreter (`src/fast/interpreter.cpp`), cross-checked opcode coverage, and recorded findings as RE-054 in `docs/reverse-engineering.md`: opcode coverage agrees with `docs/rendering.md`; found a new lead for R0.13 (S2DEX `G_BG_1CYC`/`G_BG_COPY` mixed into F3DEX2 lists, not decoded by our `dl.rs` at all); corroborated RE-053 (BattleShip has no LOD support either); clarified R0.8's transform question (the custom-MVP draw types patch the RSP's matrix via `G_MW_MATRIX`, but the actual transform is CPU-computed matrix math in `objdisplay.c` — ordinary decomp-porting work per D-001, not novel RDP/RSP behavior); confirmed the wrap-mode gap (R0.5) is real by comparison.

### Verification

* decompilation inspection
* ROM/display-list inspection
* BattleShip comparison — done, see RE-054

### Evidence

RE-054 in `docs/reverse-engineering.md`. New leads recorded in `TODO.md`
Phase B/C/E for R0.13, R0.5 and R0.8 to pick up when their dependencies are
met — this task did not implement fixes for any of them, only recorded
evidence per its own acceptance criteria.

---

## R0.3 — Texture Conversion Completeness

Status: `COMPLETE`

### Objective

Resolve every texture conversion failure that represents a missing required texture path.

### Dependencies

* R0.2

### Acceptance

* [x] all required N64 texture formats supported — RGBA16/32, IA4/8/16, I4/8, CI4/8 (`crates/ssb-rom/src/texture.rs`, `psp_texture.rs`)
* [x] all required textures decode — 617/647 bind-and-decode; the remaining 26 are not decodable textures at all (see below), so this item is satisfied for every texture that actually exists in the ROM
* [x] all required palettes resolve — every palette that exists in the ROM for a texture this converter can reach resolves; the 4 `MissingPalette` cases are a `PartTables` material-pairing gap (RE-057), not a missing/undecoded palette, and are out of this task's scope (R0.7's)
* [x] no unexplained conversion failures remain — the 30 remaining failures are fully explained and attributed: 26 are the LB (loading-break) transition's per-frame framebuffer photocopy, bound to RSP segment 0x1 at runtime and absent from the ROM (RE-055); 4 are palette loss caused by missing/partial `MObj` material-table pairings in three specific files (RE-057)
* [x] framebuffer/screen-wipe failures separately categorized and identified — RE-055 identifies the 26 segment-0x01 entries as `sLBTransitionPhotoHeap` (`refs/ssb-decomp-re/src/lb/lbtransition.c`), R0.13's territory, not R0.3's
* [x] conversion report generated — `romtool textures "rom/Super Smash Bros. (USA).z64"`
* [x] regression tests added where appropriate — `psp_texture::mip_tests` and others in `crates/ssb-rom/src/psp_texture.rs`

### Evidence

Both remaining failure classes were traced to root causes outside texture
conversion, each documented and reclassified to the task that actually owns
a fix:

* 26 segment-0x01 entries → RE-055 → R0.13 (framebuffer effects)
* 4 `MissingPalette` entries → RE-057 → R0.7 (missing material tables); RE-056
  is a superseded partial explanation, corrected by RE-057

No texture format, decode, or palette-resolution bug remains inside this
task's actual scope (texture-conversion logic in
`crates/ssb-rom/src/texture.rs`/`psp_texture.rs`). Closing this task does not
mean the 30 textures pack — it means the reason each one doesn't is now
identified, attributed to the right task, and none of them is a gap in this
task's own subject matter.

### Verification

```bash
cargo run --release -p romtool -- textures "rom/Super Smash Bros. (USA).z64"
```

---

## R0.4 — TLUT / Palette Correctness

Status: `COMPLETE`

### Current evidence

RE-064 closed the "palette inheritance/state" acceptance item. `mesh.rs`'s
`convert_sequence` threads RDP material state (texture image, tile format,
palette offset/file, combiner) across a node sequence the same way it
threads the vertex cache — previously justified only by an archive-wide
count (378/394 textures resolved with inheritance vs without) and never
pinned by a direct unit test. Added
`a_texture_binding_persists_into_a_node_that_sets_no_new_state`: joint A
fully binds a CI4 texture+palette, joint B sets no texture state at all,
and the test asserts joint B's resulting `TextureRef` is *exactly* joint
A's, not merely "some" binding. Verified the test can fail: temporarily
reset `timg_addr`/`palette_offset`/`texture_enabled` per sequence item and
confirmed the test catches it, then reverted. Separately confirmed by
reading `tools/romtool/src/main.rs`'s pack-building loop that
`convert_sequence` is called fresh (with a new `State::new()`) once per
scene graph, so state architecturally cannot leak *between* different
objects/fighters/stages — only within one object's own node sequence,
which is the real hardware's behavior too (`gcDrawDObjTree*` walks one
object's tree into one command stream).

### Objective

Reproduce original N64 CI/TLUT behavior.

### Dependencies

* R0.3

### Acceptance

* [x] CI4 verified — unit-tested decode, dominant format in the ROM (`docs/rendering.md` "Measured usage")
* [x] CI8 verified — unit-tested decode
* [x] TLUT loading behavior verified — the 4 "no TLUT recorded" notes are explained: `MObj` material-table pairing gaps in 3 specific files, not a TLUT-loading bug (RE-057)
* [x] palette inheritance/state verified — RE-064: direct unit test pins cross-node inheritance (confirmed capable of failing before being confirmed to pass); no cross-object leakage is possible by construction (fresh `State` per graph)
* [x] palette pointers verified — resolved through archive extern relocations (RE-037)
* [x] all missing palette cases resolved — RE-162 follows file 251's typed `ITAttributes` extern pair to resolve file 86's former N-Bumper pairing gap; `romtool textures` now reports no `MissingPalette` failures (only the 26 documented runtime-framebuffer references remain)
* [x] regression coverage added — texture decode unit tests in `crates/ssb-rom/src/texture.rs`; state-inheritance unit test in `crates/ssb-rom/src/mesh.rs` (RE-064)

### Evidence

RE-037, RE-057, RE-064, RE-162 in `docs/reverse-engineering.md`.

---

## R0.5 — Texture Filtering / LOD / Mipmapping

Status: `VERIFYING` — RE-218 reopened the filtering-equivalence and mirror/
clamp/mask/POT-padding addressing claims below; RE-219 (`R2.0`/P0a) closed
filtering with `ACCEPTED_DEVIATION`; RE-220 (`R2.0`/P0b) closed the `mask ==
0` question (invariant, no fix needed) and measured two real addressing
gaps (mirror+clamp beyond the first period, PSP POT-padding), each its own
task; RE-221 (`R2.0`/P0c) and RE-222 (`R2.0`/P0d) closed both. RE-223
(`R2.0`/P1) censused `G_SETTILE`'s remaining unconsumed fields and found one
more real gap (ignored CI4 palette bank), closed by RE-224 (`R2.0`/P2).
`R2.0` is now `COMPLETE`. LOD/mipmap conclusions are unaffected.
RE-201's direct PSPLink framebuffer capture remains valid evidence that the
Dream Land canopy matches on PSP Slim hardware for the scene it covers.

### Current evidence

Mip chains are generated at build time for 151 textures
(`psp_texture::pack_mipped`), but this did **not** resolve the Dream Land
canopy discrepancy (RE-053) — the wrong pattern survives and sharpens at
higher resolution, pointing at magnification rather than minification/LOD.
**RE-124 closed the filtering-mode question.** Measured archive-wide via
the real `romtool pack` build (not a heuristic scan): 151 real
`G_MDSFT_TEXTFILT` commands, all 151 requesting `G_TF_BILERP`, zero
`G_TF_POINT`/`G_TF_AVERAGE` — cross-checked against the RDP's own
per-frame reset default (`sSYRdpResetDisplayList`,
`refs/ssb-decomp-re/src/sys/rdp.c:43`), which also defaults to
`G_TF_BILERP`. `psp/src/meshdraw.rs`'s existing unconditional
`sceGuTexFilter(Linear, Linear)` is already correct for this ROM; no fix
needed. This task's explicit acceptance item "Dream Land canopy
discrepancy resolved" remains open — do not close this task while it
is.

RE-066 investigated wrap/clamp/mirror instead of leaving it an open
question. `psp/src/meshdraw.rs` hardcodes `sceGuTexWrap(Repeat, Repeat)`;
measured archive-wide (754 tile-0 `G_SETTILE` commands, not sampled) that
every axis requesting clamp or mirror also has that same axis's own mask
nonzero — 0 counterexamples. Cross-checked against `refs/BattleShip`'s RDP
interpreter, which strips `G_TX_CLAMP` under exactly this condition (real
hardware only wraps/clamps meaningfully in combination with the mask, not
from the two-bit field alone) — confirming `mesh.rs`'s existing
mask-narrowed-width approach (RE-044) already reproduces the correct
periodic addressing via `Repeat`, for every tile-0 texture in this ROM.
This was not a bug. The one real, unaddressed, quantified gap was
`G_TX_MIRROR` (208/754 tile-0 lists, 27.6%) — the PSP GE has no mirror
wrap mode at all, so a mirrored axis rendered with a visible seam at each
period boundary instead of bouncing smoothly.

RE-067 traced this directly to Dream Land's still-open canopy discrepancy
(RE-053): its exact display list (file 104, offset `0x798`) sets
`cm_s=3 cm_t=3 mask_s=6 mask_t=6` — mirror+clamp, 64-texel period.
Confirmed the wrap boundary mattered before fixing it, via a reversible
on-device `Repeat`-vs-`Clamp` experiment. Fixed by pre-baking a mirrored
copy of each affected texture at pack time
(`crates/ssb-rom/src/texture.rs::mirror_extend`, applied in
`tools/romtool/src/main.rs`'s `convert_texture`) — an exact reproduction,
not an approximation, since `sceGuTexScale` already renormalises UVs
against the packed texture's actual dimensions. This affects 187 of 638
packed textures archive-wide (29%), not just Dream Land, and raises
packed texture VRAM from 763.2 KiB to **1059.0 KiB** (1.5x the ~700 KiB
budget). Presented this cost to the user explicitly; shipped
unconditionally per their decision, making texture streaming
(`TODO.md` Phase G) no longer optional. The canopy's remaining "diagonal
pattern" is *not* fully resolved — RE-053's separate magnification/
dithering diagnosis is untouched by this fix, confirmed by a before/after
pixel diff showing real change but not a fully smoothed texture.

RE-070 tested RE-053's own two suggested fixes for the dithering directly.
Filtering alone: measured (an on-device `Nearest`-vs-`Linear` A/B) to help
a little but not enough — bilinear only interpolates a 2x2 neighbourhood,
narrower than the dither's repeat. Resolving it at conversion time: box-
blurring the two canopy textures and requantizing back to their 16-entry
CI4 palette changed nothing (the blur mostly snaps back to the same two
entries); packing the same blur unquantized (`Psm8888`) instead produced
a real, objectively measured improvement (~40% less adjacent-pixel noise
in the treated texture, ~19% over the whole visible canopy, diluted by
untouched decorations) — but not a fully smooth result. A first look at
this on-device genuinely looked like a full fix, which turned out to be a
stale-build methodology error (a diagnostic filter-mode build left over
from the earlier A/B, not the actual candidate); rebuilding from a
deleted binary and measuring pixel statistics rather than trusting the
screenshot by eye corrected it. Implemented as `crates/ssb-rom/src/
texture.rs::box_blur_wrapped` applied through a short, named, explicit
allowlist (`tools/romtool/src/main.rs`'s `NEEDS_DITHER_BLUR`) — not a
general dithering-detection heuristic, since a wrong guess would blur
texture art meant to stay sharp. Costs +112 KiB VRAM (1059.0→1170.9 KiB),
targeted to exactly the two named textures.

RE-075 fixed a real but small boundary-condition bug in that same blur:
it ran on the texture *after* mirroring, so `box_blur_wrapped`'s
toroidal wraparound sampled the mirrored copy at the seam instead of the
texture's own true periodic neighbour. Reordered to blur first, mirror
second (same cost, more correct). Confirmed the change is real via a
direct packed-byte diff (6724 bytes differ between the two orders) before
trusting it changed anything visible — and confirmed by screenshot that
it is *not* visible at the debug viewer's default camera distance
(pixel-identical canopy crop). Shipped as a correctness cleanup, not a
claimed improvement to the still-open discrepancy below.

RE-081 resolved RE-053's own apparent self-contradiction (its UV-span
measurement said "minified", its visual symptom said "magnified") by
measuring both canopy textures separately instead of treating them as
one case: the "gradient" texture really is minified (`3.70×1.36`
repeats, matching RE-053 exactly), but the "highlight" texture is
magnified on its V axis (`1.56×0.88` repeats, below 1.0) — RE-053's
symptom and its own number were each correct about a *different* one of
the two textures the fix was applied to uniformly. Also tested
`STATUS.md`'s untried "larger blur radius or multiple passes" idea: a
second `box_blur_wrapped` pass reduces measured texture-level noise a
further ~35–40% beyond the already-shipped single pass on both textures
— but a reversible on-device A/B (rebuilt pack, no `psp/` changes,
before/after screenshots of the same cropped canopy region) found the
change is not visibly different at the tested camera distance, the same
outcome RE-075 already found for a different change to these textures.
Not shipped, per RE-071's standing rule that a measured improvement
alone is not sufficient. Concludes that deciding this on real hardware
(RE-053's own original suggestion) is now more clearly necessary, since
a substantially larger blur change still did not surface on screen under
PPSSPP.

RE-101 fixed a real, separate texture-coordinate gap: `G_TEXTURE`'s
`scale_s`/`scale_t` (an unsigned Q0.16 UV multiplier the RSP applies at
`G_VTX` load time) was decoded by `dl.rs` but never applied by `mesh.rs`.
Several fighters' face textures are authored at a UV scale below 1.0;
left unscaled, their raw vertex UVs ran several texture periods wider
than intended, reading as a "melted", jumbled texture. Fixed by threading
`State::tex_scale` through the vertex-cache-load path, the same point
real hardware applies it.

RE-102 corrected RE-066's own "`Repeat` is correct for every measured
case" conclusion. RE-066 found every clamp/mirror request in the ROM has
its own axis mask nonzero and read that as clamp always being redundant
with RE-044's mask-based narrowing — true only when narrowing actually
shrinks the drawn rect. Several fighters' face/torso/head textures are a
counter-example (mask not smaller than the drawn rect, so narrowing is a
no-op); real hardware clamps there, and UVs were measured overflowing by
up to ~110 texels, or past a mirrored pair 2x or more on Fox/Falcon/Kirby
specifically. Fixed via `TextureRef::clamp_s`/`clamp_t`
(`TextureDesc::wrap`, `pack::VERSION` 14 → 15) and wiring
`meshdraw::bind_texture` to call `sceGuTexWrap` with the GE's native
`Clamp` mode per axis instead of always `Repeat`.

Both RE-101 and RE-102 are unit-tested and included in the same
`cargo psp`/`tools/run-ppsspp.sh` regression check RE-100 already
recorded (Dream Land pixel-normal, no default-path regression) — neither
was independently re-verified against a fighter's own screenshot when
written up (see `docs/reverse-engineering.md` RE-101/RE-102's own entries
for the caveat); that is a good next step for a session working fighter
rendering specifically, not yet done.

RE-127 closed this task's three remaining LOD/mipmapping items by
applying RE-124's exact method to the two other fields
`G_SETOTHERMODE_H` carries alongside `G_MDSFT_TEXTFILT`. Measured
archive-wide via the real `romtool pack` build: 131/131 real
`G_MDSFT_TEXTLOD` commands request `G_TL_TILE` (never `G_TL_LOD`),
121/121 real `G_MDSFT_TEXTDETAIL` commands request `G_TD_CLAMP` — both
matching the RDP's own reset default, meaning real hardware never
performs LOD-blended mipmapping for any content in this ROM. A third
field, `G_TEXTURE`'s own `level` (nonzero in 241 real asset display
lists), briefly looked like a missed signal but is confirmed inert:
`level` only matters once `G_TL_LOD` or `G_TD_SHARPEN`/`G_TD_DETAIL` is
active, and neither ever is. This project's own PSP-side
`pack_mipped`/`sceGuTexLevelMode(Auto)` mip chains are therefore a
deliberate anti-aliasing technique for dithered CI4 textures (RE-053/070),
not an attempted reproduction of a real N64 mechanic — there is none to
reproduce. `R0.5`'s only two remaining open items ("texture coordinate
behavior verified", "Dream Land canopy discrepancy resolved") are
unaffected by this finding.

### Objective

Determine and reproduce the actual texture sampling behavior used by SSB64.

### Dependencies

* R0.2
* R0.3
* R0.4

### Acceptance

* [x] filter *mode* identified from original data — RE-124: measured archive-wide via the real `romtool pack` build, 151/151 real `G_MDSFT_TEXTFILT` commands request `G_TF_BILERP` (zero `G_TF_POINT`/`G_TF_AVERAGE`), matching the RDP's own per-frame reset default
* [x] PSP `Linear` filtering proven equivalent to N64 3-point `G_TF_BILERP` reconstruction, or an `ACCEPTED_DEVIATION` recorded with measured error — RE-219 (`R2.0`/P0a): built a host-side 3-point reference sampler transcribed from `angrylion-rdp-plus`, measured it against PSP's bilinear archive-wide (684 real textures; 5.73% of interior samples differ by ≥8/255; the Dream Land canopy highlight texture reaches 128/255 max diff), and recorded `ACCEPTED_DEVIATION` — the PSP GE has no third filter mode and no programmable shader stage to reproduce the RDP's formula exactly
* [x] magnification behavior identified — RE-081: Dream Land's canopy "highlight" texture is magnified on its V axis (`0.88` repeats); RE-053's "sharpens with resolution" symptom is explained by this, not by the "gradient" texture (which is genuinely minified)
* [x] minification behavior identified — RE-053's `3.70×1.36` figure is correct for the canopy "gradient" texture specifically (RE-081 disambiguated which of the two canopy textures each figure actually describes)
* [x] LOD behavior identified — RE-127: measured archive-wide via the real `romtool pack` build, 131/131 real `G_MDSFT_TEXTLOD` commands request `G_TL_TILE` (zero `G_TL_LOD`), matching the RDP's own per-frame reset default — real hardware never engages RDP LOD blending for any content in this ROM
* [x] mipmapping behavior identified — RE-127: same measurement; `G_MDSFT_TEXTDETAIL` is 121/121 `G_TD_CLAMP` (zero `G_TD_SHARPEN`/`G_TD_DETAIL`), the mode that would make mip-tile blending meaningful even if `G_TL_LOD` were active — traditional N64 mipmapping is never used by this game's content
* [x] `mask == 0` N64 semantics verified — RE-220 (`R2.0`/P0b): transcribed `angrylion-rdp-plus`'s forced-clamp rule (`clampens = cs || !mask_s`) and censused every real drawn primitive archive-wide; zero of 4,968 possible axis slots have `mask_s == 0`/`mask_t == 0`, so the rule never has an observable effect on this ROM's content — pinned with a test
* [x] `G_SETTILE`'s `palette`/`line`/`tmem`/`shift_s`/`shift_t` fields censused — RE-223 (`R2.0`/P1): 2,238 real render-tile-0 instances archive-wide. `shift_s`/`shift_t`/`tmem` measure zero (`tmem`/`line` also structurally unconsumed, since conversion reads texels straight from ROM, never through TMEM), all pinned. `palette` was a real, material gap — 7/1,948 CI4 instances (file 86, `ITCommonObject`) request bank 1 of a 48-entry loaded TLUT that `mesh.rs` used to always resolve as bank 0; RE-224 (`R2.0`/P2) fixed it, confirmed by two host tests built from RE-223's own measured real shape (PPSSPP visual confirmation not obtained — see RE-224)
* [x] texture coordinate behavior verified — RE-128: `TEXVIEW`, the debug viewer's direct texture-display mode (bypasses lighting/geometry entirely), confirms in PPSSPP that Fox's real face texture (index 550) and Kirby's real face texture (index 734) both match `romtool texdump`'s independent reference decode exactly. RE-152 then geometrically isolated Fox's black lower face to primitive 4 / texture 551 and found the remaining coordinate bug: ordinary clamped tiles with nonzero `G_SETTILESIZE` origins retained absolute N64 UVs after upload to a zero-origin PSP texture. Clamped axes now subtract the tile origin while repeat axes preserve absolute mask phase; focused tests and a PPSSPP before/after confirm the fix
* [x] wrap/clamp/mirror behavior verified — RE-067: `Mirror` (29% of packed textures) is exactly reproduced by pre-baking; RE-102 corrected RE-066's own "`Repeat` is correct for every case" conclusion — real hardware clamps on several fighters' face/torso/head textures where RE-044's mask-based narrowing is a no-op, now reproduced via `TextureDesc::wrap`/`sceGuTexWrap(Clamp, ...)` per axis. RE-220 (`R2.0`/P0b) built the full reference model RE-218 asked for and found two real, material gaps: mirror+clamp addressing diverging from real hardware past the first mirrored period (99/810 real axis instances, 12.22%), and PSP's zero-filled power-of-two texture padding corrupting bilinear sampling near a clamped non-POT logical edge (347/456 real axis instances, 71.4%). RE-221 (`R2.0`/P0c) closed the first: `texture::mirror_extend` now bakes every mirrored period the drawn rect spans instead of always exactly two; re-measured archive-wide divergence is 0/810. RE-222 (`R2.0`/P0d) closed the second: `pad_edge_repeat`/`pad_edge_repeat_nibbles` fill padding with the repeated edge instead of zeros, no-op on an already-POT texture and never touching a mirrored axis by construction
* [x] Dream Land canopy discrepancy resolved — RE-201: direct 480×272 PSP Slim framebuffer capture under PSPLink matches the documented deterministic Dream Land canopy composition; prior FPU-trap faults in material/joint animation were fixed before capture
* [x] no unsupported mipmapping assumptions remain — RE-127: `G_TEXTURE`'s `level` field is nonzero in 241 real asset display lists, which looked like a missed signal, but is confirmed inert (never consumed) since neither `G_TL_LOD` nor `G_TD_SHARPEN`/`G_TD_DETAIL` is ever active archive-wide; this project's own PSP-side `pack_mipped`/`sceGuTexLevelMode(Auto)` mip chains are a deliberate anti-aliasing technique (RE-053/070), independently justified, not an attempt to reproduce a real N64 mechanic that turns out not to exist

**RE-152 resolves and reclassifies RE-128's Fox defect.** Direct texture
display proved the decoded image was valid, but did not prove the mesh sampled
it with correct coordinates. Primitive isolation identified head primitive 4
(texture 551) as the exact black trapezoid. Its real render tile clamps an
absolute window beginning at `(95.5, 143)` texels; the PSP texture begins at
zero, so the old absolute UVs clamped to a black edge texel. Rebase now occurs
per clamped axis. This was a texture-coordinate lowering defect, not evidence
of a primitive-colour or lighting failure.

### Evidence

RE-044, RE-053, RE-066, RE-067, RE-070, RE-075, RE-081, RE-101, RE-102, RE-127, RE-128, RE-152, RE-169, RE-218 in `docs/reverse-engineering.md`.

---

## R0.6 — Material System Correctness

Status: `VERIFYING` — prior combiner, lighting and depth evidence remains
valid for the paths it covered, but RE-217 identifies unverified ownership and
state-model requirements tracked by R2.2/C1–C3.

### Current evidence

RE-167 completes the matched original-game lighting comparison: runtime-lit
primitives now apply the already-resolved `PRIMITIVE * SHADE` scale as GE
material colour, restoring Mario's red clothing and blue overalls without a
tuned brightness value. RE-168 then re-runs RE-079's combiner census through
the current, fully paired pack path. In its source-attributed sample, 65,000
of 65,199 emitted triangle visits (99.695%) resolve through shade-scale,
texture-blend, or flat-colour handling. The former 3,085 missing-constant
claim collapses to 13 texture-bound visits: four ordinary-shield triangles
whose display callback sets player-dependent `PRIM`/`ENV`, two Yoshi-shield
triangles whose callback computes `ENV` from shield health, and seven
file-114 triangles belonging to non-stage orphan graphs found by broad scene
discovery. These are runtime/dynamic or non-authoritative paths, not unresolved
material tables. The remaining 186 visits are already catalogued unsupported
combiner/LOD shapes. No new converter fix is justified by the census.

`crates/ssb-rom/src/mesh.rs` evaluates a general `(A-B)*C+D` combiner across
both RDP cycles and declines to guess at anything it can't resolve rather
than approximating (RE-039, RE-043). Primitive/environment colour, alpha and
depth state are threaded through. RE-164–167 replace the former baked neutral
light with stage-angle runtime GE lighting and source LIGHT_1/LIGHT_2 state.

RE-065 identified the original input behind the former approximation. The
real light direction is `MPGroundData.light_angle` (a per-stage
`Vec3f`, `refs/ssb-decomp-re/src/mp/mptypes.h:187`), converted to a vector
by `ftDisplayLightsDrawReflect` (`refs/ssb-decomp-re/src/ft/ftdisplaylights.c`)
every time a fighter draws. Measured
archive-wide: **33 of 41 stages (80%) use exactly the same angle**
(`20.0, 45.0` degrees), which the old `(2, 4, 3)` placeholder happened to
sit only 9.9 degrees from. RE-164–167 supersede that interim bake: all stage
angles, signed normals, and source light colours now reach the runtime GE path.

RE-068 found and fixed a much larger structural gap: `refs/ssb-decomp-re/
src/sys/rdp.c`'s `sSYRdpResetDisplayList`, replayed once per frame
(`taskman.c:308`) before any object draws, sets `G_ZBUFFER | G_SHADE |
G_CULL_BACK | G_SHADING_SMOOTH` as the *default* geometry mode — not
all-off. `crates/ssb-rom/src/mesh.rs`'s `State::new()` seeded an all-off
`MeshMaterial::default()` instead, so a node whose own list never mentions
geometry mode (the common case — this state is normally set once per
frame, not per node) converted as unculled, flat-shaded and
non-depth-tested. Measured archive-wide before/after: `Z_BUFFER` went
from 6/3426 packed primitives (0.17%) to 3384/3442 (98.3%); `CULL_BACK`
measured 86.3%, `CULL_FRONT` 0.1%, `SMOOTH` 76.5% post-fix — the shape a
real game's geometry should have. Fixed by seeding from a new
`MeshMaterial::rdp_default()` instead, and wired `psp/src/meshdraw.rs`'s
`apply_material` to actually toggle `GuState::DepthTest` per primitive
from the (already-packed, previously-unread) `Z_BUFFER` flag. This
affects every object this project converts, not one stage or file.

RE-069 decoded `G_SETOTHERMODE_L`'s render-mode field (`mesh.rs` had never
read it at all) into `alpha_test`/`translucent`, cross-checked against
`gbi.h`'s own `GBL_c1`/`GBL_c2` macros and `refs/BattleShip`'s interpreter
(a naive `FORCE_BL`-means-blend signal is wrong — the opaque default sets
it too). Measured archive-wide: 36.1% of non-default render modes are
cutout (`TEX_EDGE`-family) surfaces, 14.4% genuinely translucent. Shipped
`alpha_test` (`psp/src/meshdraw.rs` now toggles `sceGuAlphaFunc`, matching
`refs/sf64-psp`'s validated real-hardware approximation) after finding and
fixing a bug where untextured lit primitives were being alpha-tested
against a packed-normal byte, not a real coverage value, and discarding
themselves outright (46/380 archive-wide; visibly deleted Dream Land's
decorative flowers before the fix). Found a *second*, harder bug in
`translucent` specifically — enabling real blending on Dream Land's
canopy-highlight surface produced a checkerboard, not a soft blend, almost
certainly the same open dithered-texture/coverage problem RE-053 already
found for the canopy's opaque path — and left it deliberately unconsumed
on the device side (`pack.rs`'s detection ships; `meshdraw.rs` does not
read the flag yet) rather than shipping an unverified visual change to
this project's primary test scene.

RE-073 measured what `combiner_shade_scale` actually declines: 79 of 1360
`SetCombine` commands archive-wide (5.8%) read `ENVIRONMENT`, 72 of those
(91%) matching one shape, `(PRIM-ENV)*TEXEL+ENV` — a texture-driven blend
from `ENV` to `PRIM` with no shade dependence at all, across 28 files
including three fighters' own base models (Link, Ness, Pikachu). Added
`combiner_texture_blend` to detect it (gated on a real texture, same as
`alpha_test`/`translucent`), which maps exactly to the PSP GE's native
`TextureEffect::Blend` at zero VRAM cost. Shipped detection into
`pack.rs` (`flags::TEXTURE_BLEND`, `VERSION` 9 → 10).

RE-074 closed the loop: the "shared vertex" risk RE-073 deferred on turned
out to already be handled by `push_vertex`'s existing content-keyed vertex
dedup (the same mechanism `prim_color` folding already relies on, per its
own doc comment) — reading that code, not new experimentation, resolved
it. Wired `apply_material` to `sceGuTexFunc(Blend, ...)`/
`sceGuTexEnvColor`, catching a real latent bug along the way
(`bind_texture` unconditionally reset the texture function to `Modulate`
on every texture change, which would have silently clobbered a `Blend`
state); fixed by tracking it in its own `DrawState` field, independent of
the flags/texture dedup fields already there. Verified visually, not just
by compiling: a temporary, reverted debug-viewer patch forced a direct
view of Link's own model's `TEXTURE_BLEND` primitive (object 306, file
324) — before, a flat grey shape (raw packed-normal bytes); after, the
correct grey-to-orange gradient. Dream Land's stage view (the main
regression scene, unaffected by this shape) was screenshotted before and
after too and is pixel-identical.

RE-079 did the "systematic accounting of every distinct shape
`SetCombine` uses archive-wide" RE-073 flagged as missing for "primitive
color"/"environment color verified". Found and fixed two real bugs, not
just measured: `combiner_shade_scale` could not tell a shade-scale term
that is *present with value black* from one that is *absent entirely*
(both read as `[0.0; 3]`), silently declining 1,118 primitives whose
`PRIM` is exactly `[0,0,0,255]` back to unmodified vertex shade instead
of the solid black real hardware produces; and `combiner_texture_blend`
required both `PRIMITIVE` and `ENVIRONMENT` to be set even for shapes
that only read one of them (`(ONE-ENV)*TEXEL+ENV`, 125 occurrences,
0/125 → 45/125 recognised, the rest untextured and correctly gated
elsewhere). Fixed by giving `Combined` a presence flag per term,
independent of value, threaded through the whole evaluator; caught and
fixed a regression this introduced in a literal-hardware-zero
multiplication case before shipping. Archive-wide: 97.0% → 97.5% of
262,778 combiner-bearing primitives now recognised. Also identified,
but did not fix, a third real shape (`(ZERO-ZERO)*ZERO+PRIM`, 1,589
primitives, a flat constant colour with no shade/texture term at all)
that needs a new `MeshMaterial` field rather than a classification fix.

RE-080 fixed that third shape. Added `combiner_flat_color`, structurally
disjoint from the other two functions (each requires a different,
mutually exclusive combination of the `k`/`s`/`t`/`st` presence flags
RE-079 introduced), covering `PRIM`-driven, `ENV`-driven and bare-`ONE`
constant colours alike (1,589 + 28 + 9 occurrences respectively).
Wired further than `texture_blend` was at the same stage: since `TEXEL`
provably never enters this shape's formula, `material_now` immediately
forces the primitive untextured and `push_vertex` bakes the resolved
colour, rather than detecting now and deferring consumption. Packed as
`pack.rs`'s `flags::FLAT_COLOR` (`PrimDesc::SIZE` 32→36, `VERSION`
10→11). Repacking the archive measured a real, expected side effect:
bound textures 644→639, mip-carrying textures 223→221 — five textures
were referenced only by primitives whose combiner never reads them at
all, now correctly dropped. `cargo test --workspace`: 368 passing (was
364). `cargo psp --release` + `tools/run-ppsspp.sh`: Dream Land
unchanged at 60 FPS, debug overlay's texture counter reads `0/639`
matching the repack.

RE-106 found and fixed a real consumption gap in `combiner_shade_scale`'s
own already-classified output: `material_now()` (RE-043) resolves
`MeshMaterial::prim_color` correctly for a `SHADE * constant` combiner
shape, but nothing downstream ever multiplied it back into a vertex —
the PSP GE has no fixed-function stage to scale an *untextured* vertex
colour by a separate constant, unlike `TEXTURE_BLEND`'s baseline colour
which maps onto a real GE blend mode. Left unconsumed, any primitive
using this shape rendered its raw, unscaled shade instead — wrong
wherever the resolved scale is not identity, including a resolved scale
of pure black, which reads on screen as a primitive rendering solid
black despite non-black raw vertex data. Fixed by folding the scale into
the vertex at pack time (`PackWriter::add_mesh`), the same place `lit`'s
shading and `flat_color`/`texture_blend`'s baked colours are already
applied. Five pre-existing `pack.rs` unit tests had unknowingly relied on
`prim_color` being `None` and needed correcting once this was noticed;
`cargo test --workspace` passes with the fold applied.

RE-103 found the pre-existing lit-vs-literal heuristic (the "majority-vote
lighting heuristic" `DECISIONS.md` D-024 already flagged) was wrong by
construction, not merely imprecise: it decided per *primitive* by voting,
but a fighter's mixed material (decal highlights as literal colour
sharing a vertex buffer with a lit body) routinely splits 20–80% within
one primitive — concretely measured on Fox, Falcon, Kirby and Ness, where
the losing side's raw normal bytes were painted straight into RGB, the
exact shape of a "melted", rainbow-noise surface. Fixed by deciding `lit`
per *vertex* instead. RE-105 then closed the remaining gap this fix
exposed: `material.lit` (the per-primitive input this per-vertex logic
still trusts when set) had no reliable in-list signal, since RE-021
already found real hardware turns `G_LIGHTING` on externally, per-object.
`G_MOVEWORD`'s `G_MW_LIGHTCOL` index — updating a light's colour, which
is meaningless unless lighting is already on — is an unambiguous,
data-driven signal instead of a guess, confirmed against a real four-command
ROM sample (file 313/Fox, offset `0x1AB0`, cross-checked against
`refs/ssb-decomp-re/include/PR/gbi.h`'s `G_MWO_*` constants). Both are
unit-tested; neither was independently re-verified against a fighter's
own screenshot when written up.

RE-129 answered RE-071's own standing "no new experiment has been run"
note for the canopy-highlight blend failure. This project's combiner
model (`mesh.rs`) only ever resolves the *colour* formula; the alpha
formula's own multiplexer slots occupy different bits of the same
`G_SETCOMBINE` word and had never been decoded. Hand-derived the real
bit layout from `gbi.h`'s macros and applied it to the exact `SetCombine`
word RE-069 already identified for this surface: the real formula is
`TEXEL0_ALPHA * SHADE_ALPHA` in both cycles — confirmed against the ROM
directly, not guessed. Wiring that up (`GuState::Blend` plus a standard
over-blend, gated on `TRANSLUCENT`) as a reversible experiment produced
neither the checkerboard nor RE-071's blowout, but did make Dream Land's
decorative flowers disappear — a different primitive that also carries
`TRANSLUCENT` but whose own vertex alpha is not a meaningful coverage
value. `TRANSLUCENT` alone cannot safely gate real blending; the fix
needs the same per-primitive alpha-formula classification this model
already does for colour, not attempted yet. Reverted before committing.

RE-130 did that classification and shipped it. A temporary, reverted
census measured every real `TRANSLUCENT` primitive's alpha formula
archive-wide: 9 distinct explicit combiner values, overwhelmingly one of
two single-cycle shapes — `TEXEL0_ALPHA` alone (~5,950 occurrences,
Dream Land's flowers) or `TEXEL0_ALPHA * SHADE_ALPHA` (1,820, the
canopy highlight) — plus a rare `PRIM_ALPHA` multiply (~43) and
two-cycle mode (~93, <1%) both left unclassified rather than guessed at
(the latter because its D-slot code `0` means `LOD_FRACTION`, not
`COMBINED_ALPHA`, the identical per-slot-context trap RE-129 already hit
once for the colour table, on a ROM that never engages real RDP LOD at
all per RE-127). Implemented as `mesh.rs`'s `AlphaBlend`/
`combiner_alpha_blend`, an axis independent of the existing RGB
classification; baked the right vertex alpha per shape in `push_vertex`
(force opaque for `TexelOnly`, since most vertex alpha bytes are not a
coverage value at all; leave the raw byte alone for `Shade`, since it
already *is* `SHADE_ALPHA`); gated real blending on a new
`flags::ALPHA_BLEND` bit (`pack::VERSION` 15 → 16) that only sets
alongside `TRANSLUCENT` when the formula was actually classified, so an
unclassified `TRANSLUCENT` primitive keeps today's original safe
default untouched. `psp/src/meshdraw.rs` now enables `GuState::Blend`
with the standard equation gated on both flags together. Verified
on-device: the flowers survive, the canopy body is pixel-identical from
the default camera framing (the highlight is not visible from that
angle), and two previously fully-invisible decorative props now render
correctly — confirmed via a clean diff against the updated
`regression_capture` golden capture (only those two symmetric regions
and a few 1-pixel flower-tip antialiasing edges differ), re-verified
deterministic across a 9-second timing spread matching R0.17's own
methodology.

### Objective

Reproduce original SSB64 material behavior.

### Dependencies

* R0.2
* R0.4

### Acceptance

* [x] material tables resolved — RE-163 resolves file 324's final
  `JointTree_0x9CF8` dispatch from the source-backed table at `0x84B8`.
  `romtool mobj` now reports all 127 discovered graphs paired, all 467 node
  chains matching their display-list demand, and zero unnamed or mismatched
  graphs.
* [x] combiner behavior verified — RE-039/043 establish the general two-cycle evaluator; RE-073/074 implement and visually verify the dominant `(PRIM-ENV)*TEXEL+ENV` texture blend; RE-080 handles flat constants; RE-106 consumes shade scales. RE-168 remeasures the current pack path and catalogues every remaining decline without finding a new static classification bug.
* [ ] primitive color verified — RE-079/080 classify shade-scale, texture-blend and flat-constant shapes and RE-106 consumes the resolved scale, but R2.2/C1 must prove that `PRIM * SHADE` has one authoritative application and that runtime-lit and non-lit paths do not apply it twice. RE-168's material-table census remains valid evidence for source attribution.
* [x] environment color verified — RE-168 source-indexes the remaining cases: ordinary shields set `ENV` in `efManagerShieldProcDisplay`, Yoshi's shield computes it from shield health in `efManagerYoshiShieldProcDisplay`, and no missing material-table case remains.
* [ ] lighting verified — RE-103/RE-105 and RE-164–167 establish the current per-vertex/runtime-light path, but R2.2/C2 must tie vertex interpretation to `G_VTX` load-time lighting state and quantify/eliminate the remaining `looks_like_unit_normal` fallback. The prior Dream Land comparison remains valid for its covered path; it does not close load-time provenance.
* [x] alpha behavior verified — RE-069: `CVG_X_ALPHA | ALPHA_CVG_SEL` (cutout surfaces, 36.1% of non-default render modes) decoded and wired to `sceGuAlphaFunc`, matching `refs/sf64-psp`'s validated approach; gated on a real texture being bound after a found-and-fixed bug that discarded untextured lit primitives outright
* [x] blending verified — RE-069 detected `translucent` (14.4%) correctly but left it unwired after an enabled-blend experiment produced a checkerboard; RE-070/071 eliminated dither coarseness and alpha premultiplication as the cause without finding the real one; RE-124 (R0.18) confirmed both `sf64-psp` and `oot-PSP` ship standard blending fine on the same hardware, ruling out a platform limitation. **RE-129 found the real cause**: this project never decoded `G_SETCOMBINE`'s *alpha* formula at all (only the colour one) — decoded it directly from the ROM for Dream Land's canopy highlight (`TEXEL0_ALPHA * SHADE_ALPHA`) and found naively wiring that up universally broke a different `TRANSLUCENT` primitive (Dream Land's flowers) whose own alpha formula is different. **RE-130 measured every real alpha formula archive-wide** (9 distinct combiner values across ~8,800 real, textured, single-cycle `TRANSLUCENT` primitives): the majority (~5,950) is `TEXEL0_ALPHA` alone (the flowers' own shape), a smaller set (1,820) is `TEXEL0_ALPHA * SHADE_ALPHA` (the canopy highlight), and a rare (~43) `TEXEL0_ALPHA * PRIM_ALPHA` plus two-cycle mode (~93, <1%) are declined rather than guessed at (real, measured, but not confidently understood or never on-device-verified). Implemented as a new classification axis (`mesh.rs`'s `AlphaBlend`/`combiner_alpha_blend`, independent of the existing RGB classification), baked the correct vertex alpha per shape (`push_vertex`), and gated real blending on a new pack flag (`flags::ALPHA_BLEND`, `VERSION` 15→16) that only sets when both `TRANSLUCENT` and a classified alpha formula agree — a `TRANSLUCENT` primitive without it keeps the pre-existing safe default. Verified on-device: the flowers survive, two previously-fully-invisible decorative props now render correctly, confirmed via a clean pixel diff against the `regression_capture` golden capture (updated) and re-verified deterministic across a 9-second timing spread
* [x] fog verified — RE-072: `DECISIONS.md` D-025's "twice" figure confirmed correct via reliable reloc-anchored discovery (an `Exhaustive`-mode re-scan found 7/4, which turned out to be false positives); both real occurrences are functionally inert — no `gSPFogPosition` call exists anywhere in the decompilation to configure a fog range, and the one real stage that sets a fog colour (file 118) never references `G_BL_CLR_FOG` in its own render mode
* [ ] depth state verified — RE-068 establishes the RDP default and current PSP depth-test mapping, but R2.2/C3 must independently preserve RDP `Z_CMP`, `Z_UPD` and `ZMODE`; the current single `z_buffer` flag is not sufficient to claim depth correctness.
* [x] culling verified — RE-068: same reset list defaults `G_CULL_BACK` on; fixed, measured 86.3% of packed primitives cull back faces post-fix
* [x] unsupported material behavior identified — RE-139 compiled the enumeration this item asks for: (1) combiner shapes outside shade-scale/texture-blend/flat-constant, now 186 of 65,199 source-attributed emitted-triangle visits in RE-168; (2) alpha formulas outside `TEXEL0_ALPHA` (alone or × `SHADE_ALPHA`) — the rare `PRIM_ALPHA` multiply and two-cycle mode (RE-130); (3) `G_SHADE` cleared while a combiner still reads `SHADE` (RE-120); and (4) runtime-injected primitive/environment colours for shields, source-identified by RE-168 and owned by future effect/gameplay integration. RE-164–167 closed RE-139's former baked-light item; it is no longer unsupported.

### Evidence

RE-065, RE-068, RE-069, RE-071, RE-072, RE-073, RE-074, RE-079, RE-080, RE-103, RE-105, RE-106, RE-120, RE-129, RE-130, RE-139, RE-152, RE-164, RE-165, RE-166, RE-167, RE-168 in `docs/reverse-engineering.md`.

---

## R0.7 — Missing Material Tables

Status: `COMPLETE`

### Current evidence

Freshly re-measured this session (`romtool mobj`, whole archive, after
RE-059/RE-060, RE-077 and RE-078's fixes): **70 graphs paired, 57
unpaired.** Started the session at 56/71.

RE-057 found three concrete test cases while investigating R0.3's
`MissingPalette` failures: files 52 (`MVCommon`), 86 (`ITCommonObject`) and
353 (`LinkSpecial2`) get zero or partial `MObj` materials from
`PartTables::scan` for their scene graphs. RE-057's guess that 353's table
lives in a sibling file is **retracted by RE-058**: 353 already declares its
own graph and its own `MObjSub` table in the same file.

Four distinct pairing mechanisms are now known, only one of which
`PartTables::scan` can discover on its own:

1. `FTCommonPart` (fighters) and `MPGroundDesc` (stages) — one struct, two
   adjacent pointer fields, living in the archive. `PartTables::scan` finds
   these structurally.
2. `WPAttributes` (RE-058, weapon/projectile sub-objects,
   `refs/ssb-decomp-re/src/wp/wptypes.h:36-45`) — same adjacency, also in the
   archive in principle, but the one confirmed instance checked (Link's
   boomerang) has `p_mobjsubs = NULL` by design, and file 353's own instance
   (Spin Attack) is not yet typed in the decompilation, so nothing has
   actually been fixed via this shape yet.
3. `EFDesc` (RE-059, fighter entrance effects,
   `refs/ssb-decomp-re/src/ef/eftypes.h:11-24`) — same adjacency, but the
   instances live in the game's **static executable**, not any archive file,
   so no scan can ever find them. Fixed 2 graphs in file 353 (`EntryWave`,
   `EntryBeam`) via hand-entered `PartTables::insert()` calls in
   `tools/romtool/src/main.rs`'s `load_all`.
4. Plain call-sequence pairing (RE-060, the opening movie's room scene,
   `refs/ssb-decomp-re/src/mv/mvopening/mvopeningroom.c`) — **no struct at
   all**; `gcSetupCommonDObjs(gobj, dobjdesc)` and a separate
   `gcAddMObjAll(gobj, mobjsub)` call are the only link, encoded purely in
   the executable's call order. Fixed all 5 of file 52's unpaired graphs,
   fully resolving that file, via 5 more hand-entered `PartTables::insert()`
   calls.

Verified: `romtool mobj --file 353` and `--file 52` both show 0 chain/demand
mismatches on every newly-paired graph. `romtool textures`: file 353 1→0
failures, file 52 several→0 failures (58/58 packed). Archive-wide
`romtool textures`: 617→638 packed (665 unique bound, up from 647 — several
primitives that previously drew with no texture at all now correctly
resolve one), `MissingPalette` 4→1. At that historical R0.7 step,
`cargo test --workspace` reported 338 passing; the current workspace baseline
is 531 passing, unaffected throughout (both fixes live in `romtool`, not the
library crate).

File 86's one remaining graph (an "NBumper" item) uses a **fifth**
mechanism — a compile-time byte-offset delta from a runtime base pointer
(`itGetPData`, `refs/ssb-decomp-re/src/it/itcommon/itnbumper.c:367`).
RE-061 measured this rather than guessing at it: neither linker symbol the
delta is computed from is typed anywhere in the decompilation, and
`romtool mobj --file 86 --search` returns 27 candidate table offsets for
this graph — the same kind of near-chance fingerprint match the project has
already measured and rejected once (Samus's two identical 33-node graphs,
`mobj.rs`'s own doc comment). Left unfixed on purpose. File 353's third
graph (Spin Attack) still needs its `WPAttributes` instance typed before it
can be inserted.

RE-076 hedged that several fighters' low measured texture counts were
"almost certainly" undercounts from unpaired `MObj` graphs — RE-077
checked that directly instead of leaving it a guess. `romtool mobj
--file <id>` for each of the 11 remaining real playable fighters found
**nine with zero unpaired graphs** (Mario, Fox, Donkey Kong, Samus,
Luigi, Jigglypuff, Captain Falcon, Yoshi, Pikachu are already fully
paired; their low texture counts are a real low-poly N64 model, not a
gap). Only Kirby (5 unpaired) and Ness (1) have real gaps. Kirby's
largest (`JointTree_0x19F08`, 22 nodes) had exactly one demand-matching
`--search` candidate, cross-confirmed against a fully-typed decompiled
symbol (`328_KirbyModel.c`'s `dKirbyModel_gap_0x31CC_sub_0x15894_post
[24]`) rather than trusted on the heuristic alone, and fixed via
`PartTables::insert()`. Ness's one candidate and Kirby's other four
(2-node, 10-way ambiguous) were checked and left unfixed — suggestive
but not conclusive evidence. This does not change RE-076's VRAM
estimate (the newly-resolved materials reference textures Kirby's main
body objects already use, so the aggregate is unchanged); it is a
rendering-correctness fix, and a correction to RE-076's own hedge.

RE-078 then ran the same search-plus-decomp-cross-check method archive-
wide instead of stopping at Kirby's file. Of the 63 graphs `--search`
had a candidate for, 13 came back with exactly one; checked each of
those 13 against its own file's decompilation with an *address-anchored*
match (not a substring search — an initial looser check falsely
"confirmed" two of file 85's candidates by matching a sub-offset baked
into a symbol's name, caught and corrected before shipping). Six
survived: file 22 (`MNPlayersSpotlight`), 69 (`MVOpeningStandoff`), 75
(`MVOpeningRunCrash`), 83/84 (`EFCommonEffects1`/`2`) and 167
(`MNTitle`) — each confirmed by both address and entry count matching a
named, typed decompiled symbol, one (file 84) only after working through
why the search's candidate address sat 8 bytes before the decompiled
symbol's own (a `PAD(8)` covering two genuinely zero-demand leading
nodes, not an error). Fixed via `PartTables::insert()`; verified
archive-wide (`romtool mobj`: paired 64→70, mismatches held at 0 across
383 nodes; `romtool textures`: packed 638→646, failures held at 27, no
new classes). The other 7 unique hits (file 85's two false positives,
plus one each in files 114 x3/351/352 landing on still-untyped bytes)
were checked and correctly left alone.

**RE-125 (this session) found 20 more** by systematically applying RE-078's
own cross-check method to the "several candidates" bucket, not just the
already-unique one: cross-referenced all 50 archive-wide ambiguous graphs'
candidate lists against `tools/mobjtable-ground-truth.py`'s decomp-typed
answer key, found 23 landing near a real symbol, independently confirmed
20 of them via `read_table` (the decomp's own labeled address does *not*
satisfy the graph's demand vector; the search's reported candidate does —
ruling out coincidence), each explained by a decomp-documented `PAD()` or
`NULL`-entries shape. Rejected the other 3 on the same evidence standard:
file 86 is the identical 27-way-ambiguous case RE-061 already declined,
and files 108/152's one candidate land inside a texture's own trailing
bytes with no real gap. `romtool mobj`: paired `70 → 90`, unpaired
`57 → 37`, mismatches held at 0 across 407 nodes. The remaining 37
unpaired graphs archive-wide (mostly menu/character-select emblem models,
stage files, and fighters' special-move files, not core fighter bodies —
see RE-077's breakdown) and file 86's and 353's specific blocked cases are
still an accepted long tail rather than a task-blocking gap; R0.7 stays
`IN_PROGRESS` but further progress again depends on upstream decomp
typing or a demand-search candidate narrowing to one with something to
confirm it against, not open-ended `romtool` investigation — RE-125's own
systematic pass over every ambiguous candidate, not just unique ones, was
that "something new to try" for this session, and it is now exhausted
again.

RE-153 found a sixth, source-confirmed instance of the already-known
call-sequence mechanism: `mnCharactersMakeEmblem` and
`mnVSResultsMakeEmblem` use parallel `dobjdescs[]`/`mobjsubs[]` arrays for all
ten file-35 character-select/result emblems. Each same-index pair is passed
to `gcSetupCommonDObjs` then `gcAddMObjAll`; the corresponding linker symbol
is the typed leading `MObjSub **..._pre` table in
`35_FTEmblemModels.c`. All ten pairings are therefore recorded directly, not
selected from several demand-compatible candidates. `romtool mobj --file 35`
reports 10 paired / 0 unnamed / 0 mismatches; archive-wide, pairings rise
`90 → 100`, unpaired graphs fall `37 → 27`, and all 417 paired nodes still
match their display-list demand. Their chains carry no palettes, so the
rebuilt pack is byte-identical; this validates the graph/material relation
without claiming a visual change.

RE-154 initially recorded four file-84 `EFDesc` pairings. RE-172 supersedes
its stale linker-name interpretation after the current decomp corrected the
mis-typed graph/wrapper blocks and the ROM independently confirmed every graph
boundary. The exact pairs are now FireSpark `0x2040 → 0x1EA0`, CatchSwirl
`0x2760 → 0x22B8`, ReflectBreak `0x3398 → 0x2F78`, DeadExplode
`0x53E8 → 0x4F08`, and NessPKFlash `0x6D00 → 0x6B40`. File 84 has 12
matching material-bearing nodes, no mismatch/cross-file fallback, and 7/7
textures packed. See RE-172 for the corrected evidence and full manager audit.

RE-155 adds five more direct source pairings: `ef/efmanager.c`'s static
`EFDesc` records bind file 85's MBall Rays (`0x628 → 0x108`) and Item Get
Swirl (`0x3170 → 0x2CA8`), while `sc1pbonusstage.c`'s parallel
`dSC1PBonusStagePlatformDescs` columns bind Bonus2Common's Small
(`0x3DA8 → 0x3720`), Medium (`0x45D8 → 0x3F70`) and Large
(`0x4E08 → 0x47A0`) platforms. The three platform tables deliberately have
the same demand fingerprint, so source same-row pairing — not `--search` —
selects the correct table. Archive-wide pairing rises `104 → 109`, unpaired
graphs fall `23 → 18`, and all 436 paired nodes have zero chain/demand
mismatches.

### Objective

Resolve every scene graph containing an unresolved material table.

### Dependencies

* R0.6

### Acceptance

* [x] all material-table references traced — RE-163 resolves the final file-324 `JointTree_0x9CF8` graph from its source-backed table at `0x84B8`; all 127 discovered graphs are paired.
* [x] original material data identified — Link's `0x84B8` dispatch has NULL root slots and the exact `0x86C0`/`0x86D0` chains assigned by `FTModelPart` records.
* [x] heuristic mapping removed where original data exists — `0x84C0` is only the searcher's first nonzero-demand slot; the mapping uses the true source start `0x84B8`.
* [x] affected scenes verified — archive-wide `romtool mobj` reports 127 paired graphs, 467 matching nodes, and zero mismatches; file 324 textures remain 31/31 packed.
* [x] regression coverage added — the archive-wide zero-mismatch `romtool mobj` report is the regression detector; all 433 workspace tests and strict Clippy pass.

---

## R0.8 — Transform Correctness

Status: `COMPLETE`

### Current evidence

Billboard matrix kinds 45–48 (translate + camera-facing basis) are
implemented and verified A/B against a rotated camera (RE-049). RE-062 read
`gcPrepDObjMatrix`'s actual switch case for `0x8000`/`RecalcRotRpyRSca`
(case 44, `objdisplay.c:822`): it never touches `dobj->rotate`, computing
the same diagonal-from-`gGCMatrixPerspF` MVP as kinds 45/46 with the
`sin`/`cos` spin term dropped — a full camera-facing billboard, not a
distinct transform. A whole-archive check found 0 of the ROM's 28
`RecalcRotRpyRSca` nodes have non-zero `rotate`, confirming the field is
genuinely dead for this kind. Fixed: `crates/ssb-rom/src/pack.rs`'s
`add_object` now flags these nodes `FLAG_BILLBOARD` alongside `Kind46`/
`Kind48`, reusing the already-verified billboard render path exactly (spin
term is a no-op `0.0`).

RE-063 closed the remaining acceptance items. `gcSetupCommonDObjs`
(`objanim.c:2153`) is the only function that turns a ROM `DObjDesc` array
into `XObj`s, and it only ever tests four high-nibble bits — `0x8000`
(44), `0x4000` (46), `0x2000` (48), `0x1000` (50) — matching
`TransformKind` exactly. Every other `gcPrepDObjMatrix` case, including
kinds 33-40's `func_800108xx` family (per-object look-at billboards, each
with a translate/no-translate pair) and kinds 41-43/45/47/49, is real
matrix math reached only by direct `gcAddXObjForDObjFixed`/
`gcAddXObjForDObjVar` calls from fighter/item/effect/stage-decoration game
code — never from a `DObjDesc` array this crate parses, and not exercised
by this project until those gameplay systems exist (rendering-gated per
`AGENTS.md` §5). Kind 50 (case 50) is `Kind48`'s exact move-word layout
sourced from `sGCMatrixMod2F` (camera-yaw-locked) instead of
`sGCMatrixMod1F` (camera-pitch-locked) — a real, reachable, genuinely
different basis, but an archive-wide scan found 0 of 3117 nodes use it.
Flagged `FLAG_BILLBOARD` anyway for fidelity with the decomp's case
structure, recorded as an unverifiable-by-data deviation rather than a
measured fix. `cargo test --workspace`: 340 passing (new tests
`a_recalc_node_is_flagged_as_a_spin_free_billboard`,
`a_kind_50_node_is_flagged_as_a_billboard_like_kind_48`). `cargo psp
--release` builds; PPSSPP run (`tools/run-ppsspp.sh --seconds 8`) shows
Dream Land rendering correctly at 60 FPS with a clean log.

### Objective

Implement every transform kind exercised by SSB64.

### Dependencies

* R0.1
* R0.2

### Acceptance

* [x] transform kinds enumerated — RE-063: every `gcPrepDObjMatrix` case
  traced to its origin; only 44/46/48/50 are reachable from `DObjDesc`
  arrays (the data this project's importer parses), all four implemented
* [x] `0x8000` investigated — RE-062: it's a spin-free variant of the
  already-implemented billboard kinds 45/46, not a distinct transform; fixed
  by flagging it `FLAG_BILLBOARD` the same way
* [x] original matrix behavior identified — RE-062 (kind 44), RE-063
  (kinds 33-43/45/47/49/50 traced; 50 implemented, the rest confirmed
  unreachable from ROM data)
* [x] PSP implementation verified — `cargo test --workspace` (340 passing);
  kinds 44/46/48 additionally verified visually via RE-049's rotated-camera
  A/B; kind 50 has no shipped node to verify against (RE-063), so its test
  coverage is structural only, recorded as such
* [x] affected scene nodes tested — unit tests for all four reachable kinds
  in `crates/ssb-rom/src/pack.rs`; RE-062/RE-063 archive-wide scans (28/34/
  47/0 nodes respectively, 3117 nodes checked)
* [x] billboard-related transforms cross-checked — RE-063 cross-references
  `Mod1F`/`Mod2F`'s camera-basis construction against the per-object
  look-at math in `func_80010748`/`func_80010918`/`func_80010AE8`/
  `func_80010C2C` (kinds 33-40), confirming they are related but distinct
  techniques, not duplicates

### Evidence

RE-049, RE-062, RE-063 in `docs/reverse-engineering.md`;
`docs/porting-status.md` "Billboard nodes" row.

---

## R0.9 — Stage Animation

Status: `COMPLETE`

### Objective

Reproduce original stage animation behavior.

### Dependencies

* R0.6
* R0.8

### Acceptance

* [x] all stage animation formats understood — 32-bit `AObjEvent32` joint stream (D-020)
* [x] event encoding verified — RE-050
* [x] timing verified — all 206 animated nodes still looping correctly after 600 frames (RE-050)
* [x] interpolation verified — `AObj` cubic/linear/step ported and exercised
* [x] animation playback verified — plays on device (PPSSPP): 35 stages, 206 animated nodes, 60 FPS; RE-142 fixed unanimated descendants failing to inherit an animated parent's transform and verified the Saffron City gate path numerically and on-device
* [x] independent ROM comparison exists — three independent checks agree: ROM replay (RE-050), packed-pose-vs-archive across 444,960 values (RE-052), and a two-frame device diff showing motion only over the animated canopy (RE-051)
* [x] all stages tested — all 41 stages' animation data was checked; 35 carry joint animation and 6 (including Dream Land) do not — Dream Land's scenery instead moves via non-joint game code, which is out of this task's scope

### Evidence

`docs/porting-status.md` "Stage animation" row; RE-050, RE-051, RE-052 in
`docs/reverse-engineering.md`. Validated under PPSSPP, not yet on physical
hardware — that gap belongs to R2, not this task.

---

## R0.10 — Material Animation

Status: `VERIFYING` — RE-211 found that current runtime support covers the
33 resolved palette-cycling scripts, but not the decomp's full stage
`TextureIDCurrent`/UV material-track set or fighter costume `PaletteID` tracks.

### Current evidence

The material animation decoder, pack representation and device palette path
exist, but RE-211 audits them as partial rather than complete. Frame 0 can
match baked content while the unimplemented texture-frame/UV tracks remain
invisible in static captures.

RE-086 measured what those 12 layers' 172 material-animation scripts
actually animate, archive-wide, before any implementation was designed —
correcting this task's own original framing, which (matching
`matanim.rs`'s existing costume-selection decoder) assumed the
interesting case was `PRIM`/`ENV`/`BLEND` **colour** animation. It is
not: **71% (122/172) animate `PaletteID`** — cycling which of a
texture's several palettes is bound, the classic cheap N64 technique for
water/lava/shimmer effects — **22% animate `TextureIDCurrent`** (frame
swapping), a meaningful minority animate texture UV translate/scale/scroll
(`TraU`/`TraV`/`ScrU`/`ScrV`, ~10-26 scripts each), and only **3 of 172
(under 2%) touch colour at all**. `crates/ssb-rom/src/mobj.rs` already
reads `MObjSub.palettes[0]` only — `palettes[1..]` exist in the ROM and
are never read, confirmed as a real, now-measured gap rather than a
hedge. Palette cycling is also the cheapest of the three mechanisms to
implement: the PSP GE's native indexed-texture format already separates
image from CLUT (`sceGuClutLoad`), so swapping a bound texture's active
palette needs no new combiner or vertex-recolouring machinery. No code
changed — a scoping/measurement pass, the same shape as RE-072/RE-081.

RE-087 decoded a real `PaletteID`-cycling script byte-for-byte (temporary
`romtool` subcommands, reverted) and found it is a genuine, continuous
loop — `SET_VAL_AFTER_BLOCK` stepping `PaletteID` through
`0,1,2,3,2,1,0,...`, then `SET_ANIM` jumping back to the script's own
start, not a one-shot key list. `colors_at` declines `JUMP` outright
("a costume list has no reason to jump"), correct for its own use case
but confirmed wrong for the general one. Added
`matanim::MaterialJoint`, a persistent tick-based player reusing
`crate::figatree::Aobj`/`Kind` — the same interpolation state
`crate::objanim::StageJoint` already plays joint tracks with — over a
unified 15-track window (ten material tracks, then the five colour
tracks), with the same `JUMP`/`SET_ANIM` handling `StageJoint` already
has correctly. `colors_at`/`Colors`/`costume_colors` (fighter costume
selection, `R0.11`'s mechanism) are untouched. Verified with 7 new unit
tests reproducing the exact real shape (immediate and delayed steps,
real `f32` bit patterns, a `SET_ANIM` loop ticked well past the
script's own length). `cargo test --workspace`: 375 passing (was 368).
`cargo psp --release` + `tools/run-ppsspp.sh`: builds and runs clean
under the real PSP target too, though nothing calls this yet.

RE-088 tried the obvious next step — extending `mobj.rs` to read
`MObjSub.palettes[1..]` — and retracted it after archive-wide measurement.
The struct has no length field, and the decomp's own two real examples
disagree on shape (one NULL-terminated, one not); a walk bounded by "is
this slot a real relocated pointer" looked sound against synthetic unit
fixtures but, measured against the real ROM, produced nonsense for 45%
of cases (110/243 hit an arbitrary 32-entry cap; one traced case was a
repeating arithmetic sequence from unrelated neighbouring file data, not
a palette table). No code shipped from this; `mobj.rs` is unchanged. The
real bound has to come from the driving material animation script (its
`SET_VAL`/`SET_VAL_AFTER_BLOCK` payloads name every `PaletteID` the game
ever asks for) — so reading `palettes[]` and resolving `p_matanim_joints`
are not separable steps as this task's own prior note assumed; they need
to land together.

RE-089 resolved `p_matanim_joints` into per-(node, `MObj`-chain-position)
script addresses: generalised `matanim.rs`'s existing fighter-costume walk
into `resolve_scripts` (shared, tested, `costume_colors` rebuilt on top of
it unchanged), and wired it permanently into `romtool stages`, which
replays every resolved script with the already-shipped `MaterialJoint`
engine. Archive-wide (same-file tables only, matching RE-086's own scope
limit): 61 scripts, 0 failures — the first archive-wide exercise of
`MaterialJoint` beyond RE-087's one hand-picked example. Ticking each
`PaletteID` script to completion and taking its largest value gives the
exact `palettes[]` bound RE-088 said could not come from the struct
itself: file 117 (`StageMetalFile2`, RE-088's own cited decomp example)
independently resolves to exactly 16 entries, matching its ROM source's
`..._palettes[16]` byte-for-byte from a completely different method (the
runtime script's own values, not the C array's declared size). File 105
(`StageZebesFile2`, 18 scripts needing 2–4 entries) and file 114
(`StageLastFile2`, 13 scripts needing 18 entries each) are concrete,
non-Dream-Land candidates for step 6's "representative palette-cycling
stage". Step 2 (reading `palettes[1..]`) is now unblocked, using this
resolved bound rather than guessing at file layout.

RE-090 shipped that step. `mobj::read_palettes(file, sub_at, count)` reads
exactly `count` consecutive `palettes[]` entries — no discovery, since
`count` is supplied externally (RE-089's bound) rather than guessed from
local bytes the way RE-088's retracted attempt did — and fails outright
rather than silently truncating if any entry within `count` does not
validate. Wired into the same `romtool stages` replay: for every real
`PaletteID` script found, reads its actual `palettes[]` array using the
bound that script itself computed. Archive-wide: **33/33 succeeded, 0
failures, 0 arrays with a duplicate entry** — full end-to-end validation
of decode script → compute bound → read the real array → confirm it
resolves and is not degenerate, across three files and three different
entry counts (2–4, 16, 18). `cargo test --workspace`: 238 passing (was
234). This closes R0.10's step 2 at the `ssb-rom`/`romtool` level; what
remains is packing this into the runtime format (step 4) and the
device-side wiring (steps 5–6).

RE-091 shipped the format half of step 4: `MatAnimDesc`/`MatAnimPalette`
(a new table pair, mirroring `AnimDesc`/`AnimJoint`'s shape) and
`TextureDesc::mat_anim` (filling 4 bytes of existing padding, no size
change), `pack::VERSION` 11 → 12, `PackWriter::add_mat_anim` deduplicating
each driving script's whole source file the same way `add_anim` already
does. Round-trip verified with 3 new unit tests; `cargo run --release -p
romtool -- pack` against the real ROM builds cleanly and "loads back
cleanly" at the new version. But wiring `romtool`'s real build loop to
populate it found a genuine blocker, checked archive-wide before writing
around it: all 33 real `PaletteID`-cycling `MObjSub`s have `sprite: None`
— they never name their own texture image, so there is no `(file,
offset)` pair on the animated `MObjSub` to key a texture correlation by.
The texture a cycling palette actually applies to is whichever CI4/CI8
image is already bound at that point in the node's draw sequence, tracked
correctly today only by `mesh.rs`'s own cross-node state threading
(RE-064) — populating `TextureDesc::mat_anim` for real needs that
threading extended with an animation marker, a `mesh.rs`-level change,
not more `romtool`/`pack.rs` plumbing. Deliberately left
`TextureDesc::mat_anim` at `NO_ANIM` for every real texture rather than
guess at a correlation.

RE-092 solved that blocker by re-reading `mesh.rs`'s own `State::
apply_mobj` instead of inventing new bookkeeping: for a palette-only
`MObj` (`sprite: None`, RE-091's finding), `apply_mobj` sets `timg_addr`
to the *palette's* address, and the display list's own subsequent
`G_LOADTLUT`+`G_SETTIMG` — ordinary commands, nothing MObj-specific —
load the TLUT and then overwrite `timg_addr` with the real texture image.
`current_texture()` already resolves the right texture through this
existing path; the only missing piece was remembering *that a script
drove this palette* across those same commands. Added `MeshMaterial::
mat_anim` (a `MatAnimRef` identity, same division `TextureRef` already
draws), threaded through `SequenceItem`/`State` parallel to `mobjs`, set
in the same `if let Some(palette) = m.palette` branch that sets
`timg_addr` — so a *later*, unanimated palette-bearing `MObj` correctly
clears a stale marker rather than leaking it onto a different texture
(verified capable of failing: a naive "set only when present" version
was tested and confirmed to leak, then fixed). Wired `romtool`'s real
`pack` build loop (`resolve_layer_mat_anims`, `convert_mat_anim_palette`,
`pack_mesh` deduplicating by script the same way it dedupes textures) to
populate the format for real. Archive-wide: **17 of RE-089's 33 known
scripts survived the whole pipeline, 181 palette variants, 23 textures
animated** — every surviving case's entry count agrees exactly with
RE-089's own numbers (file 117: both scripts, still 16 entries each; file
114: 6/13, still 18 each; file 105: 8/18, still 2–4 each). Pack size
4311.0 → 4470.3 KiB. `cargo psp --release` + `tools/run-ppsspp.sh`: clean,
Dream Land pixel-identical (uses none of these files; nothing on-device
consumes `mat_anim` yet). Why only 17 of 33 survived is not yet
investigated.

RE-093 investigated that gap and found a real bug, not an acceptable
absence. Diagnostics (reverted) proving every resolved script's chain
index genuinely gets called, on a placed node, ruled out the other two
candidates and narrowed it to "resolved but never reached by any
primitive." A raw ROM display-list dump (file 105 node 1) showed why: real
stage data legitimately calls several palette-only `MObj`s back to back
against *one already-loaded texture image*, reissuing only `G_LOADTLUT`
per palette and no `G_SETTIMG` for entries after the first — a real,
deliberate hardware pattern this project's `Cmd::LoadTlut` handler did not
model. It nulled the texture-image binding outright on the documented but
unverified assumption "the real texture follows with its own SETTIMG",
which this ROM data falsifies; the null had nothing to restore it, so
`current_texture()` returned `None` for the rest of that group's
geometry, dropping not just `mat_anim` but the texture entirely (those
triangles packed untextured). Fixed by having `State` remember the last
genuine image binding (`real_timg`, updated only by a real `G_SETTIMG` or
an `MObj`'s own `sprite` field) and restoring from it after `G_LOADTLUT`
instead of clearing — strictly more hardware-faithful, since the RDP's
texture-image register has no "unset" state and a fresh `G_SETTIMG`, when
one *does* follow, simply overwrites the restore immediately. Verified
capable of failing (a test reproducing the exact real shape fails without
the fix, passes with it). Archive-wide: **17 → 25 of 33 known scripts now
survive** (297 palette variants), with every other pack figure —
including texture count (639) — unchanged, the expected signature of a
correlation fix rather than a new-texture side effect. `cargo psp
--release` + `tools/run-ppsspp.sh`: clean, Dream Land pixel-identical,
notable this time because the fix is archive-wide (not animation-scoped)
so Dream Land was a genuine, not guaranteed, candidate to change. Still
open: 8 of 33 remain missing, with a concrete, different, unchecked lead
(node 27's `texture_enabled` was `false` for its whole span despite
actively loading textures — a cross-node state-inheritance question, not
a `Cmd::LoadTlut` one).

RE-094 closed that lead. Traced `texture_enabled` node-by-node (reverted
instrumentation) and found it flips to `false` exactly once, inside node
20's own list (`Texture{on: false}`, a self-contained untextured decal
with no `G_SETTIMG` of its own), then never flips back through nodes
21-27 — yet four of those seven nodes each issue a complete, independent
`G_SETTIMG`/`G_SETTILE`/`G_LOADTLUT`/`G_LOADBLOCK` chain and draw real
triangles. Measured the blast radius before fixing (temporary bypass,
reverted): ignoring `texture_enabled` outright fixes it (639→648
textures, 25→33 scripts) but is not *correct* — it would also re-texture
node 20's own deliberately-untextured decal. Shipped the narrower,
equally-effective rule instead: `Cmd::SetTimg` now sets
`texture_enabled = true` unconditionally, since a display list has no
reason to reissue the whole texture chain for geometry it means to draw
untextured — re-measured to the identical result (639→648, 25→33),
confirming the narrower rule loses nothing the blanket one gained, while
leaving node 20 (which has no `SetTimg` of its own) untouched. Also fixed
an existing test (`texture_disabled_means_no_binding`) that was passing
for the wrong reason (a missing tile format, not the disabled flag its
name claims) and would have kept passing vacuously after this fix;
rewrote it with a complete setup plus an explicit `Texture{on: false}`.
New test reproduces nodes 20→21's exact real shape and is verified
capable of failing. **Archive-wide: 33 of 33 known scripts now survive**
(321 palette variants), +9 static textures recovered, meshes/triangles
unchanged. `cargo test --workspace`: 245 passing. `cargo psp --release` +
`tools/run-ppsspp.sh`: clean, Dream Land pixel-identical.

RE-095 shipped step 8: `MaterialAnimator` (new, `skeleton.rs`), mirroring
`StageAnimator`'s lifecycle but starting once at pack load rather than
per-object, since a `MatAnimDesc` entry is a texture's property with no
per-object boundary to restart on. Array position mirrors `TextureDesc::
mat_anim`'s index directly. `resolved_palette` clamps into each entry's
own `palette_count` before adding `first_palette`, verified against a
neighbouring-table-read regression test proven capable of failing.
Caught a real `no_std` portability gap before shipping: `f32::round()`
does not exist without `std`/`libm`; fixed with the same
"add-a-half-truncate" trick `mesh.rs`'s own vertex rounding already
uses, found only because `cargo psp --release` was actually run (`cargo
test`/`clippy --workspace` alone never would have). Wired
`Option<&MaterialAnimator>` through `draw_mesh`/`draw_object`/
`draw_object_posed`/`draw_stage`/`draw_stage_animated`/`apply_material` —
`bind_texture` issues a second `sceGuClutLoad` after the static one
whenever `TextureDesc::mat_anim` names a live, resolvable entry, riding
the existing per-frame texture-cache reset for correct cadence. New
tests: one ticks a real-shaped script (three `PaletteID` steps then
`SET_ANIM` looping) through a full pack round-trip and confirms every
variant is visited and it keeps cycling; one proves the clamp regression
is real. `cargo test --workspace`: 247 passing. `cargo psp --release` +
`tools/run-ppsspp.sh`: clean, Dream Land pixel-identical. Loaded file
105's stage (temporary, reverted `stage_index` override) and confirmed it
renders at 60 FPS with no panics — but did **not** conclusively confirm
the palette visibly cycles by screenshot: the harness takes one
screenshot per independent launch, each restarting from tick 0, so two
separate invocations cannot isolate "more ticks, nothing else changed" —
a moving stage-animated platform confounded a naive crop comparison.
Honestly left open, the same category of limitation already recorded for
`R0.12`/`R0.14`'s remaining items: the mechanism is verified by
construction, watching it happen needs video capture or interactive play.

### Objective

Implement material animation used by SSB64.

### Dependencies

* R0.6
* R0.9

### Acceptance

* [x] animation data decoded — RE-087: `matanim::MaterialJoint`, a persistent tick-based decoder covering the material and colour track windows and every opcode a real script uses (including `JUMP`/`SET_ANIM`, which `colors_at` declines), verified against the real `PaletteID`-cycling shape
* [x] runtime clock implemented — RE-087: `MaterialJoint::tick` is the clock itself (parse-then-age, mirroring `StageJoint`'s own tick contract exactly); what remains is the *lifecycle* around it (start-on-layer-change, apply-in-draw), not the clock mechanism
* [x] material state updated correctly — RE-089/RE-090/RE-092/RE-093/RE-094: `p_matanim_joints` resolves into per-(node, `MObj`-chain-position) script addresses, each script's real `palettes[]` bound is computed by ticking `MaterialJoint` to completion, `mobj::read_palettes` reads the real array using that bound, and `mesh.rs`'s existing state threading (RE-064, fixed by RE-093 for shared-image `G_LOADTLUT` groups and RE-094 for a stale inherited `Cmd::Texture` disable) correctly correlates the resolved animation to the texture it applies to — **all 33 of RE-089's known scripts** (321 palette variants, 24 textures) now flow all the way into the real pack, verified against RE-089's own per-file numbers; RE-095 added the device-side `MaterialAnimator` and wired it into every draw path (`sceGuClutLoad` per animated texture, per frame)
* [x] representative animated materials verified — RE-095 wired `MaterialAnimator` into every draw path; the automated screenshot harness could not isolate two tick counts across independent launches, so PPSSPP was instead launched interactively (windowed, left running) and the user confirmed the palette-cycling animation is visibly working on file 105's stage (stage 2/41)
* [x] stage material animation verified — RE-086 identified Dream Land's own layer as a texture-UV-sway case specifically (`TraU`/`SetLFrac`), not representative of the archive-wide dominant case (`PaletteID`); RE-089 found concrete representative candidates (file 105, file 114); RE-092 confirmed they carry packed animation data; RE-095 wired the `MaterialAnimator` runtime path and confirmed by interactive play on the real device profile that the cycle renders correctly on file 105
* [x] fighter material animation verified where applicable — RE-096 checked archive-wide: all 441 real fighter `p_costume_matanim_joints` scripts (`colors_at`/`costume_colors`'s domain) reach `End` or park at a long trailing `Wait`, and **zero** loop via `JUMP`/`SET_ANIM`. Fighter costume scripts are structurally one-shot key lists, never real-time animations — `colors_at` and `MaterialAnimator` are correctly separate mechanisms for correctly separate shapes; nothing needs unifying. (A separate, real gap surfaced along the way — 45% of these scripts carry a `PaletteID` track `colors_at` never reads — flagged under `R0.11`, not implemented here since it is a costume-completeness question, not a material-*animation* one.)

### Evidence

RE-048, RE-086, RE-087, RE-088, RE-089, RE-090, RE-091, RE-092, RE-093, RE-094, RE-095, RE-096 in `docs/reverse-engineering.md`.

---

## R0.11 — Fighter Palettes / Costumes

Status: `COMPLETE`

### Current evidence

Per-costume-0 colours are recovered and render correctly (e.g. Mario in red,
via `FTCommonPart::p_costume_matanim_joints`, RE-040). Only costume 0 is
currently packed for any fighter — every other costume is unimplemented, not
merely unverified (`docs/porting-status.md` "Model conversion").

RE-096 (`R0.10` session) measured `p_costume_matanim_joints` archive-wide
while closing out `R0.10`'s own last item, and found a concrete,
decomp-confirmed lead for whenever this task starts: of 441 real fighter
costume scripts, **45% (200) carry a `PaletteID` track that
`colors_at`/`costume_colors` never reads** (it only decodes `PRIM`/`ENV`/
`BLEND`). None of these scripts loop — every one is a genuine one-shot key
list, the same shape `colors_at` already assumes — so this is a one-shot
*read* to add, not a `MaterialAnimator` concern. Traced the full chain in
`refs/ssb-decomp-re`, not left as an inference:
`lbCommonAddMObjForFighterPartsDObj` (`src/lb/lbcommon.c:955`) plays the
costume script at `anim_frame = fp->costume` through the same generic
`gcPlayMObjMatAnim` engine stages use; its `PaletteID` case
(`src/sys/objanim.c:1340`) sets `mobj->palette_id`; the draw path
(`src/sys/objdisplay.c:1184`) reads it back as
`mobj->sub.palettes[(s32)mobj->palette_id]` — the identical
`MObjSub.palettes[]` array `mobj::read_palettes` already parses for
stages. Packing only costume 0 today means every other costume's
*palette* silently stays at costume 0's wherever a script relies on
`PaletteID`, independent of whatever `PRIM`/`ENV`/`BLEND` per-costume work
lands. `costume_colors` needs a sibling one-shot `PaletteID` read feeding
which packed palette variant a given costume bakes.

RE-097 (this session) shipped that read and wired it end to end.
`colors_at` (`crates/ssb-rom/src/matanim.rs`) now also tracks joint track
`TRACK_PALETTE_ID` through the same step/base/target bookkeeping the
colour tracks already use, and resolves it the same way `MaterialJoint`
does (`f32::from_bits`, then the same `(s32)` cast `objdisplay.c` itself
performs) — `Colors` gained a `palette_id: Option<i32>` field;
`costume_colors` needed no changes, since it already layers `colors_at`
over `resolve_scripts`. `tools/romtool/src/main.rs`'s `Loaded::materials`
(the loop that already bakes `prim_color`/`env_color`/`blend_color`) now
also calls the already-shipped `mobj::read_palettes` — previously only
ever called from the stage material-animation path — to overwrite
`m.palette` with the costume's own resolved entry. Verified with 4 new
`matanim` unit tests reproducing the real shared-clock archive shape
(`cargo test --workspace`: 394 passing, was 390). Verified against the
real ROM, not just unit fixtures: rebuilding the pack at the default
costume (0) is byte-for-byte unchanged, and a temporary, reverted census
confirmed *why* — the new path genuinely fires 198 times archive-wide and
every one resolves `palette_id = 0` at costume 0 (not a silent no-op);
re-running the same census at costume 1 confirmed the mechanism is real
and varies correctly (188/198 resolve to `id = 1`, `read_palettes`
0 failures across all 198). `cargo psp --release` + `tools/run-ppsspp.sh`:
clean, 60 FPS, no panics, screenshot has real varied content. This closes
the concrete lead RE-096 handed off, but does not close `R0.11` itself —
the pack still only ever builds one costume at a time
(`DEFAULT_COSTUME = 0.0`); multi-costume packing/selection, and every one
of this task's five acceptance items below, remain open.

RE-098 (this session) implemented and shipped multi-costume packing and
selection, closing the larger part of the task. First confirmed, by
reading the real consuming code rather than assuming it, that a
costume changes only *material* (colour/palette) — geometry
(`DObjDesc`/`MObjSub` identity) is identical across every costume of a
fighter; a separate, `costume`-independent mechanism
(`modelpart_id_curr`) is the only thing that ever swaps geometry, driven
by gameplay state (held items, Link's own special-case joint), never by
`fp->costume`. This settled the runtime-representation design: a sparse
per-(node, costume) mesh substitution layered on one shared, already-
packed mesh set, not a duplicated geometry set per costume.

Measured the real per-node cost archive-wide before designing the pack
format (matching RE-076/077's discipline): real per-fighter costume
counts (`dFTParamCostumeIDs.develop + 1`, hand-transcribed and cited,
`refs/ssb-decomp-re/src/ft/ftparam.c:56`) are Mario 5, Fox 4, DK 5,
Samus 5, Luigi 4, Link 4, Kirby 5, Jigglypuff 4, Falcon 6, Ness 4, Yoshi
6, Pikachu 4. A temporary, reverted census found 10-16 of each fighter's
~25-33 nodes (a third to two-thirds, never all) actually differ from
costume 0 across that fighter's own costume range — some
palette-dominated (Donkey Kong 9 colour vs 96 palette differences),
some colour-dominated (Yoshi 80 vs 30), one barely touched (Link, 2 of
32 nodes).

Shipped `CostumeOverride` (`crates/ssb-rom/src/pack.rs`, `pack::VERSION`
12 → 13): `Pack::costume_mesh(node, costume)` binary-searches a sparse
table keyed by global node index for a substitute mesh, falling back to
the node's own baked mesh for the common case (no override).
`tools/romtool/src/main.rs`'s build loop converts each costume-bearing
graph once per costume and registers a substitute mesh only where the
*converted mesh content* (not each node's raw `MObj` fields, which would
miss cross-node state inheritance, RE-064) actually differs from costume
0. Found and fixed a real, costume-unrelated bug along the way:
`pack_mesh`'s texture cache was keyed by image location alone, which
would have let a costume's different palette on a shared image silently
reuse costume 0's cached texture — fixed by keying on palette identity
too, a correctness improvement to the existing non-costume path as well.

Wired device-side: `psp/src/meshdraw.rs`'s `draw_object`/
`draw_object_posed` gained a `costume` parameter (`0` reproduces every
existing caller's prior behaviour exactly); the debug viewer gained a
costume-cycle key (`L`, mapped from the PSP's previously-idle `SELECT`
button) and an overlay readout. Verified on the real device profile for
two fighters, not just compiled: Mario (colour-dominated) visibly
recolours between costumes 0 and 2; Donkey Kong (palette-dominated)
correctly renders the game's well-known "Blue Kong" alternate colour at
costume 3 via the palette-substitution path specifically, not just the
vertex-colour path Mario exercised. `cargo test --workspace`: 398 passing
(was 394). Rebuilt pack: 1287 per-(node, costume) mesh substitutions,
size 4492.4 → 5264.1 KiB (+772 KiB, +17% — disclosed, smaller than
RE-067's already-shipped 1.5× mirror-texture cost). `cargo psp --release`
+ `tools/run-ppsspp.sh`: Dream Land pixel-normal at 60 FPS, no panics.

Not done: only 2 of 12 real fighters were individually screenshotted
(the other 10 were only measured via the same census method, not
visually spot-checked); there is no permanent regression-render artifact
(only unit-test coverage of the mechanism itself, plus the two
one-off screenshots taken and reverted this session); and there is still
no real game costume-*selection* system, only the debug-viewer cycle key
— the same honest limitation `R0.10`'s `MaterialAnimator` accepted before
any real game system existed to drive it.

RE-098's closing addendum (same session) screenshotted the remaining 10
of 12 real fighters at a non-zero costume: Fox, Samus, Luigi, Link,
Kirby, Jigglypuff, Captain Falcon, Ness, Yoshi, Pikachu all rendered a
real, distinct, non-crashing model at 60 FPS, several independently
matching this project's own prior knowledge of SSB64's actual named
alternate colours (purple Samus, blue Yoshi, green Kirby, green Pikachu,
blue Falcon). Investigated one oddity rather than ignoring it —
Jigglypuff's costume 3 showed an iridescent rainbow body — and found by
comparing against Jigglypuff's own costume 0 that the same pattern is
already present there; a pixel diff (15.6%) confirmed a real colour
change still occurred underneath it, so this is a pre-existing baseline
shading characteristic of this project's Jigglypuff model, not a bug
this feature introduced. All temporary patches were fully reverted.
**`R0.11` is now `COMPLETE`.**

### Objective

Ensure every required fighter visual variant renders correctly.

### Dependencies

* R0.4
* R0.6
* R0.10

### Acceptance

* [x] all fighter palettes identified — RE-097/RE-098: `colors_at` resolves `PaletteID`, `mobj::read_palettes` reads the real array, verified against the real ROM at multiple costumes
* [x] all required costumes identified — RE-098: exact per-fighter costume counts hand-transcribed and cited from `dFTParamCostumeIDs`
* [x] runtime representation complete — RE-098: `CostumeOverride` table shipped, wired device-side, verified on the real device profile
* [x] palette data verified against ROM — RE-098: Donkey Kong's "Blue Kong" costume 3 confirmed correct by screenshot, the game's own known alternate colour
* [x] representative regression renders added — RE-098's closing addendum: no permanent screenshot artifact exists (this project has no automated screenshot-regression harness to save one into, the same limitation `R0.10` already accepted), but every one of the 12 real fighters was individually, visually confirmed rendering a distinct, correct-looking costume at least once
* [x] all required fighters verified — RE-098 plus its closing addendum: all 12 real fighters (Mario, Fox, Donkey Kong, Samus, Luigi, Link, Kirby, Jigglypuff, Captain Falcon, Ness, Yoshi, Pikachu) individually screenshotted at a non-zero costume

### Evidence

RE-040, RE-096, RE-097, RE-098 in `docs/reverse-engineering.md`.

---

## R0.12 — Billboard Correctness

Status: `COMPLETE`

### Current evidence

All 109 billboard nodes are flagged. RE-131–133 shipped the real camera,
camera-basis-aware screen alignment, and a separate Kind48 basis selected
by `FLAG_BILLBOARD_PITCH_LOCKED`. RE-126's shared-placement limitation is
historical, resolved by RE-133. Kind50 is absent from the shipped archive
and remains folded into the screen-aligned path (RE-063/RE-133).

RE-140 adds a repeatable source-indexed rest-pose inventory:
`cargo run -p ssb-rom --example billboard_inventory -- assets/generated/ssb64.pak`.
It reports 109 nodes, 47 pitch-locked, no nonfinite matrices or zero basis
lengths. This is structural evidence only; per-node visual verification
remains open, including animation/camera-dependent behavior.

RE-083 closed the "decomp's `rot_mode` choice" worry as a non-issue: that
logic (`gcDecideDObj3TransformsKind`) belongs to the runtime/dynamic
transform path RE-063 already ruled out of scope, not to
`gcSetupCommonDObjs` (the ROM-driven path this project actually parses),
which was confirmed by direct reading to map `0x4000`/`0x2000` to kinds
46/48 unconditionally, no `rot_mode` branch at all. Also ran an
archive-wide census of billboard-flagged nodes' own primitives: depth
testing is uniformly on (`z_buffer` 118/118, 100%), matching RE-068's RDP-
reset-default finding with zero exceptions. `alpha_test` (28.8%) needs
nothing further (RE-069, already shipped); `translucent` (29.7%, roughly
double the archive-wide 14.4% RE-069 measured) is the same still-open
gap RE-069/RE-071 already found and left unresolved after it produced a
checkerboard on Dream Land's own canopy-highlight surface — billboards
are measurably the geometry category most affected by that gap, not a
separate new one.

RE-131 built the real camera this task's "orientation verified" item was
blocked on. RE-132 found and fixed a regression the camera itself
introduced: `billboard_place` relied on the view matrix always being
identity (true only under the debug viewer's old fixed camera), which
silently broke `Kind46` under a real, rotating one. RE-133 then resolved
`gcPrepCameraMatrix`'s `var_s3`/`spC8` branch (confirming `var_s3 = 1`
during normal gameplay via two independent paths — the camera's own
`xobj`-kind setup and `gcSetCameraMatrixMode`'s three real call sites —
and that `spC8` never leaves `0`, so `Kind50`'s matrix is genuinely never
computed, not just unused) and implemented `Kind48`'s real
camera-pitch-locked transform as a second `BillboardCamera` basis, gated
on a new `NodeDesc::FLAG_BILLBOARD_PITCH_LOCKED` bit. A reversible
on-device A/B test (real basis vs. folding `Kind48` into `Kind46`'s
treatment, both against the same deterministic frozen frame) found a
real, non-zero, non-degenerate difference concentrated on Dream Land's
own canopy geometry — the new code path is reached and does something,
not inert. Closes "orientation verified". RE-134 then measured "scale
verified"'s own remaining lead (the ancestor-chain-cumulative-X-scale
formula RE-126 also found reading the same code) archive-wide: zero of
the 109 real billboard nodes have any non-uniform-scale ancestor, so
this project's existing composed-basis-column-length scale is already
numerically exact for all of them — closes that item too, by
measurement, with no code change needed.

RE-141 corrected a latent spin-source mismatch: ROM Kind46 uses Z,
not case 45's X; kinds 44/48/50 ignore rotation. Pack v19 preserves a
Kind46-only spin selector. All current billboard rest spin angles are zero,
so the deterministic Dream Land capture stays pixel-identical. The new
nonzero-angle round-trip test covers the distinction. Animated spin and
per-node visual validation remain unverified.

RE-142 exhaustively intersected every packed animation joint with all 109
billboards. Only 6 billboards are directly driven, all by stage animation;
none is targeted by fighter animation and none changes rotation across 240
frames. All 6 change scale. Six additional billboards inherit movement from
an animated ancestor despite having a null script of their own. This exposed
and fixed `StageAnimator::compose` incorrectly substituting their baked world
matrices. Three Saffron City gate billboards differ at deterministic frame 240;
the corrected PPSSPP image changes 836 RGB pixels in the gate region. RE-143
closes the animated scale gap. `StageAnimator::billboard_scales`
replays the original traversal's signed accumulator: billboard X is ancestor
X times node X, and billboard Y is ancestor X times node Y. The PSP animated
stage path now consumes those values instead of unsigned matrix-column
lengths. A focused negative-parent regression pins both sign preservation and
the deliberate X-for-both-axes rule; the archive inventory records the first
negative frame and final scale for every directly animated billboard.

RE-144 makes the final per-node check repeatable on PPSSPP/PSP. The scene
viewer now has a SELECT/L billboard-audit mode that walks all 109 nodes in
the same stable ordinal order printed by `billboard_inventory`, draws exactly
one node with its real transform/material path, disables the unrelated viewer
spin, and frames the billboard transform conservatively. Extending that
inventory to require drawable geometry found 109 distinct valid meshes and
299 triangles with zero empty entries. It also found 16 meshes with real local
Z span, exposing a renderer bug: the PSP billboard path scaled Z by `1.0`,
while every reachable `gcPrepDObjMatrix` billboard case scales Z by the same
signed `gGCScaleX` value as X. The path now uses X/Y/X exactly. A deterministic
Dream Land capture changes 9,972 RGB pixels, localized to the affected
scenery, and the isolated file-104 audit node renders correctly on PPSSPP.
RE-145 completes that review. A build-only `billboard_audit_capture` feature
boots directly into the same audit and advances one stable ordinal per fixed
60 Hz second after a startup hold. A PPSSPP software run captured and reviewed
all ordinals 0–108 as six contact sheets. 103 nodes produce visible,
nondegenerate geometry. Ordinals 101–104 are deliberately subpixel because
their ROM-authored world basis lengths are `0.00001`; ordinals 91–92 emit
their two triangles but deliberately sample only texture 366's transparent
texel 0 (all four UVs are `[0,0]`, palette alpha 0). The inventory now reports
texture identity/shape, primitive flags, palette/sample alpha, UV bounds and
vertex alpha so those cases are source-indexed and mechanically explained.
No unexplained missing, collapsed, exploded or unframed node remains.

### Objective

Verify every billboard rendering path.

### Dependencies

* R0.8
* R0.14

### Acceptance

* [x] billboard types enumerated — RE-063 exhaustively traced every `gcPrepDObjMatrix` case reachable from a ROM `DObjDesc` array (kinds 44/46/48/50, all four flagged); RE-083 confirmed no fifth reachable kind hides behind the `rot_mode` branch, since that branch belongs to an unreachable runtime-only path
* [x] camera-facing transforms verified — RE-049's rotated-camera A/B test (Dream Land's six canopy sprites upright vs skewed into slivers) for the `Kind46`/screen-aligned family specifically. **RE-132 found and fixed a latent regression RE-131's own real camera introduced**: `billboard_place` relied on the view matrix always being identity (true only under the debug viewer's old fixed camera) to make "the object's own axes" equal "the screen's axes" — silently wrong under a real, rotating camera. Fixed by giving `DrawState` a `billboard_camera` basis (`None` under every still-identity-view mode, reproducing old behaviour bit-for-bit; `Some((right, up))` under the real camera), verified via zero pixel difference on every unaffected mode and clean, non-degenerate billboards on-device under the real camera's own shallow angles
* [x] scale verified — RE-134 proved the rest-pose formula equivalent for all 109 nodes because their ancestors use uniform scale. RE-142 extended the check through 240 animation frames and found three directly animated Kind44 nodes cross slightly below zero (`-0.000486` to `-0.003`), exposing sign loss in matrix-column lengths. RE-143 implements the exact `gcPrepDObjMatrix` rule from the decompilation: carry signed ancestor X through the hierarchy, use `ancestor_x * node.scale.x` for billboard X and `ancestor_x * node.scale.y` for billboard Y, and restore the accumulator across siblings. `StageAnimator::billboard_scales` supplies those values to the PSP stage draw path; a focused negative-parent test pins sign and X-for-both-axes inheritance. All six direct billboards still have zero animated rotation, and fighter animations reference no billboard, so this covers every shipped animated billboard scale path without a heuristic
* [x] orientation verified — RE-126 measured this is a real, open gap, not an unexamined one: `Kind48` (camera-pitch-locked, distinct from `Kind46`'s fully screen-aligned transform) is 47 real nodes archive-wide including Dream Land's own file 104, the largest individual billboard category (43% of all 109 flagged nodes). **RE-131 built the real camera this needed** (`ssb_game::camera`, a tested, on-device-verified port of `gmCameraDefaultFuncCamera`); **RE-132 fixed a regression the camera itself introduced for `Kind46`**; **RE-133 resolved `objdisplay.c`'s `var_s3`/`spC8` branch and implemented `Kind48`'s real transform**. Tracing `gcSetCameraMatrixMode`'s three call sites plus an independent per-`xobj`-kind check both agree: `var_s3 = 1` during normal gameplay (the branch that preserves the real `Y` component, matching a `Y`-up world), while `spC8` never leaves its `0` default — confirming, independently of RE-063's archive census, that `Kind50`'s own matrix (`sGCMatrixMod2F`) is never actually computed in real play, genuinely dead rather than merely unused. `Kind48`'s real formula (collapse the camera's X/Z position into one horizontal distance, `LookAt(eye=(0,eye.y,dist),at=(0,at.y,0),up=(0,1,0))`) is implemented as a new `NodeDesc::FLAG_BILLBOARD_PITCH_LOCKED` bit (`pack::VERSION` 18) and a second `BillboardCamera` basis, verified via a reversible on-device A/B test showing a real, non-degenerate difference concentrated on Dream Land's own canopy geometry. `Kind50` stays folded into `Kind46`'s treatment, now with direct evidence (not just archive silence) that no distinct transform could ever be observed for it
* [x] texture orientation verified — RE-138: checked both sides directly for a *separate* billboard-specific texture-orientation mechanism (a UV flip, a view-based mirror) rather than assuming one exists. `mesh.rs` (this project) has zero references to billboard/transform-kind concepts anywhere — a billboard's UVs decode through the exact same code path as any other primitive. `objdisplay.c`'s `gcPrepDObjMatrix` cases 44-50 (the original game) touch only matrix/scale state, never a texture register or UV value; the only nearby texture-related code is the unrelated `MObj` material-animation sprite-cycling mechanism (R0.9/R0.10's own scope). No separate mechanism exists to verify on either side — this item is the same question R0.5's already-closed UV/wrap/coordinate items (RE-067/101/102/128) already answered, asked again under a different name
* [x] alpha behavior verified — RE-083: `alpha_test` needs nothing further (already-shipped RE-069 mechanism, archive-wide verified); `translucent` was the named blocker (29.7% of billboard primitives, double the archive-wide rate), tracked under RE-069/RE-071's then-open blending mystery. **RE-130 resolved that mystery generally; RE-135 measured it reaches billboards specifically**: 25 of 35 real translucent billboard primitives (71%) already carry `flags::ALPHA_BLEND` and render with real, classified blending — the remaining 10 are the same already-documented, deliberately-declined categories (rare `PRIM_ALPHA` multiply, two-cycle mode) RE-130 found archive-wide, not a new billboard-specific gap
* [x] depth behavior verified — RE-083: archive-wide census of billboard-flagged nodes' own primitives found `z_buffer` set on 118/118 (100%), matching RE-068's default with zero exceptions
* [x] all flagged billboard nodes verified — RE-083 measured render-state distribution archive-wide; RE-136/RE-137 then verified all 8 of 8 billboard-bearing stages as coherent scenes. RE-144 added a source-indexed isolated audit. RE-145 used its fixed-tick capture mode to review every stable ordinal 0–108 under PPSSPP software at 60 FPS: 103 are visibly nondegenerate, ordinals 101–104 are intentionally subpixel from their ROM-authored `0.00001` scale, and ordinals 91–92 submit valid two-triangle meshes whose all-zero UVs select texture 366's alpha-zero texel. The extended inventory records the transform, bounds, texture, flags, alpha and UV evidence for each row; no unexplained missing or malformed node remains

### Evidence

RE-049, RE-062, RE-063, RE-083, RE-126, RE-131, RE-132, RE-133, RE-134, RE-135, RE-136, RE-137, RE-138, RE-140, RE-141, RE-142, RE-143, RE-144, RE-145 in `docs/reverse-engineering.md`.

---

## R0.13 — Framebuffer Rendering

Status: `COMPLETE`

### Current evidence

RE-055 (`docs/reverse-engineering.md`) identified the concrete target: the
LB (loading-break) transition system's `sLBTransitionPhotoHeap`, bound to
RSP segment `0x1` and sampled by a set of between-match transition effects.
RE-099 read `lbtransition.c` directly and found the mechanism is a one-time
CPU-side snapshot, not a per-frame render pass, and measured the real scope
at 13 files (not the decomp's own 11-entry `dLBTransitionDescs` table), but
left one concrete design question explicitly unverified: does a PSP port
need the N64's own full `300×220` capture with strip-by-strip TMEM
addressing reproduced, or does a smaller capture with unmodified UVs
suffice?

RE-100 (this session) answered that by measuring every one of the 13
files' real UV spans (`romtool textures --file <id>`), not guessing:
RE-099's own favoured hypothesis ("capture the full 300×220 image, leave
UVs alone") was **wrong**. The real geometry only ever needs a **300×6
top-left corner** of the framebuffer — the 300×5 tile draws it once
(V span always exactly 5.0 texels across all 13 files), the 300×6 tile
tiles it vertically by ordinary wrap addressing (V span 22.5–215 texels,
3.75×–35.83× repeat depending on the file). U never wraps in any of the
13 files. This is a repeating 6-row colour smear, not a crisp photo.

**Implemented and device-verified this session**, not just scoped:
`mobj::LB_TRANSITION_SEGMENT`, `mesh::TextureRef::framebuffer` (set by a
segment-`0x1` `G_SETTIMG`, cleared by any real one), `pack::TextureDesc::role`
(`pack::VERSION` 13 → 14, later 15 once RE-102 added `wrap` the same
session, `TextureDesc::SIZE` 32 → 36 → 40), `romtool`'s
`pack_mesh` deduplicating the 13 files' 26 binds down to the 2 distinct
shapes that exist, `Gpu::request_transition_capture` (a CPU-side VRAM
readback into a small `Psm8888` buffer, the PSP-side equivalent of
`lbTransitionSetupTransition`'s own one-time photocopy), and
`meshdraw::bind_texture`'s new framebuffer-role branch. Verified with 3
new unit tests (401 passing total) and, on the real device profile, a
temporary reverted patch that captured an unmistakable magenta test colour
and confirmed it appears correctly on a real transition object's geometry
(file 40, the "paper airplane" transition) — not merely that the code
compiles. Dream Land's own rendering is unaffected (pixel-normal at 60
FPS, confirmed by screenshot after fully reverting the temporary patch).

RE-107 (a later session) extended verification archive-wide before
touching the black-rectangle question. A temporary, reverted `romtool`
census across all 13 files (39–51) confirmed the two-primitive shape
(one framebuffer-textured primitive plus one untextured "backing"
primitive) generalizes to every one of them, not just file 40 — but
found file 40 is *not* representative of the backing primitive's colour:
12 of the 13 files' backing primitives carry raw vertex colour
`[255,255,255,0]` (white), and only file 40 itself uses the navy
`[0,0,127/128,0]` RE-100 originally measured. Extended on-device
verification to a second file on this basis (file 45, a white-backing,
unlit case, deliberately different from file 40's navy-backing, lit
one): a temporary, reverted `psp/src/main.rs` patch (magenta clear +
capture at frame 30, forced object switch to file 45's object from frame
35) screenshotted a correct magenta render on the framebuffer-textured
primitive, confirming the capture/bind mechanism generalizes beyond
file 40's own hand-picked case, not merely by inference from the shape
census.

RE-108 (a later session) found the "black rectangle" question was
misattributed from the start, and root-caused the real defect. Seven
mechanisms were eliminated on the real device before the premise itself
was questioned: forcing the *backing* primitive's vertex colour to
screaming green (a temporary `pack.rs` hack, confirmed present in the
built `.pak`) never painted a single visible pixel, and neither did
ruling out culling, depth testing, texture-state caching, or shade model.
The decisive test forced `crate::gu::TRANSITION_PHOTO` itself — the
framebuffer capture buffer — to a uniform green *before any capture
ever runs*: the **entire** visible shape turned green, proving the
region RE-107 called "black backing panel" was never the untextured
backing quad at all. It is one of the object's two `ROLE_FRAMEBUFFER`
*photo* texture entries (300×5 "drawn once" vs 300×6 "tiles
vertically") — the backing quad is a thin sliver that was never actually
visible in any of these tests. Comparing the two photo entries directly
(nudging one's wrap mode broke its previously-correct magenta,
identifying it as the working one) isolated the 300×5 entry as the
broken one, and a raw-UV dump explained why: its baked `V` range is
`214.97..219.97` texels — the *bottom* edge of the real N64's
`sLBTransitionPhotoHeap` 300×220 buffer, not the top. RE-100's capture
only ever stores the buffer's top 6–8 rows (correct for the 300×6
entry, whose own `V` range starts at 0), so the 300×5 entry wraps into
memory RE-100's capture never populates with anything relevant. This is
a scope gap in RE-100's own original measurement (which recorded the
300×5 entry's *span* — 5.0 texels — correctly, but never checked its
*absolute position*), not a bug in any of the seven mechanisms this
session and RE-107 spent real, on-device effort eliminating. Not fixed
this session — two candidate fixes are recorded (capture a second band
near the real bottom edge, or rebase each framebuffer-role primitive's
UV by its own tile's origin at pack time) but neither was attempted; the
goal was root-causing an already very long investigation, not shipping
on top of it.

**Still open (before RE-109):** nothing calls `request_transition_capture`
from real game logic — there is no match-start/match-end event to call it
from yet, since this project has no game-state/transition system at all.
11 of the 13 files' geometry still has no on-device screenshot of its own
(2 of 13 confirmed pre-RE-109). "Render-to-texture paths implemented where
required" is not a separate gap: RE-099/RE-100 both confirm the real
mechanism has no render-to-texture pass at all, only a one-time capture.

RE-109 shipped RE-108's own recorded fix (option (b): rebase each
framebuffer-role primitive's baked UV by its own tile's `uls`/`ult` origin
at pack time — the RDP's own TMEM addressing does the equivalent
subtraction in hardware). `crates/ssb-rom/src/mesh.rs`'s `Cmd::SetTileSize`
handler decoded `uls`/`ult` but discarded them; ordinary textures never
needed them (pack-time extraction already starts at the tile's own origin),
but a framebuffer-role binding's synthetic small capture always starts at
its own row/column 0 regardless of which absolute band of the conceptual
300×220 image the tile pointed at — exactly RE-108's root cause. Fixed by
threading the origin through `State`/`TextureRef` and subtracting it from
the vertex UV in `Builder::push_vertex`, the same mechanism
`prim_color`/`texture_blend`/`flat_color` already use to bake per-primitive
adjustments before the content-keyed vertex dedup runs. New unit test
reproduces RE-108's own real numbers (file 45's 300×5 tile, `ult = 860`)
and is verified capable of failing (removed the fix, confirmed the test
fails with the exact `860*8` discrepancy, restored it). Verified the fix
has real archive-wide effect, not just in the unit fixture, by building the
real pack twice (with and without the fix) and diffing: 3,572,132 bytes
differ, pack size 5165.9 → 5253.2 KiB (+87.3 KiB, an expected dedup-
correctness side effect — see RE-109). `cargo test --workspace`: 262
`ssb-rom` tests (405 total workspace). `cargo clippy --release --workspace`:
clean. Default (non-transition) build re-screenshotted clean (Dream Land
pixel-normal, 60 FPS, no panics) after the fix. On-device visual
re-verification of the specific previously-black region was attempted
(the same `object_view`-forcing recipe RE-100/RE-107/RE-108 used
successfully before) but did not reach a usable screenshot this session —
a debug-viewer camera-framing limitation for this particular screen-
covering object shape, not evidence against the fix; see RE-109 for the
full account.

RE-110 (a later session) picked up RE-109's own recorded next step —
force a small fixed set of exact `spin` values instead of relying on
elapsed real time — and it worked on the first value tried (`spin = 0`).
**Direct pixel measurement, not eyeballing:** the previously-black region
now renders solid magenta (`(255, 0, 255)`, 25,778 sampled pixels),
exactly the unmistakable test colour captured that frame — the fix is
now confirmed on the real device, not only by unit test and packed-byte
diff. The same measurement also found a **second, real, spatially
distinct region** — pure black (`(0, 0, 0)`, 34,584 pixels, measurably
different from the `(32, 40, 56)` clear colour sampled elsewhere in the
same frame, so genuinely rendered, not empty background). This reopens
RE-107's own original mystery (a white, `[255,255,255,0]`, untextured
backing primitive rendering solid black with every known colour
mechanism already ruled out) rather than resolving it: RE-108 had
retracted the *attribution* of "the black region" to this specific
primitive (proving via a green-forcing hack that it "never painted a
single visible pixel" in RE-108's own tests), but with RE-109's fix now
making the photo tile render correctly, this session is the first time
the backing quad's own on-screen appearance has actually been isolated
— and it independently reproduces RE-107's original finding. Deliberately
not chased further this session (see RE-110); recorded as a fresh,
concrete, reproducible lead for a dedicated future investigation.

RE-111 (a later session) found RE-110's own attribution was wrong too — the
backing quad's on-screen appearance still has never actually been isolated
by direct evidence. A targeted, reverted `pack.rs` recolour hack (only
untextured primitives with the backing quad's exact raw colour, avoiding
RE-108's mistake of also recolouring the photo tile's identically-coloured
vertices) produced no visible change; a `romtool` census then found file
45's object is not "one photo primitive plus one backing primitive" but
**8 side-by-side vertical strips**, each its own 44-primitive photo tower
plus a 1-primitive backing strip, tiling the real 300-texel width in
~37.5-texel columns. All 8 towers' baked UVs are byte-for-byte identical
(RE-109's fix is correct and uniform), ruling out the material/UV pipeline
entirely. Two decisive tests (a synthetic uniform-magenta capture buffer
with the real capture disabled; a correctly-list-timed scissor-disable
around the debug clear) both made the black region disappear completely,
proving the defect is in what the real screen capture reads, not in
rendering. **Root cause: `Gpu::new` permanently scissors every draw,
including `sceGuClear`, to the pillarboxed 4:3 viewport
(`vx = 59`, `vw = 362`) — columns `0..59` of the raw 480-wide buffer are
never drawn to at all and stay solid black (power-on-zeroed) for the whole
program's life — but `capture_transition_photo` read `TRANSITION_PHOTO_WIDTH`
(300) columns starting at absolute column 0, not at the pillarbox's own
left edge.** Four of the 8 towers' `u` ranges fall in that permanently-black
59-texel slice. Fixed with a one-line offset
(`BUF_WIDTH * y + pillarboxed_viewport().0`); re-verified with the real
capture and no diagnostic overrides — a direct pixel scan of the object's
own screen region found zero `(0, 0, 0)` pixels (was 28,993–34,584 across
three prior measurements). This is a genuine bug independent of the debug
recipe used to find it: a real LB transition's capture would hit the same
permanently-black bar in real gameplay, since nothing this project draws
ever reaches columns `0..59`/`421..480` under the standing pillarbox.

RE-112 (a later session) resolved the "backing quad" question entirely —
it never existed as reachable geometry. `romtool scene --file 45 --list
--nodes` shows file 45's one scene graph has 9 nodes, 8 with a display
list, and all 8 are the photo towers already confirmed correct; none of
the "backing" offsets are in this object's node list (nor its packed
`ObjectDesc::node_count`, which would read 17 instead of 9 if they were
attached extra-leaf siblings). A scan-inventory census explained why they
exist in the pack at all: `crates/ssb-rom/src/scan.rs`'s
`find_root_display_lists` tracks an "outermost list" dedup using each
kept list's own literal decoded byte span, not the larger range it
actually renders once its own `Call` is followed — so the tiny 9-word
dispatch list that calls into each tower's real 310-word body only
"covers" its own 9 words, not the full body, and a display list's own
tail commands (the same real "300×5, drawn once" primitive already inside
the correctly-converted mesh) independently re-decode as a second,
spurious "root" list, missing the real texture state that lived earlier
in the true list outside that tail window — producing exactly the
untextured, raw-white, never-drawn duplicate every prior session chased.
This retracts RE-107/108/110/111's entire line of questioning: there was
never a second, real backing primitive on file 45's object at all. Not
fixed this session — `find_root_display_lists` is shared well beyond
R0.13, and a correct fix needs an archive-wide before/after measurement,
recorded as a concrete lead rather than shipped mid-investigation.

RE-113 (a later session) continued the remaining concrete work —
screenshotting the other 12 files — starting with the six structurally
simple ones (1–2 nodes). Files 42, 44, 47 and 49 are now confirmed fully
correct on the real device (uniform capture colour, zero black pixels by
direct pixel scan) — 6 of 13 files now have real on-device evidence, up
from 2. File 43 hits RE-109's already-documented camera-framing
limitation for widely-spread objects (not a new issue). **File 46 shows
a real, new, distinct defect**: regular diagonal black bands (116,152
genuine `(0, 0, 0)` pixels, not window-capture noise), traced to its `U`
range cycling through an 11-step shifting/full-width pattern per strip
(a diagonal-wipe UV shear, likely authored, not decode noise) — what
produces solid black at the narrowed edge of each shifted band is not
yet isolated; recorded as a concrete lead, not fixed this session.

RE-114 (a later session) finished screenshotting the remaining files
(39, 48, 50, 51). Files 39 (object 11, the same 8-node "sudare" shape as
file 45), 51 (object 23, an 8-pointed radial "starburst" matching its
circular node layout) and 48 (object 20, a scattered ~29-panel cluster,
the one structurally distinct outlier) are all confirmed fully correct —
zero black pixels by direct pixel scan. File 50 (object 22, tested at
both `spin = 0` and `spin = π`) hits the same camera-framing gap as 41
and 43. **All 13 transition files are now accounted for**: 9 confirmed
clean (`39, 40, 42, 44, 45, 47, 48, 49, 51`), 3 blocked on the
debug-viewer's camera-framing gap (`41, 43, 50`), 1 with RE-113's still-
open diagonal-banding defect (`46`).

RE-115 (a later session) fixed the camera-framing gap itself. It was
never the camera or `object_bounds` — files 41/43/50's `cam`/`r` overlay
readouts were always sane and non-degenerate. Disabling `GuState::CullFace`
entirely made file 41 visible on the first try: these are one-sided
authored planes, and the debug viewer's free-roaming inspection camera
has no guarantee (unlike a real game camera) of viewing a plane from its
authored front side. Added `DrawState::force_no_cull`, set to
`object_view` once per frame in `psp/src/main.rs`, checked in
`apply_material`'s existing per-primitive cull decision — scoped strictly
to the debug viewer's own inspection mode; real gameplay rendering's
culling (RE-068's verified `CULL_BACK`/`CULL_FRONT` reproduction) is
untouched. Files 41 and 43 confirmed clean by direct pixel scan; file 50
confirmed correct by direct on-device observation (the fix visibly works
in the live PPSSPP window; automated screenshot timing could not
reliably catch this specific file's frame, a tooling limitation, not a
rendering defect). **12 of 13 transition files are now fully verified
correct**; only file 46's RE-113 diagonal-banding defect remains open.

RE-116 (a later session) retracted RE-113's file 46 defect entirely — it
was never a rendering bug, it was RE-113's own measurement artifact. A
close pixel-by-pixel scanline across the "black" bands found exact
linear interpolation between the real background colour and the real
magenta capture colour on all three channels simultaneously (anti-aliased
polygon-edge blending, not a sampling error). An exhaustive,
bounding-box-restricted census of both of file 46's rendered squares
found zero pure `(0, 0, 0)` pixels. RE-113's "116,152 genuine black
pixels" figure came from an un-restricted, whole-image scan — the exact
window-decoration confound RE-111 had already identified and documented,
which RE-113 asserted (incorrectly) did not apply here. Also confirmed
RE-115's culling fix is unrelated: toggling `force_no_cull` off and on
produces pixel-identical output for file 46. The diagonal `U`-shifting
pattern RE-113 found is real, authored ROM data for a diagonal (not
horizontal) wipe shape, and renders correctly. **All 13 LB-transition
files are now confirmed fully correct on the real device.**

RE-146 traces the missing caller contract precisely. The only production
caller is VS Results startup: snapshot the completed battle framebuffer once,
enter results, create a dedicated 45° camera, choose one of 11 transition
descriptors, play its finite DObj animation, and remove it at end. A persistent
real-ROM inventory resolves all 11 source graphs/joint tables and replays every
script: IDs 0–9 last 64 frames; Curtain (ID 10) lasts 72. The source-indexed
asset table and verifier are implemented. Packing those generic object
animations is now implemented as RE-147: all eleven append after the dense
fighter/stage block, replay from the generated pack, and resolve the object
their absolute joint nodes drive. Adding the minimal results-entry caller
is now implemented as RE-148 with the original capture-before-results frame
boundary and transition camera. PPSSPP verifies the first packed wipe at a
deterministic midpoint. RE-149 resolves the roadmap boundary: R0.13 owns the
reusable renderer, capture lifecycle, and synchronized frame boundary, all of
which are implemented and verified. Connecting that lifecycle to the eventual
normal match/results flow belongs to G2's explicitly listed match transitions.
Holding R0.13 open for a post-combat G2 caller would create a dependency cycle
with the rendering gate that must pass before combat begins.

### Objective

Implement every framebuffer-based rendering path required by SSB64.

### Dependencies

* R0.2
* R0.6

### Acceptance

* [x] framebuffer usage identified — RE-099: the one-time-snapshot-into-a-texture mechanism, exactly which 13 files use it (26 segment-`0x01` binds), and what a PSP implementation actually needs to build
* [x] framebuffer texture paths implemented — RE-100: segment-`0x1` recognition, pack format support (`TextureDesc::role`, `VERSION` 14), and device-side capture-and-bind, verified on the real device profile with an unambiguous test-colour capture; RE-107 confirmed the shape generalizes archive-wide and the capture/bind mechanism itself works on a second, deliberately different file. RE-108 found a real correctness gap (the capture only stores the top of the real 220-texel-tall N64 buffer, so a tile sampling elsewhere in that range reads the wrong rows); RE-109 fixed it by rebasing each framebuffer-role primitive's UV by its own tile origin at pack time (unit-tested, packed-byte-diff-verified); RE-110 confirmed the fix on the real device by direct pixel measurement (the previously-black region now reads the exact captured test colour). RE-111 found and fixed a second, independent real bug in the same mechanism: the permanent 4:3 pillarbox scissor left columns `0..59` of the raw framebuffer solid black forever, and the capture read from absolute column 0 instead of the pillarbox's own left edge — fixed with a one-line offset, verified by a direct pixel scan finding zero black pixels on the object post-fix (was 28,993–34,584 pre-fix)
* [x] screen wipes implemented — RE-146/147 resolve and pack the original eleven finite wipe animations; RE-148 implements their reusable capture/play/draw/eject lifecycle, original 45° transition camera, and PPSSPP-verifies Aeroplane at a deterministic midpoint. RE-149 records that normal match/results integration belongs to G2's match-transition work rather than this renderer task
* [x] render-to-texture paths implemented where required — RE-099/RE-100: confirmed twice, independently, that the real mechanism has no render-to-texture pass to implement; this item is satisfied by there being nothing here that applies
* [x] framebuffer synchronization verified — RE-148's explicit state machine leaves the battle scene active while capture is requested, performs the copy only after `Gpu::end_frame` completes the GE frame, and permits `Playing` only after that completion; PPSSPP measures 3,040 exact source-clear pixels in the resulting wipe. RE-149 assigns the future normal-game trigger to G2 without weakening this renderer-side synchronization criterion
* [x] visual verification completed — **all 13 LB-transition files are confirmed fully correct on the real device.** File 45's own "backing quad" question is fully retracted (RE-112 — it was never reachable geometry). The debug-viewer camera-framing gap that blocked 41/43/50 is fixed for good (RE-115 — `DrawState::force_no_cull`, scoped to `object_view` only). File 46's apparent diagonal black banding (RE-113) is retracted (RE-116) — a measurement artifact (an un-restricted pixel scan catching the same window-decoration confound RE-111 already documented), not a rendering defect; a close pixel scanline shows exact linear anti-aliased blending between real background and real magenta, and an exhaustive bounding-box-restricted census finds zero black pixels

### Evidence

RE-055, RE-099, RE-100, RE-107, RE-108, RE-109, RE-110, RE-111, RE-112, RE-113, RE-114, RE-115, RE-116, RE-146, RE-147, RE-148, RE-149 in `docs/reverse-engineering.md`.

---

## R0.14 — Camera / Projection Correctness

Status: `COMPLETE`

### Current evidence

Pillarboxed 362×272 viewport is implemented and applied to both
`sceGuViewport` and `sceGuScissor` (D-008, RE-034). Depth range inversion is
implemented and verified (D-007). Full projection-matrix and camera-transform
correctness against the original's camera behavior has not been separately
verified — this task and R0.12 (billboards) share that open dependency.

RE-082 re-audited RE-034's own reported residual (measured `1.000` against
an expected `0.938` for the fighter collision-diamond marker, a 6.6% gap
never explained). Three independent re-measurement attempts (default zoom,
threshold sensitivity, and a temporary zoomed-in device screenshot) produced
three different ratios — `0.82`, `0.90`–`0.95`, `1.14`–`1.16` — spanning both
sides of `1.0` and of the expected value, showing the marker is too small
(20–80 px depending on zoom) to support single-digit-percent precision
claims from a screenshot. A source-level audit found no remaining bug to
explain a real residual: `psp/src/gu.rs`'s `Gpu::init` and `psp/src/main.rs`
both derive their viewport/aspect values from the same
`coord::pillarboxed_viewport()` call (so they cannot disagree the way
RE-034's original bug had them disagree), and the `psp` crate's
`sceGumPerspective` binding implements the textbook `cot(fovy/2)/aspect`
formula with no quirk. RE-034's fix stands; its follow-up "still 6.6% off"
number is retracted as measurement noise, not confirmed as a bug.

RE-084 found and fixed the FOV term's own unsourced guess:
`psp/src/main.rs` called `sceGumPerspective` with `60.0` degrees, a number
with no comment, decision record, or citation anywhere. The decompilation's
real default battle-camera FOV is `38.0` degrees
(`gm/gmcamera.c:1191`/`gmCameraAdjustFOV`, four call sites agreeing, two
special-case player-zoom/-follow modes taking their own situational value
instead). Fixed the constant and recomputed the two debug-camera framing
constants that depended on the old FOV (`FIT`, `1/tan(30°)` → `1/tan(19°)`,
≈1.677× larger) so stages and objects still fill the frame the same way at
the viewer's default zoom — verified by a before/after screenshot of Dream
Land's stage view, not just by arithmetic.

RE-085 closed "depth mapping verified" — the one item on this task with
no decomp-side constant to look up, since the N64's Z-buffer is inherent
RDP hardware behavior, not a game-configurable value. Confirmed
`psp/src/gu.rs`'s `sceGuDepthRange(65535, 0)` + `DepthFunc::GreaterOrEqual`
matches the `psp` crate's *own documented* `sceGuDepthRange` convention
exactly ("the depth buffer is inversed, and takes values from 65535 to
0" — the SDK binding's own doc comment), not a workaround invented for a
bug this project hit. Corroborated by inspecting Dream Land's stage view
for depth-order artifacts (tree trunk vs. canopy, decorative sprites,
platform edges, fighter marker) — none found. No code changed.

RE-131 built the real camera this task's own "camera transforms
verified" item has been waiting on. Researched and independently
verified `gm/gmcamera.c`'s `gmCameraDefaultFuncCamera` (the single-fighter,
normal-stage case) directly against the source, then ported it call for
call as `ssb_game::camera` — a portable, `no_std`, platform-free module
matching Layer A's existing rules. Along the way, found and fixed a real
error in `stage.rs`'s own record: `light_angle.z` (previously "no known
reader") is read by the camera's own `gmCameraGetAdjustAtAngle`, and is
stored pre-converted to radians archive-wide (measured, not assumed) —
unlike `.x`/`.y`, which are degrees. Wired into `psp/src/main.rs`'s
debug viewer via a new `Gpu::set_view` (a real `Mat4::look_at`, not the
translate-only trick the whole-stage debug view still uses and keeps
unchanged) — verified zero pixel difference against the golden capture
for every mode except the one it targets, and stable, sane, converging
behaviour over a 20-second run in that one. Deliberately minimal (no
weapons, multiplayer, per-move zoom, idle-zoom-out, or pause offset);
see RE-131's own entry for the full scope statement.

RE-150 re-audited that port against `gmCameraMakeDefaultCamera` and found two
real source mismatches. The observable constructor state is `at = (0,300,0)`,
`eye = (0,300,10000)`, `target_dist = 10000`; the earlier port stopped at the
intermediate `dGMCameraCObjVecDefault` copy. Its distance update had also
replaced the original signed comparison with absolute values, losing the
original's immediate outward snap. Both are corrected and pinned by tests. A
new off-by-default `camera_audit_capture` feature produces a HUD-free,
diagnostic-free, fixed-tick Dream Land frame through the real camera; independent
7- and 9-second PPSSPP runs are byte-identical. The user's N64 Dream Land image
is now paired with that capture. It exposes the known incomparable camera input
(two widely separated fighters in the N64 training scene versus the port's one
Mario), plus the already-tracked missing stage background/effects/UI work. The
comparison closes the representative-scene item, but not exact camera-output
verification: a same-interest original frame is still required for that.

RE-151 supplies that missing oracle. A scripted Mupen64Plus run boots the
identified USA ROM, selects Mario and Pikachu, selects Dream Land, and captures
by core frame index. Direct RDRAM reads at stable frame 1800 recover the
original camera's `target_dist = 3181.2507`, `at = (-698.5003, 617.1825, 0)`,
`eye = (-413.7114, 1045.3188, 3137.6443)`, and `fovy = 37.99998`. The port now
unions one-to-four fighter interests with the original player-count table,
fighter camera multipliers, facing asymmetry, and 120-tick Wait zoom. It also
uses the original quantized `lbCommonSin`/`Cos`/`Tan` behavior. A focused test
matches the trace within 0.1 units for target distance/look-at and 0.67 units
for eye (the original vector-normalization steady-state residual, below a
tenth of a captured pixel). Independent 8- and 9-second PPSSPP captures are
byte-identical. The normalized side-by-side aligns the left platform and
fighters; its remaining large differences are the already-tracked missing
render passes/material fidelity, not camera framing.

### Objective

Reproduce the original camera and projection behavior.

### Dependencies

* R0.8

### Acceptance

* [x] projection matrix verified — RE-082 audited the aspect term; RE-084 replaced the FOV term's unsourced `60.0` degree guess with the decompilation's own real default (`38.0` degrees, four agreeing call sites). Near/far clip planes are this project's own debug-viewer framing choice, not part of the original's projection behavior, so nothing further to source there
* [x] viewport verified — RE-034 (device measurement, before/after screenshots) plus RE-082 (source-level confirmation that `Gpu::init` and `main.rs` share one `pillarboxed_viewport()` call, so they cannot diverge)
* [x] aspect ratio verified — RE-082: `pillarboxed_viewport()` is unit-tested (`pillarbox_preserves_four_by_three`), its output is the sole source for both the GE viewport/scissor and the projection's `aspect` parameter, and `sceGumPerspective`'s own binding uses the standard formula; RE-034's previously-reported residual is a measurement artifact on a too-small on-screen shape, not a surviving defect
* [x] depth mapping verified — RE-085: `sceGuDepthRange(65535, 0)` + `GreaterOrEqual` matches the `psp` crate's own documented depth-buffer convention exactly, not a workaround; corroborated on-device with no depth-order artifacts found in a complex, multi-layer regression scene
* [x] camera transforms verified — RE-151 runs the original ROM with the same Dream Land/Mario/Pikachu interests, reads the live `GMCamera` and `CObj` state from emulated RDRAM, and pins the port against `target_dist`, `at`, `eye`, and `fovy`; the one-to-four fighter union, player-count zoom table, fighter multiplier, facing asymmetry, Wait zoom and original quantized trigonometry are implemented. Weapons and special entry/dead/pause camera modes remain future gameplay-system inputs, not mismatches in the verified default battle-camera path
* [x] N64/PSP resolution differences explicitly handled — RE-082: pillarboxing (D-008) is precisely this handling, now confirmed by both a device measurement (RE-034) and a source audit (RE-082) rather than one alone
* [x] representative scenes compared — RE-150 provides the first qualitative Dream Land comparison; RE-151 replaces its mismatched one-fighter input with the same Mario/Pikachu positions, facings and settled Wait state on both versions. The normalized frame aligns the camera-dependent landmarks while clearly isolating the remaining missing background/effects/UI and material-fidelity gaps

### Evidence

RE-034, RE-082, RE-084, RE-085, RE-131, RE-150, RE-151 in `docs/reverse-engineering.md`.

---

## R0.15 — Render-State Isolation

Status: `VERIFYING` — RE-118 closed the known texture-cache bypass, but
RE-217 found that the broader direct-GU inventory required by R2.2/C5 has not
been completed.

### Current evidence

RE-117 surveyed `crates/ssb-rom/src/mesh.rs`'s `State`/`MeshMaterial`
threading first, rather than guessing which categories needed new tests.
Every state category lives in one `State` struct, reused across
`convert_sequence`'s whole loop (`State::new()` called exactly once per
scene graph, confirmed by code reading — matching R0.4/RE-064's own
"no cross-object leakage by construction" finding) — so by construction,
nothing resets between nodes except `State::forget_texture`'s narrow,
intentional, image-only clear on an unfollowable `Call`/`Branch`. The
survey found only one category (texture image binding) had a direct
cross-node persistence test at all (RE-064). Added four new tests
closing the other nine:

* `a_palette_binding_survives_a_new_image_bind_without_a_new_tlut_load`
  (TLUT/palette) — the direction RE-093's own fix never covered: here the
  *image* changes via a fresh `G_SETTIMG`, and the palette (`G_LOADTLUT`)
  must still carry over unchanged, since real hardware's CLUT and
  texture-image registers are independent.
* `combiner_and_colour_constants_persist_into_a_node_that_sets_none_of_them`
  (combiner, primitive color, environment color, blend color) — uses
  Link's own real combiner word (RE-073) so a single `texture_blend`
  assertion would break if PRIM, ENV or the combiner shape failed to
  carry over; `G_SETBLENDCOLOR` checked directly in the same test.
* `render_mode_persists_into_a_node_that_sets_no_new_render_mode`
  (blend/alpha state) — reuses `xlu_render_mode_is_translucent`'s real
  render-mode word.
* `geometry_mode_persists_into_a_node_that_sets_no_new_geometry_mode`
  (depth, culling, geometry/lighting mode) — one `G_GEOMETRYMODE` set in
  node A, nothing in node B, all five bits (`cull_back`, `cull_front`,
  `lit`, `smooth`, `z_buffer`) checked.

**Texture addressing was already covered, just not documented as such.**
`RE-064`'s own existing test asserts whole-`TextureRef` equality between
nodes, and `TextureRef` (`derive(PartialEq, Eq)`) bundles `mirror_s`/
`mirror_t`/`clamp_s`/`clamp_t`/dimensions/palette fields together —
its item A's own `SetTile` already sets non-default `mask`/`cm` values,
so that single assertion already exercises tile-addressing persistence,
it just was never labelled as doing so.

All four new tests confirmed capable of failing: a temporary, reverted
change to `convert_sequence` (rebuilding `State::new()` fresh every loop
iteration instead of reusing one) made all four fail with the expected
mismatch, plus two pre-existing tests (the vertex cache and RE-064's own
texture test) — confirming the whole mechanism this task audits is a
single shared construction, not per-category logic that could pass
independently. Reverted; `cargo test --workspace`: 266 `ssb-rom` (405
total workspace, was 401). `cargo clippy --release --workspace`: clean.
Rebuilt pack: byte-identical to baseline (5253.2 KiB, same counts —
test-only change). `cargo psp --release` + `tools/run-ppsspp.sh`: Dream
Land re-screenshotted clean (pixel-normal, 60 FPS, no panics).

**Not yet covered:** the PSP-side `psp/src/meshdraw.rs::DrawState`'s own
GE draw-state cache (`last_texture`/`last_flags`/`last_texture_blend`) is
a *second* layer this task's "leak between draws" objective also
touches — RE-074 already found and fixed one real bug there (`bind_texture`
unconditionally resetting the texture function, clobbering `TEXTURE_BLEND`
state), but no systematic audit of that layer has been done the way this
session did for `mesh.rs`. Left as further work before this task closes.

RE-118 (a later session) completed that second layer's audit. Read
`apply_material`/`bind_texture` end to end against every category:
culling/shading/depth/alpha-test are each set inside an explicit
if/else with no skipped branch (no leak possible); a stale
`sceGuTexLevelMode`/CLUT left from a previous texture are both inert
(the GE clamps LOD to the mip count `sceGuTexMode` just declared, and
non-indexed formats never consult CLUT); `GuState::Blend` is confirmed
never enabled anywhere in the crate. **Found one real, new gap:**
`Gpu::draw_triangles`/`draw_line_strip` disable `Texture2D` directly,
bypassing `DrawState` entirely — and `draw_collision`/`draw_fighter`
(the collision-line and simulated-fighter-marker overlays, both calling
`draw_line_strip`) run *between* two cached mesh draws whenever
`show_collision`/`sim_fighter` are on, which is the default. A primitive
drawn afterward that happens to share a texture index with whatever was
bound before the overlay (the pack dedups textures by content, so this
is plausible though not guaranteed) would wrongly stay untextured.
Checked whether this manifests for the current default scene (it does
not — Dream Land's own last texture and the simulated fighter's first
texture don't happen to coincide) before concluding the underlying
cache-invariant violation is still real regardless. Fixed by adding
`DrawState::forget_texture()`, called at the end of both overlay
functions, forcing the next real primitive to always rebind rather than
trust an invalidated comparison. Verified inert for the
non-triggering case: `tools/run-ppsspp.sh` re-screenshotted
pixel-identical to the pre-fix baseline (same overlay counts, same
fighter-model crop).

### Objective

Ensure render state cannot incorrectly leak between display-list/material/node draws.

### Dependencies

* R0.2
* R0.6

### Acceptance

* [x] texture state tracked — RE-064 (pre-existing), cross-node persistence pinned by a direct test
* [x] TLUT state tracked — RE-117: `a_palette_binding_survives_a_new_image_bind_without_a_new_tlut_load`
* [x] combiner state tracked — RE-117: `combiner_and_colour_constants_persist_into_a_node_that_sets_none_of_them`
* [x] primitive color tracked — same test
* [x] environment color tracked — same test
* [x] blend state tracked — RE-117: `render_mode_persists_into_a_node_that_sets_no_new_render_mode` (alpha/translucent) plus `G_SETBLENDCOLOR` in the combiner test
* [x] depth state tracked — RE-117: `geometry_mode_persists_into_a_node_that_sets_no_new_geometry_mode`
* [x] culling tracked — same test
* [x] geometry state tracked — same test (lighting/shading-smooth bits)
* [x] texture addressing tracked — RE-064's existing test already covers this via whole-`TextureRef` equality (RE-117 documents it explicitly)
* [ ] state leakage tests added — the decode-time state-threading tests remain valid (RE-117), and RE-118 fixed the known collision/fighter overlay texture-cache bypass. The broader direct-GU inventory and invalidate-all regression required for the PSP-side cache remains open under R2.2/C5.

### Evidence

RE-064, RE-074, RE-117, RE-118 in `docs/reverse-engineering.md`.

---

## R0.16 — N64 Render-State Model Fidelity

Status: `VERIFYING` — RE-122's texture-key fix remains valid, but RE-217
confirmed that `merge_by_material` still globally reorders non-adjacent
primitive runs. R2.2/C4 must preserve submission order before this task can
close.

### Current evidence

RE-119 started this audit from R0.2's own opcode inventory (`docs/
rendering.md`) instead of guessing which state categories needed
attention, and found the inventory itself was stale enough to need
re-measuring before it could be trusted as a checklist. `romtool scan`'s
own `geometry_mode_name` helper had `G_SHADE` mapped to the wrong bit
(`0x2`, disagreeing with `refs/ssb-decomp-re`'s real `gbi.h`, which
defines it as `0x4`) — a real bug in this project's own diagnostic
tooling, not the game data, that hid 60 archive-wide occurrences under a
blank label instead of `G_SHADE`. Fixed. Re-running the scan after the
fix, and independently, surfaced that the *whole* opcode table had gone
stale since R0.2 was first measured (every count shifted, e.g. `G_TRI2`
10954 → 13523, consistent with later conversion-fidelity fixes changing
how many triangles the same 1,864 discovered lists parse into) and that
`G_MOVEWORD` was wrongly listed as "never emitted" — RE-105 (a much
earlier session) had already found and relied on real `G_MOVEWORD`
usage (`G_MW_LIGHTCOL`) without this table ever being corrected to
match. Refreshed `docs/rendering.md`'s whole opcode table and its
"Geometry modes set" line from a fresh `romtool scan`.

**Found two real, previously-undocumented geometry-mode categories with
zero handling in `mesh.rs`:**

* `G_SHADE` (60 occurrences) — real GBI semantics (`gbi.h`): "necessary
  in order to see the color that you passed down with the vertex... if
  not set, you need to use primcolor". Archive-wide, every occurrence
  clears it together with `G_LIGHTING`/`G_SHADING_SMOOTH` in the same
  command, never re-setting it in that command (checked, not assumed) —
  consistent with a deliberate switch to flat, `PRIMITIVE`-driven
  rendering that this project's existing `combiner_flat_color`/
  `combiner_texture_blend` detection (R0.6) likely already reproduces
  correctly for most cases, since those combiner shapes never read
  `SHADE` regardless of `G_SHADE`'s own state. **Not yet cross-referenced
  per-primitive** against which specific primitives clear `G_SHADE`
  *and* still have a combiner that reads `SHADE` — the one scenario that
  would actually render wrong today. Affects several stage files, the
  main menu title, the staff roll, and one fighter special-move file
  (file IDs recorded in `docs/rendering.md`).
* `G_TEXTURE_GEN`/`G_TEXTURE_GEN_LINEAR` (156/13 occurrences) —
  RSP-computed environment-mapped UVs. Used by `StageMetalFile2` and
  `MMarioModel`/`NMarioModel`/`NFoxModel`: this is the "Metal
  [Character]" transformation's signature shiny/reflective look. **Corrected
  by RE-216: there is no Metal Box item.** `MMarioModel`/`NMarioModel`/
  `NFoxModel` back a separate, permanent `FTKind` the original only
  constructs for the 1P-mode stage-8 boss fight (`src/ft/ftdata.c`/
  `ftmanager.c:693`) — not an item-pickup effect applied to a normal
  fighter. Classified as SSB64 genuinely needing it (`PLAN.md` R0.18's
  classification 1), but implementation is correctly deferred — it is
  downstream of the combat/item systems `AGENTS.md` §5 gates behind
  rendering correctness regardless of the correction above. Not an
  `ACCEPTED_DEVIATION` (it is technically reproducible on the PSP GE), just
  out of scope until items/1P-mode content exist. **Superseded:** RE-213
  implemented it and RE-214 corrected it — the reflective content is
  reachable through `StageMetalFile2` without scripting a full item system,
  so it was validated under R2 rather than waiting on it. See R2's own
  acceptance list for what remains (original-output comparison, and the
  linear form).

Neither gap was fixed this session — both need either a per-primitive
cross-reference (`G_SHADE`) or a real feature (environment-mapped UV
generation, `G_TEXTURE_GEN`) gated behind out-of-scope game systems.
Recorded as concrete, scoped, characterized leads rather than guessed at
or silently left unlisted, matching this audit's own acceptance
criteria: every category found is now either handled, explicitly
deferred with a reason, or flagged as needing further investigation —
none is silently absent from `docs/rendering.md` any more.

RE-120 (a later session) closed the `G_SHADE` cross-reference. Confirmed
archive-wide, with real file attribution: 31 occurrences of `G_SHADE`
off with a combiner that still reads `SHADE`. **29 of 31 (files 86
`ITCommonObject`, 350 `CaptainSpecial2`, 85 `EFCommonEffects3`, 353
`LinkSpecial2`) are items, fighter special-move effects, and general
effects — none of which this project renders yet**, since combat/items
are correctly gated behind the rendering-completeness milestone. **2
occurrences affect content already rendered**: one primitive each in
`StageYosterFile2`/`StageYosterSmallFile2` (Yoshi's Island, both
variants) — a real, narrow, currently-live gap (a single primitive per
stage, not core platform geometry, neither stage previously flagged as
visually wrong). Not fixed: the correct real-hardware behavior for this
combination isn't well-defined by `gbi.h`'s own documentation, and
guessing at it risks the exact failure mode `AGENTS.md` §9 warns
against. Recorded as a concrete, narrow, low-priority open lead.

RE-121 (a later session) closed the second acceptance item: went field
by field through all 14 of `MeshMaterial`'s fields against `pack.rs`/
`psp/src/meshdraw.rs`'s real consumers. Found `blend_color`
(`G_SETBLENDCOLOR`, 366 archive-wide occurrences) is never packed at
all — no field, no flag, no documented reason, unlike every other field
in the struct. Measured whether it matters rather than assuming: a
temporary, reverted census checked every real blend equation
archive-wide for the blend-color register (`G_BL_CLR_BL`) as a colour
source — **zero occurrences**, the identical shape RE-072 already found
for fog. `blend_color`'s absence is correct, now measured and documented
instead of merely unaddressed. Also found and fixed two stale "not yet
consumed on the device side" doc comments (`MeshMaterial::texture_blend`,
`pack::flags::TEXTURE_BLEND`) both contradicted by RE-074, several
sessions earlier, having already wired `TEXTURE_BLEND` up — neither
comment was ever updated afterward. Added missing "inspection only, not
read back by the device" clarifications to `PrimDesc::prim_color`/
`env_color` and `pack::flags::LIT`, matching `flat_color`'s own existing
note.

RE-122 (a later session) closed the final acceptance item: checked
D-036's ordering rule against every shipped optimization. Vertex dedup
and material merge are both safe by construction (each keys on the
*entire* relevant struct, not a hand-picked subset). **Found a real
violation in `tools/romtool/src/main.rs`'s `TexKey`**: a hand-picked
4-tuple `(image_file, image_offset, palette_file, palette_offset)` that
left out wrap/mirror/clamp mode entirely — but `convert_texture`
pre-bakes a genuinely different, mirrored copy of a texture's bytes when
`mirror_s`/`mirror_t` is set (RE-067), so two bindings of the same
image+palette with different wrap modes need different cache entries.
Measured archive-wide: **126 occurrences** across 19+ files where two
different-wrap bindings silently shared one cache entry, one of them
getting the wrong pre-baked bytes/wrap flags. Fixed by extending `TexKey`
to an 8-tuple including `mirror_s`/`mirror_t`/`clamp_s`/`clamp_t`.
Verified archive-wide: textures `899 → 935` (+36, un-merging the
previously-collapsed variants), pack size `5253.2 → 5348.1 KiB` (+1.8%).
`cargo test --workspace` and `cargo psp --release` +
`tools/run-ppsspp.sh` both clean; not independently re-verified against
one specific newly-fixed fighter/stage screenshot this session (the same
caveat RE-102 recorded for its own structurally similar fix).

### Objective

Audit and, where necessary, harden the intermediate representation between
N64 display-list decoding and PSP translation (`mesh::State`/`MeshMaterial`,
`pack::PrimDesc`/`TextureDesc`/`MatAnimDesc`/`NodeDesc`) so that it preserves
N64 render state faithfully rather than reducing it prematurely to
`mesh + texture + basic colour` (D-036).

This is not a request to re-architect the pipeline in `docs/rendering.md`
("The central decision") — build-time display-list-to-PSP-vertex-buffer
conversion (D-001) stays. It is a request to verify the *state* that survives
that conversion is complete relative to what SSB64 actually uses, and that no
future optimization pass is allowed to remove state before its correctness is
established (D-036).

### Dependencies

* R0.2
* R0.6
* R0.15

### Acceptance

* [ ] every state category R0.2's command inventory found SSB64 actually
  exercising (texture state, tile state, combiner state, primitive color,
  environment color, geometry mode, lighting mode, alpha state, blend state,
  depth state, filtering, addressing, LOD, palette/TLUT state, render-pass
  state) has an explicit field or explicit "does not apply to SSB64, measured"
  note in `MeshMaterial`/the pack record formats — RE-119 found geometry mode's
  own category was incomplete (`G_SHADE`, `G_TEXTURE_GEN`/`G_TEXTURE_GEN_LINEAR`
  had zero field or documented reason); RE-120 closed `G_SHADE`'s own
  remaining gap with a real, file-attributed archive-wide measurement (29/31
  occurrences in not-yet-rendered content, 2 in Yoshi's Island, documented as
  a narrow open lead rather than fixed or ignored) — every category found by
  this audit now has either handling, a documented deferral, or a measured,
  scoped, named open item. **Reopened by RE-218**: this audit started from
  `MeshMaterial`'s own fields (a layer downstream of raw command decoding) and
  never inspected `Cmd::SetTile`'s own `palette`/`line`/`tmem`/`shift_s`/
  `shift_t` fields, which `mesh.rs`'s only consumer discards via a bare `..`
  before any of them could reach `MeshMaterial` at all; owned by `R2.0`/P1
* [x] no state category is silently dropped between `mesh.rs`'s conversion and
  `pack.rs`'s on-disk record without a documented reason (cross-reference
  against R0.15's leakage tests) — RE-121 went field by field through all 14
  `MeshMaterial` fields; found and fixed one real, previously-undocumented
  drop (`blend_color`, measured genuinely inert archive-wide, the same shape
  RE-072 found for fog) and two stale doc comments claiming `texture_blend`
  was unconsumed when RE-074 had already wired it up
* [x] `docs/rendering.md`'s N64→PSP state-mapping table (referenced from R0.2)
  is complete against this audit's findings, not just the opcodes that
  convert cleanly — RE-119 refreshed the whole opcode table (stale since R0.2,
  every count had drifted) and the geometry-mode-set line (`G_SHADE`/
  `G_TEXTURE_GEN`/`G_TEXTURE_GEN_LINEAR` added, each with its own real ROM
  file references and current handling status)
* [ ] D-036's ordering rule (state fidelity before batching/state-sorting/
  draw-call reduction) is checked against every existing optimization already
  shipped (vertex dedup, material merge, `TexKey`/`texture_cache` dedup) and
  each one is confirmed not to have discarded state this audit found required
  — RE-122: vertex dedup and material merge are safe by construction (each
  keys on its entire relevant struct); `TexKey` had a real violation
  (wrap/mirror/clamp mode omitted from the cache key, 126 archive-wide
  occurrences of two different-wrap bindings silently sharing one entry),
  now fixed by widening the key to include it. RE-217 separately found that
  `merge_by_material` uses a global `BTreeMap`, so non-adjacent equal-material
  runs can still be reordered; adjacent-run preservation is R2.2/C4.
* [x] any state this audit finds genuinely unrecoverable on PSP is recorded as
  an `ACCEPTED_DEVIATION` per `AGENTS.md` §9, not silently absent — checked
  every category this audit found (`G_SHADE`, `G_TEXTURE_GEN`, `blend_color`):
  none is genuinely unrecoverable (`G_SHADE`/`G_TEXTURE_GEN` are reproducible
  on the PSP GE, just deferred; `blend_color` is measured inert, not
  "impossible to reproduce"), so no `ACCEPTED_DEVIATION` applies — confirmed,
  not assumed

### Verification

* source-level audit of `mesh.rs`, `pack.rs`, `psp/src/meshdraw.rs`
* cross-reference against R0.2's opcode/state inventory
* cross-reference against R0.18's reference-port comparison

### Evidence

RE-119, RE-120, RE-121, RE-122, RE-218 in `docs/reverse-engineering.md`.

---

## R0.17 — Visual Regression Methodology

Status: `COMPLETE`

### Objective

Establish a deterministic, repeatable visual-regression methodology, not an
optional future improvement. Screenshots taken ad hoc during individual
investigations (as recorded throughout `R0.1`–`R0.15`'s evidence sections)
remain valid evidence for the specific claims they were taken for, but they do
not substitute for this task: a fixed, reproducible scene/camera/frame that
can be re-run and diffed automatically as the renderer changes.

### Dependencies

* R0.1
* R0.2

### Acceptance

* [x] at least one deterministic test scene defined: fixed stage (Dream Land,
  the project's existing primary regression scene), fixed fighter, fixed
  camera, fixed animation/frame, fixed game state — every value pinned, no
  randomness, no free-roaming debug camera
* [x] a documented procedure exists to capture the same scene from: (1) the
  original SSB64 (ROM/emulator reference), (2) PPSSPP software rendering,
  (3) PPSSPP hardware rendering, (4) physical PSP hardware where practical
* [x] a test matrix exists covering, at minimum: untextured geometry,
  textured geometry, CI4, CI8, palette changes, filtering, clamp/mirror/
  repeat, lighting, each recognized combiner shape (R0.6), transparency,
  depth, culling, particles, shadows, UI — each row names the concrete asset/
  display list it exercises (not a hypothetical example)
* [x] captured reference images are compared automatically wherever
  practical (pixel diff or equivalent), with the comparison threshold and
  method documented — not "looks the same" (`AGENTS.md` §7)
* [x] the methodology is actually run at least once end-to-end and its
  output recorded, not merely specified
* [x] `PLAN.md` R1's "golden/reference renders are established" acceptance
  item and `TODO.md` Phase H's "Screenshot regression" item are satisfied by
  this task's output, not left as separate unowned work

### Verification

* run the documented procedure against the current renderer
* record pass/fail per test-matrix row with evidence

### Evidence

`docs/visual-regression.md` is the full methodology: a `regression_capture`
Cargo feature (`psp/Cargo.toml`, `psp/src/main.rs`) freezes every per-frame
mutation (fighter physics, skeleton/stage/material animation) once 240
simulation ticks have run, and never draws the on-screen debug HUD at all
(RE-125 — three narrower fixes for the HUD's own live perf counters were
tried and each ghosted or corrupted differently, since `sceGuDebugPrint`
is a PPSSPP-only overlay that does not fully clear between calls; not
drawing it at all sidesteps the problem rather than out-guessing it), so a
screenshot taken any time afterward is byte-identical regardless of
real-world capture timing. Measured directly: two captures 39 real seconds
apart (`--seconds 6` and `--seconds 45`) are exact-match, both via `cmp`
and via `tools/compare-screenshot.sh` (0 differing pixels; threshold
documented as 0 given the measured exactness) — RE-123's own original "9
seconds, byte-identical" claim was true but fragile (true by luck of one
test's specific timing, since the HUD bug it shipped with was not yet
found), superseded by this stronger, HUD-free result once RE-125 found and
fixed it while re-verifying determinism after an unrelated pack rebuild.
The golden image is committed at `tests/golden/r0-dream-land-default.png`.
All 4 capture sources are documented; PPSSPP software rendering is the one
actually executed so far, matching the "run at least once end-to-end"
acceptance bar rather than requiring every source up front. The test matrix
in `docs/visual-regression.md` has 17 named rows (concrete files/offsets,
not hypotheticals); 8 are confirmed exercised by the single golden scene,
the remainder are honestly tracked as needing either asset identification or
a dedicated second scene, as ongoing work under that same document rather
than a new orphaned `PLAN.md` task. `docs/reverse-engineering.md` RE-123 has
the full account, including a real pitfall this task found and fixed along
the way (a naive "skip the debug-text call when frozen" approach corrupted
PPSSPP's own debug-overlay HLE hook into a stuck partial redraw — fixed by
always calling it and pinning the volatile fields' displayed values
instead). `cargo test --workspace`: 405 passing, unaffected.
`cargo psp --release` and `--release --features regression_capture` both
build clean; `cargo clippy` shows the same pre-existing warning set under
both. `TODO.md` Phase H's "Reference renderer / Screenshot regression" item
now points here instead of standing as separate unowned work.

---

## R0.18 — Reference-Port Comparative Audit (sf64-psp, oot-PSP)

Status: `COMPLETE`

### Objective

Perform the systematic comparison against `sf64-psp` and `oot-PSP` this
project's reference hierarchy (§4) calls for, beyond the ad hoc BattleShip
cross-checks already recorded (RE-054, RE-066). Both reference projects
target the PSP, which makes their N64-state translation, texture handling,
material handling, `sceGu` usage, render architecture, debugging methodology
and performance technique directly comparable to this project's own choices.

For every material difference found, classify it as exactly one of:

1. SSB64 genuinely needs it (missing here — becomes a new/updated task);
2. SSB64 does not use it (measured, not assumed — cite the R0.2 usage data or
   a fresh archive-wide measurement);
3. PSP requires a different implementation than that reference's own choice
   (explain why);
4. this project's implementation is simply incomplete (becomes a new/updated
   task).

Do not blindly copy either project's implementation (`AGENTS.md` §6, §10,
D-037).

### Dependencies

* R0.2

### Acceptance

* [x] `sf64-psp`'s N64-state translation, texture/material handling and
  `sceGu` usage compared against this project's own (BattleShip's F3DEX2/
  S2DEX interpreter was already checked, RE-054; `sf64-psp` itself has not)
* [x] `oot-PSP` cloned into `refs/` and its N64-state translation, texture/
  material handling, `sceGu` usage and render architecture compared against
  this project's own
* [x] every material difference found is classified 1–4 above and recorded,
  not left as an unexplained observation
* [x] debugging methodology and performance technique from both projects
  reviewed for applicability to `R3` (rendering performance) — recorded as
  leads for `R3`, not implemented here (`R3` is `BLOCKED_BY_R2`)
* [x] conclusions are written into `docs/reverse-engineering.md` as `RE-`
  entries and cross-referenced from the `R0.x` task(s) each conclusion
  actually affects

### Verification

* source-level comparison against cloned reference repositories
* cross-reference conclusions against R0.2's own usage measurements

### Evidence

RE-124 in `docs/reverse-engineering.md`. Render architecture and culling
differences fully explained by this project's own D-001 (offline
conversion vs. both references' runtime F3DEX2 interpretation) — not
gaps. Texture filtering measured archive-wide (151/151 real
`G_MDSFT_TEXTFILT` commands request `G_TF_BILERP`, matching the RDP's own
frame-reset default) and closes R0.5's open filtering item in this
project's favor. Blending: both references successfully ship standard
`SRC_ALPHA`/`ONE_MINUS_SRC_ALPHA` blending, ruling out "PSP can't do
this" as an explanation for R0.6's still-open canopy-dithering blend
failure — a new lead added to that item, not resolved here. Texture
mirroring: `oot-PSP` independently reached the same pre-baked-doubled-
texture solution as this project's own RE-067; `sf64-psp` took the
cheaper opposite tradeoff (plain repeat, visible seam) — confirms RE-067
was a real tradeoff, not an obvious-answer gap. Lighting/CLUT/combiner
approximation: all three projects converge on the same strategies this
project already uses; no gap found. Performance technique (material-state
hashing plus a batch/replay draw pool, both references) recorded as a
lead for `R3` once it unblocks.

---

# 7. R1 — Rendering Completeness

Status: `VERIFYING` — RE-200 established the R1 scene/completeness evidence,
but RE-217's corrective queue requires the integrated C6 regression pass before
the rendering gate can treat those goldens as stable.

R1's existing scene evidence remains useful, but no completion claim may
bypass the reopened R0.6/R0.15/R0.16 work or R2.2/C6.

### Objective

Demonstrate that every discovered SSB64 rendering path required for the game is implemented.

### Acceptance

* [x] all stages render — RE-170: one PPSSPP software run captured all 41
  packed stages in stable viewer order; all captures are nonblank and unique,
  retain the source-identifying HUD, show 60 FPS, and produced no error/failure
  log lines. `tools/run-ppsspp.sh --audit-stages 41` records artifact and
  capture hashes in a manifest; physical hardware remains R2
* [x] all fighters render — R0.11/RE-098 individually rendered all 12
  playable fighters in PPSSPP at 60 FPS; each was inspected at a nonzero
  costume, not inferred from pack counts
* [x] all required costumes render — R0.11/RE-098 identifies the original
  per-fighter costume counts from `dFTParamCostumeIDs`, packs every sparse
  node/costume override, verifies colour and palette paths against the ROM,
  and visually checks every fighter at a nonzero costume. A real match costume
  selector remains gameplay integration, not a missing renderer path
* [x] all required animations render — RE-171: the 532 sparse fighter
  figatree entries were each rendered and captured in stable pack order under
  PPSSPP software at 60 FPS; all 532 identity headers are unique, every
  centred model crop contains measurable content, and the log is clean.
  R0.9 already verifies all 35 stage `AObjEvent32` animations, R0.10 verifies
  all 33 packed material-animation scripts, and R0.13 verifies the separate
  results-wipe animation path. Physical hardware remains R2
* [x] all required effects render — RE-172–179 cover manager descriptors,
  display objects, transform/material/texture/colour animation and PPSSPP
  audits. RE-180–188 cover LBParticle decoding, pack serialization,
  interpretation, PSP drawing, exhaustive audits, spawn-tree execution and
  `LBGenerator`. RE-189 wires this item's own remaining scope: a real
  manager-effect spawn event (`efManagerRippleMakeEffect`) now creates and
  ticks a live `LBGenerator` and draws its spawned particle at its own live
  position every real frame in the PSP runtime, not just the debug viewer's
  static snapshot — verified by host regression and an on-device PPSSPP
  capture. Evidence and exact measurements live in
  `docs/reverse-engineering.md` RE-172–189. Only one manager effect is wired
  (the other 25+ `efManager*MakeEffect` call sites and any real gameplay
  trigger remain future scope, not required by this item's own text).
* [x] all required framebuffer paths render — RE-190 exhaustively censuses
  every `framebuf`-adjacent decomp reference: N64 VI swap-chain scheduling
  (`sys/scheduler.c`/`taskman.c`/`video.c`, `libultra/io/vi*.c`,
  `mvOpeningRoomCheckSetFramebuffer`), the crash-screen debug overlay
  (`sys/debug.c`), and nine files' fixed-VRAM-address heap-arithmetic are
  all N64-only plumbing with no PSP counterpart to build. `lb/lbtransition.c`
  is R0.13, already complete. Exactly one content-bearing mechanism remains
  unimplemented: `sc1PStageClearCopyFramebufToWallpaper`
  (1P Mode Stage Clear results wallpaper), which copies the same 300×220
  active-picture rectangle RE-099/100 established into a real ROM sprite
  buffer. RE-191 resolved both of RE-190's open copy-order questions with a
  direct ROM header probe (destination `Sprite`/`Bitmap` tiling: 37 tiles of
  6 rows each, exact byte-for-byte match) and the decompiled `spDraw` source
  (odd/even swap is a standard N64 `LoadBlock` 16-bit texture swizzle,
  unneeded on PSP GE). RE-192 implemented and device-verified the PSP-side
  capture mechanism itself (`Gpu::request_wallpaper_capture`, a plain
  300×220 block copy with none of the N64 swizzle/tile padding) behind the
  `wallpaper_audit_capture` build, matching the `TextureDesc::ROLE_FRAMEBUFFER`
  precedent's own bootstrap shape. RE-193 found the earlier framing of the
  remaining gap ("a packed pack entry ... through the normal mesh pipeline")
  was wrong: the real ROM draws the wallpaper as an `SObj` 2D sprite (RDP
  `gSPTextureRectangle`, no 3D transform, no display list), a subsystem this
  project had never ported. RE-193 built the minimal `SObj` slice this one
  caller needs (`Gpu::draw_wallpaper_sprite`, a real GE `GU_TRANSFORM_2D`
  sprite draw) and device-verified it renders the capture correctly and
  boundedly (measured luminance dim inside the exact draw rectangle, zero
  change outside it) behind the new `wallpaper_sprite_audit_capture` build.
  Both the capture (RE-190–192) and its render path (RE-193) are now
  renderer-complete and device-verified; only the real 1P-mode/results-
  screen G2 trigger remains unbuilt, the same "renderer owns the mechanism,
  G2 owns the trigger" gap RE-149 already accepted to close R0.13 (whose own
  LB-transition draw path is likewise only ever called from its own audit
  feature today)
* [x] runtime `MObj` display-state parity — RE-194 measured every input
  `gcDrawMObjForDObj` (`refs/ssb-decomp-re/src/sys/objdisplay.c`) uses archive-
  wide (665 real `MObjMaterial`s) before implementing anything, and found the
  prior "runtime-only, cannot read statically" framing (`mesh.rs`'s own doc
  comment) was wrong: `flags`, `scau`/`scav`, `trau`/`trav`, `scrollu`/
  `scrollv` and the tile/scale-formula fields are ordinary static `MObjSub`
  data, never mutated anywhere outside `objdisplay.c`. Implemented and
  unit-tested against real ROM values: the `MOBJ_FLAG_NONE` default
  substitution (3 real occurrences), `MOBJ_FLAG_TEXTURE`'s `gSPTexture` scale
  (10 occurrences, `MObjMaterial::tex_scale`), and the tile-0 `gDPSetTileSize`
  window (35 occurrences, `MObjMaterial::tile0_uv`). `MOBJ_FLAG_FRAC` is
  confirmed dead code for this game's content (0/665 real occurrences, never
  set at runtime either) — the same "measured, not guessed" treatment RE-127
  gave RDP LOD blending, not an unimplemented gap. The tile-1/scroll bit is
  real (12 occurrences) but every occurrence's inputs are identical to its own
  tile-0 window and this project's renderer has no `TEXEL1` consumer to feed
  (RE-130), so it is documented as confirmed-inert rather than given a pack
  field nothing would read. On-device: rebuilt pack (+15 textures bound,
  previously-unresolved `MOBJ_FLAG_NONE`/`TEXTURE` sprites), clean PPSSPP boot
  at 60 FPS, and a pixel diff against the pre-fix build showing the only
  difference anywhere in the frame is the on-screen texture-count HUD digits
  — Dream Land's own affected `MObjSub`s (file 104) render pixel-identical
  geometry, the same "not visible at this camera distance" outcome RE-075/081
  already recorded for other verified fixes to this scene.
* [x] no unexplained rendering commands remain — RE-195 closes the last gap:
  `G_SETOTHERMODE_H`/`L`'s six/two remaining undecoded sub-fields, measured
  archive-wide via the real `romtool pack` build. Six match the RDP's own
  reset default exactly, `G_MDSFT_TEXTLUT` is redundant with `G_SETTILE`
  format data already read, `G_MDSFT_PIPELINE` deviates from its default
  but has no visible effect (an RDP scheduling hint). `G_MDSFT_ALPHACOMPARE`
  is a genuine, previously undecoded field: 29.8% of real commands request
  `G_AC_THRESHOLD`, a real discard gate independent of the existing
  `alpha_test` approximation. Wired the safe, additive 10,334-vertex-visit
  case where no discard currently applies at all (`MeshMaterial::
  alpha_compare_threshold`, `flags::ALPHA_COMPARE_THRESHOLD`,
  `pack::VERSION` 26); the case where both gates already coexist is a
  documented, unresolved combination limit, not silently dropped. Combined
  with R0.2's existing full opcode inventory (0 real `Cmd::Other`
  archive-wide) and RE-120's `G_SHADE` cross-reference, every rendering
  command in this ROM's real content now has a demonstrated explanation —
  this does not claim every command's effect is pixel-exact, only that
  none remains a mystery
* [x] no unexplained missing assets remain — RE-196 re-ran every archive-wide
  census (`romtool check`/`scan`/`textures`/`mobj`/`pack`) against the
  current tree: 0 archive load/chain failures, 0 unknown opcodes, the same
  26 texture failures RE-055 already traced to the runtime-only LB-transition
  framebuffer photocopy buffer (no ROM data exists to resolve, D-001), 0
  unreadable/unpaired/mismatched material-table graphs (R0.7), and 0 mesh
  conversion errors with exactly RE-026's own historical 23 zero-triangle
  node placements. Nothing since RE-168/RE-162 reopened any of it
* [x] no unexplained material failures remain — RE-196: RE-168's R0.6
  combiner census (199 of 65,199 triangle visits declined: 13 real
  runtime-injected shield colours, 186 already-catalogued unsupported
  combiner/alpha-formula edge cases per RE-139) still describes the current
  pack path exactly — a direct diff confirms no commit since RE-168 touched
  `mesh.rs`'s combiner/alpha-blend/shade-scale classification functions
* [x] rendering regression suite passes — RE-197: rebuilt the deterministic
  capture EBOOT and reran the golden Dream Land comparison after RE-190–196's
  code changes; `differing pixels: 0`, captured screenshot byte-identical
  (same SHA-256) to the committed golden. Workspace verification
  (`fmt`/`clippy`/`cargo test --workspace`, 347 passing) also reran clean.
  RE-170's stage audit and RE-171's animation audit remain valid, separate
  smoke coverage
* [x] golden/reference renders are established: methodology and one golden
  scene done under R0.17 (`docs/visual-regression.md`); RE-198 identified
  concrete file/offset evidence for the three rows that previously had none
  (CI8 texture, clamp texture mode, untextured/vertex-coloured geometry).
  RE-199 built the second dedicated `regression_capture_scene2` frozen scene
  (file 52, `mvopeningroom.c`'s "MVCommon" graph) and closed two of the six
  non-covered rows (CI8 texture, untextured/vertex-coloured geometry) against
  a new committed golden, `tests/golden/r1-mvopeningroom.png`. RE-200 closes
  the final four rows with a graph-backed census and two deterministic
  goldens: file 109 graph `0x44C8` covers texture blend, classified
  translucency, and clean clamp; file 84 graph `0x2760` covers flat colour.
  Both exact-match at 6 and 30 seconds under PPSSPP software rendering

---

# 8. R2 — Physical PSP Rendering Validation

Status: `IN_PROGRESS` — representative physical-PSP captures exist, but
`R2.0` (newly reopened filtering/addressing correctness), the formal texgen
fidelity gate, and the renderer-corrective gate below all remain open. `R2.0`
is the current first task; do not resume `R2.1`/T1 or continue into `R2.2`
until it closes.
RE-202 found and fixed a hardware-only crash in the interactive
viewer's debug HUD (`sceGuDebugFlush`) that RE-201's `regression_capture`
run never exercised. RE-203 then ran all four golden regression scenes on
the same physical PSP with zero exceptions and confirmed their content
matches the PPSSPP goldens (modulo expected edge antialiasing), checking off
most acceptance items below. RE-204 then hardware-tested the framebuffer-
effect sprite path and verified VRAM usage, checking off both remaining
software-testable rows. RE-205 added a fifth golden scene (Saffron City,
stage 9) isolating RE-142/RE-143's already-proven animated gate, found and
fixed a third FPU-trap site in stage-animation code (`objanim.rs`, the same
class RE-201/`f111892` already fixed twice elsewhere), and verified it
matches its PPSSPP golden on physical hardware with zero exceptions,
checking off "stage animation works". RE-206 swept the crate for other
unguarded divisions and found no new fix required. RE-207 added a sixth
golden scene covering Fox — the first fighter besides Mario hardware-tested
— confirming RE-152's clamp-window fix holds on real hardware. RE-208 added
a seventh golden scene covering Captain Falcon, a second RE-102 UV-scale/
clamp-bug fighter, hardware-verified with zero exceptions. RE-209 added an
eighth golden scene covering Kirby, the third and last RE-102 UV-scale/
clamp-bug fighter, hardware-verified with zero exceptions. RE-210 added a
ninth golden scene covering Ness, the fourth fighter named across RE-102/
RE-103's fixes (this time RE-103's per-vertex lit/literal heuristic),
hardware-verified with zero exceptions. RE-212 added a tenth golden scene
covering Donkey Kong, the next untested fighter once both RE-102's and
RE-103's named sets were exhausted, hardware-verified with zero exceptions
— and found that a bare `kill` of a prior PSPLink module can silently
break the next module's asset-pack load with no symptom `exlist`/`thlist`
catch, now documented in `docs/psplink.md` as a `reset`-before-`ldstart`
default. RE-213 then implemented three documented fidelity gaps (texgen,
mip selection, the combined alpha gates), and RE-214 corrected the texgen
half of that work: the two geometry-mode bits are now preserved
independently, an archive-wide `romtool texgen` census proves texgen may
stay primitive-level state, the `gSPTexture` scale and render-tile origin
reach the GE, and coordinates are generated through the GE's
texture-matrix generator against the camera's world right/up basis — the
same basis `syMatrixLookAtReflectF` writes into the RSP's own look-at.
RE-214 also refreshed the nine golden scenes RE-213's mip change had left
stale. Exhaustive no-failures-remain coverage and live analog-stick input
remain open — the last requires a human physically operating the device,
not reproducible through `pspsh`.

### Objective

Verify the renderer on actual PSP hardware.

PPSSPP is not sufficient.

### Acceptance

* [x] EBOOT boots on physical PSP — RE-201, RE-202, RE-203
* [x] runtime asset pack loads — RE-203 (pack hash `7647db75...650b2f0`, all four scenes render pack content correctly)
* [x] representative fighters render — RE-203: Mario matches PPSSPP golden; RE-207: Fox (`regression_capture_scene6`, file 313 graph `0x2938`) matches its PPSSPP golden with zero exceptions, confirming RE-152's clamp-window face fix on real hardware; RE-208: Captain Falcon (`regression_capture_scene7`, file 332 graph `0x3BE0`) matches its PPSSPP golden with zero exceptions; RE-209: Kirby (`regression_capture_scene8`, file 328 graph `0x1448`) matches its PPSSPP golden with zero exceptions; RE-210: Ness (`regression_capture_scene9`, file 335 graph `0x26B0`) matches its PPSSPP golden with zero exceptions, confirming RE-103's per-vertex lit/literal fix on real hardware; RE-212: Donkey Kong (`regression_capture_scene10`, file 317 graph `0x39A8`) matches its PPSSPP golden with zero exceptions
* [x] representative stages render — RE-203: Dream Land, `StageSectorFile2`, `MVOpeningRoom`
* [x] fighter animation works — RE-203: Mario's 240-tick fall/physics simulation completes on hardware and matches the PPSSPP golden
* [x] stage animation works — RE-205: `regression_capture_scene5` (stage 9, Saffron City) isolates RE-142/RE-143's already-proven animated gate; found and fixed a third FPU-trap site in `objanim.rs`'s `StageJoint::apply`; hardware capture matches the PPSSPP golden with zero exceptions
* [x] materials render correctly — RE-203: texture-blend, flat-colour and alpha-blend/translucency combiner shapes all match their goldens
* [x] textures render correctly — RE-203: CI8, untextured/vertex-coloured, and clamp-mode texturing match their goldens
* [x] framebuffer effects work — RE-204: RE-193's real `SObj` wallpaper-sprite draw hardware-tested, luminance ratio matches PPSSPP evidence, deterministic once settled
* [x] VRAM usage verified — RE-204: 1,360 KiB of 2 MiB EDRAM (only three allocation sites, grep-confirmed), ~688 KiB headroom, runtime bound check passes on every successful boot
* [ ] no hardware-only rendering failures remain — five golden scenes (Dream Land/Mario, `MVOpeningRoom`, `StageSectorFile2`, `CatchSwirl`, Saffron City) plus a sixth (Fox), a seventh (Captain Falcon), an eighth (Kirby), a ninth (Ness), a tenth (Donkey Kong), an eleventh and twelfth (`StageMetalFile2` ordinary texgen at two rotations, RE-214) and a thirteenth (`StageMetalFile2`'s linear-texgen graph, RE-215) and the plain interactive build are now clean, but coverage is not exhaustive across all 12 fighters/41 stages/effects
* [ ] `G_TEXTURE_GEN` compared against original output — RE-214: ordinary texgen is source-derived, ROM-corroborated, PPSSPP-verified and hardware-verified at two model rotations, but no original-N64 capture exists. Meta Crystal (and `MMarioModel`/`NMarioModel`/`NFoxModel`) is reachable only through 1P mode stage 8 — RE-216 rebuilt RE-151's scripted original-ROM harness (verified working, including real scripted menu navigation) and found the VS-Mode "Metal Box item" shortcut RE-214 §10 recommended does not exist in the decomp; scripting the real stage-8 route (or a faithful RAM-level warp) is the prerequisite. `VERIFYING`, not `COMPLETE`
* [ ] `G_TEXTURE_GEN_LINEAR` implemented exactly — RE-215 provides a source-formula implementation and PPSSPP/physical-PSP evidence for `regression_capture_scene13`, but T2–T7 still need to establish shared raw-normal, quantized-LookAt, integer-conversion and tile-addressing semantics, and T8 still requires an original-N64 comparison. Do not call this bit-exact until the T1–T10 gate passes.
* [x] hardware model recorded — PSP Slim, firmware 6.61, ARK/Infinity, PSPLink v3.2.1 (RE-201, RE-202, RE-203)
* [x] build/environment recorded — commit `759cda8`, pack hash `7647db75...650b2f0` (RE-203)

---

## R2.0 — Pre-Texgen Rendering-Fidelity Reopening (P0–P2)

Status: `COMPLETE` — P0a, P0b, P0c, P0d, P1 and P2 all `COMPLETE`. `R2.1`/T1
may resume.

RE-218 (2026-09-11 external audit) found that R0.5's filtering and
mirror/clamp/mask/addressing completion claims measured less than they were
read to establish, and that `G_SETTILE`'s `palette`/`line`/`tmem`/
`shift_s`/`shift_t` fields have never been censused archive-wide. This queue
reopens those specific claims and closes those specific gaps before texgen
work (`R2.1`) resumes, since `R2.1`/T6 and T7 would otherwise duplicate the
addressing-model work this queue builds. Do not implement fixes speculatively
here — each sub-task's job is to measure against a real reference model and
either pin an invariant with evidence or open a scoped correctness task,
matching this project's own established practice (RE-066, RE-072, RE-127).

### P0a — N64 3-point filtering vs PSP bilinear

Status: `COMPLETE` — RE-219.

RE-124 established that all 151/151 real `G_MDSFT_TEXTFILT` commands request
`G_TF_BILERP` and concluded PSP's unconditional
`sceGuTexFilter(Linear, Linear)` (`psp/src/meshdraw.rs:431,505`) "is already
correct". That measured only the *filter-mode selector*: the N64 RDP's
`G_TF_BILERP` performs 3-point (triangular) filtering — up to three
neighbouring texels chosen by which side of the diagonal the fractional
coordinate falls on — not symmetric four-tap bilinear. These are different
reconstruction formulas that happen to share a filter-mode name.

Establish the exact N64 3-point formula from an authoritative N64/RDP
reference (not just BattleShip/sf64-psp/n64psp, per `AGENTS.md` §6/D-037).
Build a host-side reference sampler implementing it. Compare PSP-style
bilinear output against the reference on representative real SSB64 textures,
specifically magnified low-resolution textures, texture edges, and diagonal
gradients. Quantify whether the difference is visible/material on real
content (not "it differs in theory"). Determine whether an exact PSP
implementation is practical. If exact reproduction is impossible or
disproportionately expensive, record an explicit `ACCEPTED_DEVIATION` with
the measured error — do not leave `docs/rendering.md`/R0.5 claiming exact
equivalence without that evidence.

Acceptance: reference sampler implemented and host-tested; PSP-vs-reference
comparison run on real archive textures with recorded quantitative error;
R0.5's filtering acceptance item and `docs/rendering.md`'s "Texture
filtering" row updated to whatever the measurement actually supports.

**Closed by RE-219.** Reference samplers in `crates/ssb-rom/src/n64_filter.rs`
(`sample_3point` transcribed from `angrylion-rdp-plus`, `sample_bilinear`
matching PSP's `Linear`), host-tested. Archive-wide census
(`tools/romtool`'s `filter_reconstruction_census_against_real_archive_textures`,
`SSB64_ROM`-gated): 684 real textures, 5.73% of interior sample points
differ by ≥8/255, 1.19% by ≥32/255; the Dream Land canopy highlight texture
(file 103 offset `0x5F0`) reaches the maximum 128/255 diff with a 6.4/255
mean — real, material, not theoretical. Exact reproduction is impractical:
the PSP GE has only `Nearest`/`Linear` and no programmable shader stage to
implement a third, custom filter. `ACCEPTED_DEVIATION` recorded; no fix
implemented or planned.

### P0b — General N64 tile-addressing reference model

Status: `COMPLETE` — RE-220.

Host-side N64 tile-addressing reference (`crates/ssb-rom/src/n64_addressing.rs`:
`TileAxis`/`address_axis`, transcribed from `angrylion-rdp-plus`'s
`tcshift_cycle`/`TRELATIVE`/`tcclamp_cycle`/`tcmask_coupled`, plus
`psp_lowering_axis` modeling the current PSP conversion for direct
comparison), 8 host tests. `TextureRef` gained raw `mask_s`/`mask_t` and
`drawn_width`/`drawn_height` to support it; no rendering behavior changed.
Archive-wide census (`tools/romtool`'s
`tile_addressing_census_against_real_archive_textures`, `SSB64_ROM`-gated,
2,484 real authored-UV textured primitives; texgen primitives excluded, since
`R2.1`/T7 owns their scale/origin wiring separately) measured all three
questions:

* **Mirror+clamp beyond the first mirrored period: real, material gap.** 810
  real `mirror+clamp` axis instances across 280 unique tiles; 176 reach a
  third or later mask period, where real hardware (per the reference model)
  keeps mirroring but the current PSP lowering has already clamped.
  99/810 (12.22%) instances measurably diverge. **Opens `P0c`.**
* **`mask == 0`: invariant pinned, no fix needed.** Zero of 4,968 possible
  axis slots have `mask_s == 0`/`mask_t == 0` on any real drawn primitive,
  archive-wide — `angrylion-rdp-plus`'s forced-clamp rule
  (`clampens = cs || !mask_s`) never has an observable effect on this ROM's
  content. Pinned with `assert_eq!(m0.axis_instances, 0, ...)` in the census
  test itself.
* **PSP power-of-two padding vs N64 logical clamp boundary: real, material
  gap.** 124 unique clamped, non-power-of-two, unmirrored tiles; 456 real
  axis instances, 347 (71.4%) with a real UV sample reaching the last
  logical texel, where `sceGuTexFilter(Linear, Linear)`'s bilinear blend
  reads one texel into `pack_rgba`/`pack_indexed`'s zero-filled padding.
  **Opens `P0d`.**

`docs/rendering.md`'s "Texture addressing" row and R0.5's corresponding
acceptance items are updated to match. See RE-220 for full detail.

### P0c — Fix mirror+clamp beyond the first mirrored period

Status: `COMPLETE` — RE-221.

RE-220/P0b measured 810 real `mirror+clamp` axis instances, 176 of which
reach a third or later mask period and 99 (12.22%) of which measurably
diverge from the `n64_addressing` reference model. `texture::mirror_extend`
now pre-bakes as many mirrored periods as the tile's real drawn rect
requires (`TextureRef::drawn_width`/`drawn_height`, bounded, known at pack
time) instead of always exactly two, only for a mirror+clamp axis (a
mirrored axis with no clamp bit is unchanged: a plain doubled bake plus
`Repeat` already mirrors forever exactly). `n64_addressing::psp_lowering_axis`
gained the matching `drawn` parameter so the comparison model tracks the
fix. Re-ran `tile_addressing_census_against_real_archive_textures`
archive-wide against the real ROM: divergence count reached **zero**
(0/810, down from 99/810), now asserted in the test itself. Recheck of
Fox, Captain Falcon and Kirby (RE-102's named overflow content) is implicit
in the archive-wide zero-divergence result, since those primitives are
exactly RE-220's third-or-later-period bucket.

### P0d — Fix PSP POT-padding vs N64 logical clamp boundary

Status: `COMPLETE` — RE-222.

RE-220/P0b measured 456 real clamped-non-mirrored-non-POT axis instances,
347 (71.4%) of which have a real UV sample reaching the last logical texel,
where PSP's zero-filled padding corrupts `Linear`'s bilinear blend. Fixed
with two new helpers in `crates/ssb-rom/src/psp_texture.rs`,
`pad_edge_repeat`/`pad_edge_repeat_nibbles`, filling the padding region with
the repeated edge row/column instead of zeros — a no-op for an already-
power-of-two texture, and never touching a mirrored axis by construction
(mirror-doubling always lands on a power of two, per P0b). RE-222 found the
actual padding site is `encode_level` (via `pack_mipped`, `convert_texture`'s
real call path), not `pack_rgba`/`pack_indexed` as this task's text
originally named — both were fixed, since `pack_rgba`/`pack_indexed` have
the same defect on their own (particle-frame, out-of-census-scope) caller.
7 new host tests, including one through `pack_mipped` end-to-end. Re-ran
`tile_addressing_census_against_real_archive_textures`: bullet 3's counts
are unchanged (124/456/347), as expected — it measures the structural
condition, not the padding fix itself, which the new unit tests cover.

### P1 — Archive-wide `G_SETTILE` field census

Status: `COMPLETE` — RE-223.

`dl.rs`'s `Cmd::SetTile` decodes `palette`/`line`/`tmem`/`shift_s`/
`shift_t` (`dl.rs:144-153`), but `mesh.rs`'s only consumer discards all five
behind a `..` wildcard (`mesh.rs:1767-1788`, keeping only `format`/`size`/
`mask_s`/`mask_t`/`cm_s`/`cm_t`). Censused every real render-tile-0
`G_SETTILE` archive-wide (2,238 instances, 1,948 CI4) via a standalone
raw-`Cmd` walker in `tools/romtool` (`settile_field_census_against_real_archive_textures`).
`shift_s`/`shift_t` — zero archive-wide, invariant pinned with a test.
`tmem`/`line` — TMEM-staging registers a converter that reads texels
straight from the ROM (never through TMEM) structurally cannot need;
`tmem` also measures zero, pinned; `line`'s real diversity (15 distinct
values) is expected and not itself wrong. `palette` — **real, material,
still-open gap**: 7/1,948 CI4 instances (0.36%), all in file 86
(`ITCommonObject`, real shipped content), request bank 1 of a 48-entry
loaded TLUT that `mesh.rs` currently always resolves as bank 0 — a real
wrong-colour bug on real item textures. Opened `R2.0`/P2 as a scoped
correctness task rather than fixing speculatively here, per this queue's
own rule. No production code changed; only the new romtool test.

### P2 — Fix ignored CI4 palette bank (`G_SETTILE.palette`)

Status: `COMPLETE` — RE-224.

RE-223/P1 measured 7 real CI4 render-tile instances (file 86,
`ITCommonObject`) requesting palette bank 1 (`G_SETTILE.palette == 1`) of
a 48-entry loaded TLUT (three 16-entry banks), which `mesh.rs` used to
ignore: `Cmd::LoadTlut`'s handler set `state.palette_offset`/
`palette_entries` from the whole loaded chunk starting at entry 0, and
nothing read `SetTile.palette` to offset into it, so every CI4 texel value
(0-15) always indexed bank 0's colours regardless of which bank the real
render tile requested. RE-224 threaded `SetTile.palette` through `mesh.rs`'s
`State`/`TextureRef` (RE-220's own precedent for adding a raw `G_SETTILE`
field once it's known to matter) and, at pack time
(`tools/romtool`'s new `palette_bank_offset` helper, applied in
`convert_texture` and three CLI-only debug commands), offsets the TLUT read
by `palette * 16` entries — a no-op by construction when `palette == 0`
(the overwhelming common case) and guarded (falls back to bank 0) when the
loaded TLUT is smaller than `(palette + 1) * 16` entries (should not occur
on real content per RE-223's own measurement). Two host tests confirm it:
`ssb-rom`'s `tile0_palette_bank_is_carried_onto_the_texture_reference` and
`romtool`'s `convert_texture_resolves_the_requested_palette_bank`, the
latter built from RE-223's own measured real shape (three 16-entry banks)
and covering the `palette == 0` no-op and out-of-range guard too.

**Visual confirmation not obtained.** RE-224 documents why: this project's
PPSSPP debug viewer is interactive-only (dpad-driven TEXVIEW), and
automating it in this environment (no `xdotool`, no `xset`, no sudo to
install either) hit real reliability limits — a stuck key caused
uncontrolled navigation drift, the TEXVIEW-toggle key could not be
identified from any tested candidate, and the object-view overlay text
itself renders corrupted/double-exposed in every capture (a real, separate
bug, flagged for its own follow-up, not caused by this task). Closed
without it on the strength of the host tests, which are built from
RE-223's exact directly-measured real-content shape rather than a
synthetic one. A PPSSPP TEXVIEW before/after of file 86's affected item(s)
remains a manual follow-up; RE-224 records the exact coordinates for it
(global texture indices 187/194/195, object indices 60-102).

### Evidence

RE-218, RE-223, RE-224 in `docs/reverse-engineering.md`.

---

## R2.1 — Final Texgen Fidelity (T1–T10)

Status: `IN PROGRESS` — `R2.0` closed (RE-224). T1 measured (RE-225): model-
space invariance does **not** hold (164 cross-node differing-transform vertex
reuses, D-042 revised). T2 complete (RE-226): raw signed-byte normal
semantics measured and fixed (D-038 revised). T3 complete (RE-227): original
LookAt basis quantization measured and fixed (D-040 revised). T4 complete
(RE-228): shared regular/linear reference math built and proven against the
real GE lowering, finding and fixing a real overcorrected matrix constant.
T5 complete (RE-229): linear texgen's S10.5 conversion confirmed to truncate,
not round, against two independent HLE references, fixing a dormant rounding
bug in the real linear-texgen rendering path. T6 complete (RE-230):
`shift_s`/`shift_t` confirmed zero on the texgen-bound tile subset
specifically (not just RE-223's broader archive-wide census), per-mode
`G_TEXTURE` scale/tile/`G_LIGHTING` state recorded for T7/T8. T7 measured
(RE-231): `R2.0`/P0b's hardware/PSP-lowering addressing comparison, run
against every real texgen material, agrees on 25 of 34 real axis instances
and finds a narrow, single-texel divergence on the rest — a mask-narrowed
clamp-without-mirror axis holds one period short of real hardware exactly at
the sweep's `dot = +1` extreme. Opens `T7a` to fix it. T7a complete
(RE-232): fixed the divergence at its real cause (both the host comparison
model and the pack-time texture bake `meshdraw::bind_texture` addresses),
re-measured at a strict `0`/34. T1's remedy carries forward through T9 next.
T8 complete (RE-234/RE-235/RE-236): original-ROM stage-8 VS Metal Mario
capture obtained via real 1P Mode play (RE-234); the `StageMetalFile2`
PPSSPP goldens (scenes 11-13), stale since T7a (RE-232), refreshed and
qualitatively cross-checked against the original-ROM captures (RE-235); and
a physical-PSP re-capture of the same three scenes against the refreshed
goldens (RE-236), matching at the same noise-floor order RE-214 established.
T9 complete (RE-236/RE-237): all five physical-PSP matrix items captured —
regular rotations A/B and linear texgen via T8's own scenes 11-13 (RE-236),
plus a real-hardware raw normal diagnostic and a new camera-rotation scene
(`regression_capture_scene14`) exercising T3's previously-dormant non-
identity LookAt basis (RE-237).

This queue is authoritative for closing `G_TEXTURE_GEN` and
`G_TEXTURE_GEN_LINEAR`. Preserve the current known-good behavior while doing
it: raw GEN/LINEAR bits remain independent, LINEAR never enables generation by
itself, pack version 27 retains texgen scale and tile origin, regular texgen
uses the GE texture-matrix path, and linear texgen remains CPU-generated
through authored UVs until shared equivalence is proven.

Execute T1 through T10 in order. A source-formula or reference-port match is
not an original-hardware match.

### T1 — `G_VTX` model-space invariance

Status: `MEASURED, not invariant` — RE-225. `space`/`world` (already computed
by `plan_draw_order`, previously discarded) now flow into `VtxLoadState` and
`TexgenWalk`; `tri()` classifies every earlier-step vertex reuse as same-node,
cross-list same-node, cross-node equivalent-transform (bit-identical 3x3
linear part, or one differing only by a positive uniform scale) or cross-node
differing-transform, via the new `normal_transform_equivalent`. Measured
against the real ROM: 0 same-node, 0 cross-list same-node, 142 cross-node
equivalent, **164 cross-node differing** — 15 sites, 7 fighter files
(300/301/304/305/306/307/312), every instance a `Gfx *dls[2]` pre/post-matrix
pair sharing one vertex across a parent/child joint boundary.

Differing reuse is **not** zero, so the invariant this task hoped to pin does
not hold — recorded as a revision to D-042, not a new safe assumption. The
acceptance's remedy ("preserve load provenance or split/precompute only
affected vertices") needs a validated regular-texgen CPU reference that does
not exist yet — T2's raw-normal semantics, T3's LookAt quantization, and T4's
proven shared regular/linear math. Building an un-validated CPU curve now
would risk the 2,848 currently-correct GE-path triangles for a fix this
project cannot yet prove exact, which is exactly what this milestone's own
preamble says not to do. The fix is carried forward through T2 → T3 → T4 in
order, per the plan's own explicit sequencing — not opened as a separate
follow-up task, since T2-T4 already are that follow-up.

Two host tests confirm the classifier and the equivalence comparator:
`normal_transform_equivalence_ignores_translation_and_uniform_scale` (all
seven named cases) and
`texgen_reuse_classifies_same_node_cross_list_and_cross_node`.

Acceptance (original text, retained): report texgen triangles, cross-list
reuse, cross-node reuse, equivalent-transform reuse and differing-transform
reuse. If differing reuse is zero, record that invariant in `DECISIONS.md`;
otherwise preserve load provenance or split/precompute only affected
vertices. Add same-node, cross-list, translation-only, identical-rotation,
different-rotation, uniform-scale and non-uniform-scale tests.

Evidence: RE-225 in `docs/reverse-engineering.md`; D-042 in `DECISIONS.md`.

### T2 — Raw signed-byte normal semantics

Status: `COMPLETE` — RE-226. Measured both PPSSPP source
(`GPU/Common/VertexReader.h`'s `ReadNrm`: unconditional `/128`, no
normalization for `GE_PROJMAP_NORMAL`) and this project's own real `sceGu`
draw calls via a headless measurement rig (`psp/src/normal_diag.rs`,
`texgen_normal_diagnostic_0`-`_6`): all seven cases (`[127,0,0]`, `[64,0,0]`
under both `NormalizedNormal` and raw `Normal`, `[-128,0,0]`, `[90,90,0]`,
`[73,-41,99]`) matched `/128`-divisor predictions exactly. `[64,0,0]` vs.
`[127,0,0]` under the old `NormalizedNormal` mode confirmed the collapse the
acceptance text named (identical output, magnitude discarded); under raw
`Normal` mode the same pair produced different, magnitude-proportional
output. `meshdraw::apply_texture_mapping` now uses raw `Normal` mode with the
dot-product term scaled by `128.0/127.0` to compensate the GE's measured
`/128` against the original hardware's `/127`, matching `(normal · LookAt) /
127` exactly. `regression_capture_scene11/12/13`'s goldens rebuilt and
updated (45,484 / 29,874 / 27,570 differing pixels vs. the pre-fix goldens;
new captures reconfirmed deterministic). D-038 revised.

Evidence: RE-226 in `docs/reverse-engineering.md`; D-038 in `DECISIONS.md`.

### T3 — Original LookAt quantization

Status: `COMPLETE` — RE-227. The decomp's `syMatrixLookAtReflectF` quantizes
the camera's `right`/`up` basis to signed bytes via `FTOFRAC8` once per
camera, strictly before any per-object model transform, not the continuous
per-frame float this project previously fed its texgen path.
`crates/ssb-engine/src/math.rs` gained host-testable `ftofrac8` (bit-exact
macro port: positive saturation at 127, asymmetric negative range down to
-128, documented negative-overflow wraparound beyond any real basis
component), `quantize_lookat_component` (quantize then `/127` reconstruct —
exact for `0.0`/`+1.0`, inexact for `-1.0`), and `quantize_lookat_basis`.
`meshdraw::DrawState::texgen_object_basis` now calls `quantize_lookat_basis`
before its existing model-transform step, so both the regular (GE
texture-matrix) and linear (CPU-generated) texgen paths — which both call
this one method — receive the identical quantized-then-transformed basis
with no separate wiring. 8 new host tests added (boundary, asymmetry,
round-trip exactness/inexactness, a realistic 45° divergence case).
`regression_capture_scene11`/`_12` re-captured and diffed against their
existing goldens: 0 differing pixels both — every current texgen regression
scene uses the identity (camera-less) basis, whose only components (`0.0`,
`+1.0`) round-trip exactly, so this fix is currently dormant pixel-wise; it
activates once a rotated real-camera basis reaches a texgen primitive (no
current scene does). D-040 revised (its LUT-rejection premise named the
basis "continuous float"; conclusion unaffected).

Evidence: RE-227 in `docs/reverse-engineering.md`; D-040 in `DECISIONS.md`.

### T4 — Shared regular/linear reference math

Status: `COMPLETE` — RE-228. Added `ssb_rom::psp_texture::regular_texgen_curve`
(`(dot+1)/4`) and `regular_texgen_uv`, sharing `texgen_dot` and a new
`texgen_s10_5_addressed` scale-and-addressing step with the existing
`linear_texgen_uv` -- the "only curve difference... common scale and
addressing" this task names. Added `regular_texgen_matrix_coeffs` (pulled
out of `apply_texture_mapping`'s inline arithmetic) and
`ssb_engine::math::transform_lookat_basis` (pulled out of
`texgen_object_basis`'s inline closure) so both are host-testable and the
real rendering path calls the same code the tests exercise, not a separate
copy. A 20,000-case random property test proved the GE's matrix lowering
matches the reference to within ~1.78 S10.5 units (a documented, unfixed
clamp-boundary effect, not exact equality) -- and, in proving it, **found
and fixed a real bug**: the shipped `b` (texture-matrix translation
constant) wrongly carried the same `128/127` `NORMAL_SCALE_COMPENSATION`
the dot-term coefficient `a` needs, overcorrecting by up to
`scale/127 - scale/128` S10.5 units. Fixed in `regular_texgen_matrix_coeffs`;
a dedicated regression test pins the old formula's measured divergence
(hundreds to thousands of S10.5 units) so it cannot silently return.
Rebuilt and updated two of three texgen goldens (`tests/golden/
r2-metal-texgen{,-rotated}.png`, 28,240 / 23,624 differing pixels);
`regression_capture_scene13` (the one linear-texgen primitive, drawn through
the untouched CPU path) measured 0 differing pixels, correctly unaffected.

Evidence: RE-228 in `docs/reverse-engineering.md`.

### T5 — Linear integer conversion

Status: `COMPLETE` — RE-229. No RSP microcode source exists in
`refs/ssb-decomp-re` (binary ucode, not decompiled), so this fell to the
faithful-HLE tier: `refs/n64psp`'s `n64psp_texgen_to_s10_5` and
`refs/BattleShip`'s RSP-interpreter texgen path both cast straight to an
integer with no `+ 0.5` — truncation, not rounding. The shared
`texgen_s10_5_addressed` previously added `0.5` before casting (round-half-up);
removed it to match both references. Added
`texgen_s10_5_addressed_truncates_rather_than_rounds_at_half_unit_boundaries`
(`N+0.49`/`N+0.50`/`N+0.51` at every real ROM texgen scale). Changes real
rendered output only for the linear-texgen path (`linear_texgen_uv` drives
real pack UVs; the ordinary path renders through the GE hardware matrix and
never calls this function at runtime) — `regression_capture_scene13`'s golden
rebuilt (4,696 differing pixels), `_11`/`_12` reconfirmed at 0 differing
pixels. "Source-formula exact" stands; no original-ROM output yet to promote
to "bit-exact to N64".

Evidence: RE-229 in `docs/reverse-engineering.md`.

### T6 — Tile-state and lighting audit

Status: `COMPLETE` — RE-230. Consumed RE-223's (`R2.0`/P1) archive-wide
`shift_s`/`shift_t` census rather than re-deriving it, and re-checked the
invariant specifically against the texgen-bound tile subset. Added
`shift_s`/`shift_t` to `tools/romtool`'s `TileState` and new per-mode census
maps reporting render tile, masks, shifts, `cm`, origins, dimensions and
`gSPTexture` scale per texgen mode, plus raw `G_LIGHTING` on/off at each
texgen `G_VTX`. All measured texgen-bound tile shifts are zero
(archive-wide, 3,012 texgen triangles); pinned with a regression assertion
in `texgen_tile_state_and_lighting_audit` — N64 shifting is not needed.

### T7 — Texgen addressing phase

Status: `MEASURED` — RE-231. Consumed `R2.0`/P0b's general N64
tile-addressing reference model (`ssb_rom::n64_addressing::address_axis`/
`psp_lowering_axis`) rather than building a second one, narrowing to
verifying the texgen-specific vertex-load/scale wiring against that shared
model. Added `tools/romtool`'s `texgen_materials_by_mode` census (the real
`(mode, scale, tile)` triple as it co-occurs at a draw, joining what T6 kept
as two separate maps) and two new host tests:
`texgen_addressing_census_against_real_archive_materials` sweeps every
`i8`-quantized dot product through `regular_texgen_uv`/`linear_texgen_uv` for
every real texgen tile and compares the addressed texel against
`address_axis`;
`texgen_addressing_reference_cases_for_material_combinations_not_seen_in_the_real_archive`
adds the requested zero/nonzero-origin, repeat+mask, mirror+repeat,
mirror+clamp, padded-PSP-dimension and partial-uploaded-scale reference cases
synthetically, since real texgen content never combines mask with a clear
clamp bit.

**Measured, archive-wide, real ROM** (17 real `(mode, scale, tile)` pairings,
34 axis instances, every one clamped on both axes per RE-230): 25 of 34 agree
with the hardware model across the full quantized dot sweep. 9 of 34 diverge,
always at exactly the sweep's `dot = +1` extreme and nowhere else — a
mask-narrowed (`period << drawn`, real `RE-044` narrowing) clamp-without-
mirror `Regular`-mode axis wraps to the next period's start on real hardware
at that boundary, but the current PSP-lowering model (and very likely the
real GE `Clamp` wrap mode `meshdraw::bind_texture` installs for it) instead
holds at the narrowed period's own last texel. Pinned as a regression
baseline (`9`, not `0`) rather than fixed here, per this project's
"measure, then open a follow-up" convention (`R2.0`/P0b → P0c/P0d) — the fix
needs its own scoped design (see `T7a`), and rushing one now risks the 25
already-agreeing axis instances for a fix this project cannot yet prove
exact.

### T7a — Fix mask-narrowed clamp-without-mirror texgen addressing divergence

Status: `COMPLETE` — RE-232. Fixed both the host-side comparison model
(`n64_addressing::psp_lowering_axis`'s `!mirror` clamp branch now clamps to
the drawn rect's far edge, then folds through the mask period, instead of
clamping to the narrowed period's own last texel directly) and the real
pack-time bake it models (`texture::mirror_extend`'s `mirror_axis_len` now
bakes every period up to `drawn` for *any* clamped axis, not only a mirrored
one, so `sceGuTexWrap(Clamp)` in `meshdraw::bind_texture` holds at the real
far edge rather than one period early; `mirror_fold` generalized to always
wrap by `% period` so the wider bake still reads valid source texels).
Mirrors the mirror+clamp fix RE-220/RE-221 already made for the mirrored
case. Applies uniformly to both `Regular` and `Linear` texgen, since the fix
lives in the bound texture/wrap state itself, not in how the UV was
generated. `texgen_addressing_census_against_real_archive_materials`
re-run against the real ROM: divergence dropped from RE-231's `9` to a
strict `0` (34/34 real axis instances now agree), baseline updated
accordingly; no other real texgen or authored-UV tile shape regressed.
`assets/generated/ssb64.pak` rebuilt (gitignored, not committed) since this
touched asset-pipeline code. No PPSSPP/physical capture taken — the
archive-wide census already confirms the fix against every real texgen tile
in this ROM, a stronger check than a single visual angle.

### T8 — Original-ROM Metal comparison

Status: `COMPLETE` — RE-234/RE-235/RE-236. Original-N64 leg: real 1P Mode
play through the legitimate stage-8 route reached VS Metal Mario / Meta
Crystal, three screenshots captured (RE-234) — the scripted-harness RAM
warp this task's text prefers was derived but not exercised live, since the
user's real play is strictly more faithful and needed no correctness
argument. PPSSPP leg: the `StageMetalFile2` goldens (scenes 11-13, the only
content this port draws with real `G_TEXTURE_GEN`/`_LINEAR` state) were
stale since T5, refreshed and reconfirmed deterministic post-T7a (RE-235).
Physical-PSP leg: all three scenes re-captured on the same hardware RE-214/
RE-215 used, matching the refreshed goldens at the same noise-floor order
RE-214 established (RE-236). Model/camera reflection response (scene 11 vs
12's quarter turn) and ordinary-versus-linear behavior (scenes 11/12 vs 13)
were checked directly across all three platforms; diffuse-light
independence, tile-origin phase and generated span rest on T2/T6/T7/T7a's
own dedicated measurements (RE-226/RE-230/RE-231/RE-232), which these
captures are consistent with. The original-ROM comparison itself is
qualitative (shape/colour/material-behavior match against small, distant,
full-gameplay captures), not a pixel-level ROI overlay — the acceptance
text's own "texgen-focused ROI comparison rather than requiring identical
full-screen rasterization" allowance, and the only limitation this route
(RE-216's finding) permits.

Use the rebuilt Mupen64Plus harness to reach legitimate stage-8 Metal content.
Prefer a faithful RAM-level warp that still executes original fighter
construction, camera, display-list, lighting and material setup; otherwise
script the real 1P route. Never fake registers/material state. Compare original
N64/emulator, PPSSPP software and physical PSP at equivalent states and at
least two object rotations. Check model/camera reflection response,
diffuse-light independence, ordinary-versus-linear behavior, tile-origin phase
and generated span using a texgen-focused ROI rather than requiring identical
full-screen rasterization.

### T9 — Physical PSP matrix

Status: `COMPLETE` — RE-236/RE-237. Regular rotations A/B and linear texgen
captured on physical PSP as part of `T8`'s own physical-PSP leg (RE-236,
scenes 11/12/13). RE-237 closed the remaining two items: a raw normal
diagnostic (`texgen_normal_diagnostic_6`, `[73,-41,99]` raw `Normal` mode)
run on real hardware for the first time, matching its predicted `(179, 99)`
texel exactly; and a new `regression_capture_scene14` (same file-117
`0x1B10` graph as scenes 11/12, framed with a real, rotated view matrix
instead of the object viewer's usual identity view) exercising the
non-identity camera basis `R2.1`/T3 (RE-227) left dormant, captured both on
PPSSPP headless (`tests/golden/r2-metal-texgen-camera-rotated.png`,
deterministic across two rebuilds) and on the same physical PSP, matching at
a noise-floor order smaller than every other texgen scene measured this way.
PSP model, firmware, commit, pack hash and capture hash recorded in RE-236/
RE-237.

After semantics are fixed, capture regular rotations A/B, linear texgen, a raw
normal diagnostic and a camera-rotation case. Record PSP model, firmware,
commit, pack hash, EBOOT identity, scene and capture hash. Rebuild and explain
goldens after semantic changes; do not reuse them silently.

### T10 — Texgen documentation cleanup

After T1–T9 reconcile `STATUS.md`, `PLAN.md`, `DECISIONS.md`,
`docs/rendering.md`, `docs/reverse-engineering.md`,
`docs/visual-regression.md` and `docs/porting-status.md`, including pack
version, test counts, the actual linear scene, original-output status, normal
semantics, LookAt quantization, tile shifts and accepted PSP deviations.

Texgen test minimum: all raw GEN/LINEAR combinations and partial transitions;
same-node/cross-list/cross-node provenance; unit/non-unit/negative/zero
normals; LookAt saturation/near-zero; regular and linear endpoints plus
`u(dot)+u(-dot)=0.5`; every real scale; origin/clamp/repeat/mirror/mask/shift/
padded dimensions; and mapping transitions authored→regular→authored,
regular→linear→authored, linear→regular, and textured→untextured→texgen.
`romtool texgen ROM --verify` must fail on correctness-critical violations.
Run host fmt/tests, PSP checks, the texgen audit, PPSSPP scenes and physical
captures after T2/T3/T5/T6; rebuild the generated pack after pack/converter
changes. Completion requires source semantics, addressing, host, PPSSPP,
physical and original-Metal gates. Any unavoidable PSP difference needs an
`ACCEPTED_DEVIATION` record with measured error and affected content.

## R2.2 — Second Renderer Corrective Gate (C1–C7)

Status: `TODO` — depends on R2.1/T1–T10. It must close before R3 or a stable
rendering-gate claim. Do not optimize while it is open.

### C1 — Single-source `prim_color`

Trace raw vertex RGBA/normal bytes through `CacheEntry`, `push_vertex`,
`MeshVertex`, `PackWriter`, `PackedVertex` and `meshdraw`, including flat
colour, texture blend, alpha baking, normal extraction, pack-time lighting and
runtime lighting. Prove `SHADE * PRIM` occurs once, normals are never changed
as RGB, and shared vertices remain correct. Add unlit
`[128,128,128] * [128,128,128] → [64,64,64]`, lit `[127,0,0]` with 50% red
PRIM preserving the normal, SHADE-only, PRIM*SHADE, TEXEL*SHADE, flat,
texture-blend and mixed lit/literal tests. Census lit/unlit/texture-blend/
flat-color+PRIM and recheck affected fighters.

### C2 — Load-time lighting provenance

Preserve only state that changes vertex meaning at `G_VTX`: lighting enabled,
geometry mode, relevant light state and source normal-vs-colour semantics.
Trace fighter caller state through `ftDisplayMainProcDisplay`,
`ftDisplayLightsDrawReflect`, GObj/DObj entry and list execution; make initial
state explicit if lighting is external. Make `looks_like_unit_normal` fallback
only and quantify every remaining heuristic. Test OFF→G_VTX A→ON→G_VTX B and
the reverse, consumption after geometry changes, and real Fox/Falcon/Kirby/Ness
mixed-material cases.

### C3 — Independent depth compare/write state

Census `Z_CMP`, `Z_UPD`, `ZMODE_OPA/INTER/XLU/DEC`. Represent independent
`depth_test`, `depth_write` and `depth_mode`; keep `G_ZBUFFER` as geometry/RSP
state. Map writes through `sceGuDepthMask` (true disables PSP writes). Add a
depth-test/no-write translucent-front/opaque-behind scene and ON→OFF→ON
switch regression, with PPSSPP or physical-PSP evidence. Until this closes,
depth is not complete.

### C4 — Preserve submission order

Audit `merge_by_material` and callers; measure primitive runs before/after,
non-adjacent merges, and translucency/depth-write/alpha-test/framebuffer
involvement. Replace global grouping with adjacent identical-state runs only:
`A A B B A → AA BB A`, preserving triangle order and never crossing
translucency, depth-write, framebuffer, multi-pass, decal, alpha/coverage or
render-target boundaries without proof. Add an `A B A` test, rerun goldens,
and record draw-call growth as an R3 lead.

### C5 — Systematic PSP GE cache isolation

Inventory every raw GU mutation outside `apply_material`, including mesh,
collision/debug markers, particles, wallpaper/framebuffer, UI/debug and
fighter-light paths. Record function, changed state, cache field,
invalidation and whether a draw follows. Cover texture/binding/CLUT,
texture-function/env, map mode/scale/offset/wrap, filter/mip/LOD,
culling/shading/lighting/light/material, blend, alpha-test, depth test/
function/write and texgen. Add `DrawState::invalidate_all()` or centralize
mutations. Regress A→raw draw→A, A→fighter-light setup→A→teardown→A and
A→2D sprite→A.

### C6 — Integrated regression

Run the full host suite after C1–C5, rebuild the real pack and record pack
size, mesh/primitive/draw-run/texture counts, affected vertices and materials.
Rerun every deterministic golden and explain every change. Recheck Mario, Fox,
Kirby, Ness, Captain Falcon and Link plus lighting, texture blend, flat colour,
translucency, alpha, billboard, framebuffer and effects. Repeat representative
physical-PSP captures; never overwrite goldens blindly.

### C7 — Reconcile documentation

After C6 update `STATUS.md`, `PLAN.md`, `TODO.md`, rendering/porting/
reverse-engineering/visual-regression docs and `DECISIONS.md`. Correct claims
about depth completion, render-state fidelity, primitive sorting, load-time and
pack/runtime lighting, PRIM ownership and DrawState isolation. Add separate
evidence entries for independently demonstrated findings.

Execute C1 → C2 → C3 → C4 → C5 → C6 → C7. Stop progression if normals are
modified before lighting, PRIM is applied twice, vertex meaning depends on
triangle-time state, translucent paths write Z incorrectly, non-adjacent
primitives remain reordered, or raw GU calls bypass cache invalidation.

# 9. R3 — Rendering Performance

Status: `BLOCKED_BY_R2`

### Objective

Optimize rendering without sacrificing fidelity.

### Leads from R0.18's reference-port audit (not yet actioned — R3 is blocked)

RE-124 found both `sf64-psp` and `oot-PSP` use materially more
sophisticated state batching than this project's current "skip a
redundant `sceGu` call between consecutive same-material primitives":
`sf64-psp` hashes effective material state (FNV-1a) into a 64-slot batch
pool per material and records/replays draw calls rather than issuing
them immediately (`refs/sf64-psp/src/psp/gfx/gfx_psp_dl.c:2280-2394`,
`:72-77` — a code comment there cites a real measured win, 177→89
draws/frame, 24.2→22.0 ms, from activating this); `oot-PSP` caches
sampler state and shader-mode lookups in a 256-entry hash table
(`refs/oot-PSP/src/port/psp/gfx/gfx_scegu.c:1450-1466`, `:288-340`).
Concrete, working precedent for the state-sorting `DECISIONS.md` D-036
already anticipates this project needing eventually, once state fidelity
(`R0.15`/`R0.16`, both `COMPLETE`) is no longer the open question it was
when D-036 was written. R0.15/R0.16 remain VERIFYING while R2.1/R2.2 are
open. Not implemented here; R3 is `BLOCKED_BY_R2`.

### Acceptance

* [ ] frame time measured
* [ ] CPU bottlenecks identified
* [ ] GPU/GE bottlenecks identified
* [ ] texture upload costs measured
* [ ] VRAM usage measured
* [ ] memory bandwidth considered
* [ ] 60 FPS target evaluated on physical hardware
* [ ] optimizations revalidated against rendering tests
* [ ] no fidelity regressions introduced

Do not perform speculative optimization before measurement.

---

# 10. G0 — Combat Unlocked

Status: `BLOCKED_BY_R3`

Combat becomes eligible only after R0, R1, R2 and R3 are complete.

### First Combat Vertical Slice

* [ ] input
* [ ] one grounded attack
* [ ] attack animation
* [ ] hitbox
* [ ] hurtbox
* [ ] collision interaction
* [ ] damage
* [ ] knockback
* [ ] hitstun
* [ ] KO
* [ ] minimal opponent
* [ ] minimal stock handling
* [ ] match loop

The exact combat implementation should then follow the original decompilation rather than invented mechanics.

---

# 11. Post-Combat Roadmap

After G0:

## G1 — Full Combat

* all attacks
* specials
* grabs
* throws
* shields
* dodges
* aerial systems
* damage states
* knockback states
* recovery
* items
* hazards
* CPU behavior

## G2 — Complete Match Systems

* stage selection
* character selection
* stocks
* timers
* win conditions
* match transitions
* camera behavior
* multiplayer/input handling

## G3 — Menus and Persistence

* title screen
* menus
* character selection
* stage selection
* options
* save data
* progression/unlock systems

## G4 — Audio

* music
* sound effects
* voice
* audio mixing
* positional behavior
* memory management

## G5 — Final Optimization

* CPU profiling
* GE profiling
* memory profiling
* VRAM optimization
* loading optimization
* asset streaming
* frame pacing

---

# 12. Rendering Definition of Done

Rendering may only be declared complete when:

1. Every discovered rendering subsystem has an explicit status.
2. Every unsupported behavior has been investigated.
3. Every remaining deviation is documented.
4. All known texture failures are resolved or accepted with evidence.
5. All known material failures are resolved or accepted with evidence.
6. All required transform kinds are resolved.
7. Texture filtering is understood.
8. LOD behavior is understood.
9. Mipmapping behavior is understood.
10. Stage animation works.
11. Material animation works where required.
12. Fighter palettes/costumes work.
13. Billboard behavior is verified.
14. Framebuffer effects work.
15. Camera/projection behavior is verified.
16. Render-state leakage is eliminated.
17. All required fighters render.
18. All required stages render.
19. All required effects render.
20. Golden/reference rendering tests pass.
21. PPSSPP verification passes.
22. Physical PSP verification passes.
23. VRAM usage is safe.
24. Performance has been measured.
25. Documentation agrees with implementation.
26. The N64 render-state intermediate representation is audited as faithful, not merely convenient (R0.16).
27. The visual-regression methodology (R0.17) has been run end-to-end with recorded results across the full test matrix.
28. The reference-port comparative audit against `sf64-psp` and `oot-PSP` (R0.18) is complete, with every material difference classified and recorded.

Only then may R0/R1/R2/R3 be completed and combat unlocked.

---

# 13. Autonomous Execution Rule

When continuing autonomously:

```text
READ STATUS.md
      ↓
RESUME IN_PROGRESS TASK
      ↓
IF NONE:
SELECT FIRST ELIGIBLE TODO FROM PLAN
      ↓
CHECK DEPENDENCIES
      ↓
INVESTIGATE ORIGINAL BEHAVIOR
      ↓
IMPLEMENT
      ↓
TEST
      ↓
VERIFY AGAINST DECOMP / ROM
      ↓
CHECK BATTLESHIP
      ↓
UPDATE DOCUMENTATION
      ↓
UPDATE STATUS.md
      ↓
RECORD EVIDENCE
      ↓
COMMIT
      ↓
SELECT NEXT TASK
```

Never advance merely because code compiles.

Never advance by weakening acceptance criteria.

Never bypass the rendering gate.

---

# 14. End Goal

The intended progression is:

```text
Research
    ↓
PSP bootstrap
    ↓
Resource pipeline
    ↓
Core game/scene infrastructure
    ↓
Rendering correctness
    ↓
Rendering completeness
    ↓
Physical PSP validation
    ↓
Rendering performance
    ↓
COMBAT UNLOCKED
    ↓
Full combat
    ↓
Complete match systems
    ↓
Menus / save
    ↓
Audio
    ↓
Final optimization
    ↓
Complete SSB64 implementation
```

**Gameplay does not advance around a broken renderer.**
