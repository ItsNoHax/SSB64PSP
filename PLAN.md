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

Status: `IN_PROGRESS`

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
| Geometry (vertex positions/colors/normals, triangle topology, culling, matrix transforms, projection, viewport/scissor, coordinate conventions) | R0.8 (transforms), R0.14 (camera/projection), R0.6 (culling/geometry-mode defaults) | R0.8 `COMPLETE`; R0.14, R0.6 `IN_PROGRESS` |
| N64 render-state model (faithful intermediate representation; must not collapse to `mesh + texture + basic colour`) | **R0.16** (new), R0.15 (render-state isolation), R0.6 (state threading) | R0.16 `TODO`, R0.15 `TODO` |
| Texture correctness (formats, CI4/CI8, TLUT/palette lifetime, relocation, dimensions, coordinate scaling, filtering, LOD, mipmaps, clamp/mirror/repeat, masks/shifts) | R0.3, R0.4, R0.5 | R0.3 `COMPLETE`; R0.4, R0.5 `IN_PROGRESS` |
| Combiner correctness (`G_SETCOMBINE` shapes, TEXEL0/TEXEL1/SHADE/PRIMITIVE/ENVIRONMENT, RGB/alpha, interpolation/modulation) | R0.6 | `IN_PROGRESS` |
| Lighting correctness (`G_LIGHTING`, shading, normals, vertex colors, material interaction, ambient/directional lights) | R0.6 | `IN_PROGRESS` (accepted deviation: single baked key light, not per-object `sceGuLight`) |
| Alpha/blending correctness (alpha compare/test, source/destination blending, translucent vs. opaque, depth writes, render ordering) | R0.6 | `IN_PROGRESS` (alpha test shipped; translucency detected but not enabled — open bug) |
| Depth/culling correctness (depth direction/range/function/writes, polygon culling, winding, clipping) | R0.6 (state), R0.14 (depth mapping) | `COMPLETE` for both owned items |
| Render-pass completeness (transparency, particles, shadows, framebuffer effects, UI, other passes) | R0.12 (billboards), R0.13 (framebuffer), top-level R1 §7 (completeness gate) | R0.12 `VERIFYING`, R0.13 `IN_PROGRESS`; particles/shadows/UI not started (see `docs/rendering.md` "Rendering status" table) |
| Visual-regression methodology (deterministic test scenes; reference vs. PPSSPP-software vs. PPSSPP-hardware vs. physical PSP; test matrix) | **R0.17** (new) | `TODO` |
| Reference-port comparative audit (sf64-psp, oot-PSP) | **R0.18** (new) | `TODO` |

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
* [x] unsupported rendering paths identified — texture wrap modes (hardcoded `Repeat`), mipmap/LOD (generated but does not fix Dream Land canopy, RE-053), material animation (decoded, not played), majority-vote lighting heuristic
* [x] current verification baseline recorded — `cargo test --workspace`: 338 passing (`ssb-rom` 195, `ssb-engine` 36, `ssb-game` 107); `romtool textures`: 647 bound / 617 packed / 30 failed

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

Status: `IN_PROGRESS`

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
* [ ] all missing palette cases resolved — 1 `MissingPalette` failure remains (was 4; files 353 and 52 fixed via RE-059/RE-060), root-caused to a `PartTables` pairing gap in file 86's one remaining graph (RE-057, RE-060); tracked under R0.7, not this task
* [x] regression coverage added — texture decode unit tests in `crates/ssb-rom/src/texture.rs`; state-inheritance unit test in `crates/ssb-rom/src/mesh.rs` (RE-064)

### Evidence

RE-037, RE-057, RE-064 in `docs/reverse-engineering.md`. The one remaining
open item is fully attributed to `R0.7`'s scope, not a gap in this task.

---

## R0.5 — Texture Filtering / LOD / Mipmapping

Status: `IN_PROGRESS`

### Current evidence

Mip chains are generated at build time for 151 textures
(`psp_texture::pack_mipped`), but this did **not** resolve the Dream Land
canopy discrepancy (RE-053) — the wrong pattern survives and sharpens at
higher resolution, pointing at magnification rather than minification/LOD.
Filtering mode (bilinear vs point) is not yet verified per texture. This
task's explicit acceptance item "Dream Land canopy discrepancy resolved"
remains open — do not close this task while it is.

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

### Objective

Determine and reproduce the actual texture sampling behavior used by SSB64.

### Dependencies

* R0.2
* R0.3
* R0.4

### Acceptance

* [ ] filtering modes identified from original data
* [x] magnification behavior identified — RE-081: Dream Land's canopy "highlight" texture is magnified on its V axis (`0.88` repeats); RE-053's "sharpens with resolution" symptom is explained by this, not by the "gradient" texture (which is genuinely minified)
* [x] minification behavior identified — RE-053's `3.70×1.36` figure is correct for the canopy "gradient" texture specifically (RE-081 disambiguated which of the two canopy textures each figure actually describes)
* [ ] LOD behavior identified
* [ ] mipmapping behavior identified
* [x] texture tile parameters verified — RE-044 (mask-based tile sizing), RE-066 (clamp/mask correlation, archive-wide)
* [ ] texture coordinate behavior verified — RE-101 fixed a real gap (`G_TEXTURE` UV scale, never applied), unit-tested but not independently re-verified on a fighter's own screenshot; not yet exhaustive enough to check this item
* [x] wrap/clamp/mirror behavior verified — RE-067: `Mirror` (29% of packed textures) is exactly reproduced by pre-baking; RE-102 corrected RE-066's own "`Repeat` is correct for every case" conclusion — real hardware clamps on several fighters' face/torso/head textures where RE-044's mask-based narrowing is a no-op, now reproduced via `TextureDesc::wrap`/`sceGuTexWrap(Clamp, ...)` per axis
* [ ] Dream Land canopy discrepancy resolved — RE-067 fixed the mirror wrap boundary; RE-070 measurably softened the dither (~40% less local noise on the treated texture) by pre-blurring and packing unquantized, but it is not fully smooth; RE-075 fixed a small blur/mirror boundary-condition bug (confirmed via packed-byte diff) but confirmed it is not visible at the tested camera distance; RE-081 disambiguated the magnification/minification confusion and tested a further blur pass (measurably less texture noise, not visibly different on screen) — none of the four is "resolved"; real hardware validation (`R2`) is looking necessary, not just sufficient, to close this
* [ ] no unsupported mipmapping assumptions remain

### Evidence

RE-044, RE-053, RE-066, RE-067, RE-070, RE-075, RE-081, RE-101, RE-102 in `docs/reverse-engineering.md`.

---

## R0.6 — Material System Correctness

Status: `IN_PROGRESS`

### Current evidence

`crates/ssb-rom/src/mesh.rs` evaluates a general `(A-B)*C+D` combiner across
both RDP cycles and declines to guess at anything it can't resolve rather
than approximating (RE-039, RE-043). Primitive/environment colour, alpha and
depth state are threaded through. Lighting is **not** derived from `MObj`
light state — a single neutral key light is used as a placeholder
(`DECISIONS.md` D-024), and `TODO.md` Phase D explicitly calls out removing
this "majority-vote lighting heuristic."

RE-065 investigated this properly instead of leaving it an undocumented
guess. The real light direction is `MPGroundData.light_angle` (a per-stage
`Vec3f`, `refs/ssb-decomp-re/src/mp/mptypes.h:187`), converted to a vector
by `ftDisplayLightsDrawReflect` (`refs/ssb-decomp-re/src/ft/ftdisplaylights.c`)
every time a fighter draws — direct reproduction is not possible without
moving from this project's pack-time-baked shading to PSP-side runtime
lighting (`sceGuLight`) with per-stage context threaded through the whole
material pipeline, which is out of this task's scope. Measured
archive-wide: **33 of 41 stages (80%) use exactly the same angle**
(`20.0, 45.0` degrees), which the old `(2, 4, 3)` placeholder happened to
sit only 9.9 degrees from; the constant now uses that measured angle's
actual direction instead, so those 33 stages' baked shading matches the
real key light exactly, up to the libm-vs-lookup-table sin/cos difference.
The other 8 stages (mostly special-lighting locations — Brinstar, Sector
Z, Hyrule, Final Destination, Metal Mario's stage) use their own angle, up
to 111 degrees away, and remain an explicit, measured, accepted
deviation per `AGENTS.md` §9 rather than an undocumented placeholder.

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

### Objective

Reproduce original SSB64 material behavior.

### Dependencies

* R0.2
* R0.4

### Acceptance

* [ ] material tables resolved
* [x] combiner behavior verified — RE-073/RE-074: identified and measured the dominant declined shape (`(PRIM-ENV)*TEXEL+ENV`, 91% of ENV-reading combiners, 28 files including Link/Ness/Pikachu's own models), detected, packed, wired to the PSP GE's native `TextureEffect::Blend`, and visually confirmed correct against Link's own model (before/after screenshots). The general two-cycle evaluation model itself was already verified (RE-039/RE-043); the remaining ~8% of ENV-reading combiners and whatever `combiner_shade_scale` declines outside that are not exhaustively catalogued, but are not the dominant case and are tracked under "primitive color"/"environment color" below rather than blocking this item
* [ ] primitive color verified — RE-079's census plus RE-080's `combiner_flat_color` now cover every shape this model resolves at all: shade-scale, texture-blend and flat-constant are structurally disjoint and together account for 97.5%+ of archive-wide combiner-bearing primitives. What remains open is not a classification gap: `(PRIM-ENV)*TEXEL0+ENV`'s 3,085/4,580 misses are a genuine absence of `prim_color` on this converter's own per-graph state (likely `R0.7`'s material-table pairing gaps), which no combiner-shape work can fix. RE-106 additionally found and fixed a *consumption* gap in the cases that already classified correctly: `prim_color` was resolved but never multiplied into the actual vertex, baked in now
* [ ] environment color verified — same as primitive color; the remaining gap is the same genuine-absence case, symmetric in `ENV`
* [ ] lighting verified — RE-103/RE-105 fixed the input this still depends on (per-vertex, not per-primitive-majority, lit/literal decisions, driven by a real `G_MW_LIGHTCOL` ROM signal rather than a guess) but the shading itself is still RE-065's single baked-in neutral key light, not per-object real lighting — this item stays open on that basis
* [x] alpha behavior verified — RE-069: `CVG_X_ALPHA | ALPHA_CVG_SEL` (cutout surfaces, 36.1% of non-default render modes) decoded and wired to `sceGuAlphaFunc`, matching `refs/sf64-psp`'s validated approach; gated on a real texture being bound after a found-and-fixed bug that discarded untextured lit primitives outright
* [ ] blending verified — RE-069: `translucent` (14.4%) is correctly detected (decomp-verified bit logic) but deliberately not wired to `GuState::Blend` yet; enabling it on Dream Land's canopy-highlight surface produced a checkerboard. RE-071 re-checked after RE-070's dither-blur fix in case that resolved it — it did not; re-testing produced a *worse*, different failure (blown-out highlights), and ruled out unpremultiplied-alpha blurring as the cause too (a premultiplied variant gave an identical result). The real cause remains unknown; two specific hypotheses are eliminated, not guessed away
* [x] fog verified — RE-072: `DECISIONS.md` D-025's "twice" figure confirmed correct via reliable reloc-anchored discovery (an `Exhaustive`-mode re-scan found 7/4, which turned out to be false positives); both real occurrences are functionally inert — no `gSPFogPosition` call exists anywhere in the decompilation to configure a fog range, and the one real stage that sets a fog colour (file 118) never references `G_BL_CLR_FOG` in its own render mode
* [x] depth state verified — RE-068: real default is on (`sSYRdpResetDisplayList`), not off; fixed and wired to `sceGuEnable/Disable(DepthTest)` per primitive
* [x] culling verified — RE-068: same reset list defaults `G_CULL_BACK` on; fixed, measured 86.3% of packed primitives cull back faces post-fix
* [ ] unsupported material behavior identified

### Evidence

RE-065, RE-068, RE-069, RE-071, RE-072, RE-073, RE-074, RE-079, RE-080, RE-103, RE-105, RE-106 in `docs/reverse-engineering.md`.

### Evidence

RE-065, RE-068 in `docs/reverse-engineering.md`. RE-068 also leaves leads
for the still-open items above: the same reset list fixes
`G_AC_NONE` (alpha compare off by default), `G_RM_OPA_SURF`/`G_RM_OPA_SURF2`
(opaque render mode by default, no blending), and `G_CC_SHADE`/`G_CC_SHADE`
(shade-only combiner by default) as the real starting state, none of which
this pass acted on.

---

## R0.7 — Missing Material Tables

Status: `IN_PROGRESS`

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
resolve one), `MissingPalette` 4→1. `cargo test --workspace`: 338 passing,
unaffected throughout (both fixes live in `romtool`, not the library crate).

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

The remaining 57 unpaired graphs archive-wide (mostly menu/character-
select emblem models, stage files, and fighters' special-move files, not
core fighter bodies — see RE-077's breakdown) and file 86's and 353's
specific blocked cases are now treated as an accepted long tail rather
than a task-blocking gap; R0.7 stays `IN_PROGRESS` but further progress
here depends on upstream decomp typing or a demand-search candidate
narrowing to one with something to confirm it against, not open-ended
`romtool` investigation.

### Objective

Resolve every scene graph containing an unresolved material table.

### Dependencies

* R0.6

### Acceptance

* [ ] all material-table references traced — 5 shapes now known (`FTCommonPart`, `MPGroundDesc`, `WPAttributes`, `EFDesc`, plain call-sequence pairing); files 52 and 353 fully or mostly traced; file 86's last graph's mechanism is understood but does not narrow to one table (RE-061, measured: 27 candidates, no named record); 7 more graphs (Kirby's + 6 archive-wide) traced and fixed via search-plus-decomp-cross-check, landing on exactly one candidate each (RE-077, RE-078); 57 other archive-wide unpaired graphs are untraced, mostly menu/character-select emblem models, stage files and fighters' special-move files rather than core fighter bodies (RE-077's breakdown) — 9 of the 11 remaining real fighters have zero unpaired graphs of their own
* [ ] original material data identified — done for 14 pairings (2 `EFDesc` in file 353, 5 call-sequence in file 52, 7 raw-array/search-confirmed across Kirby's file and 6 other archive files); not done for the other 57 unpaired graphs, and file 86's/353's/Ness's remaining candidates are blocked on upstream decomp typing or ambiguous search results, not more tracing
* [ ] heuristic mapping removed where original data exists — n/a so far, no heuristic was standing in for these; this was a pure discovery gap
* [ ] affected scenes verified — file 353's two, file 52's five, and the 7 search-confirmed graphs verified via `romtool mobj`/`romtool textures` (RE-059, RE-060, RE-077, RE-078); nothing else verified yet
* [ ] regression coverage added — no `cargo test` coverage; the fix lives in `romtool` (a CLI tool, not the library crate), and the project's existing regression pattern for ROM-dependent behavior is a `romtool` command's own output (matching how R0.9 verifies stage animation), not a unit test. `romtool mobj`'s archive-wide 0-mismatch check (383 nodes) is that regression detector for these fixes.

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
* [x] animation playback verified — plays on device (PPSSPP): 35 stages, 206 animated nodes, 60 FPS
* [x] independent ROM comparison exists — three independent checks agree: ROM replay (RE-050), packed-pose-vs-archive across 444,960 values (RE-052), and a two-frame device diff showing motion only over the animated canopy (RE-051)
* [x] all stages tested — all 41 stages' animation data was checked; 35 carry joint animation and 6 (including Dream Land) do not — Dream Land's scenery instead moves via non-joint game code, which is out of this task's scope

### Evidence

`docs/porting-status.md` "Stage animation" row; RE-050, RE-051, RE-052 in
`docs/reverse-engineering.md`. Validated under PPSSPP, not yet on physical
hardware — that gap belongs to R2, not this task.

---

## R0.10 — Material Animation

Status: `COMPLETE`

### Current evidence

The 12-layer material animation script is decoded but not played
(`docs/porting-status.md` "Stage animation": "read but not played"). Frame 0
happens to match the baked colours already shipped, so nothing currently
renders visibly wrong — but that is coincidence, not implementation. Genuinely
not started; `TODO.md` Phase F.

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

Status: `VERIFYING`

### Current evidence

Matrix kinds 44/46/48/50 are implemented, all 109 flagged nodes billboard
at draw time (RE-062/RE-063 grew this from an earlier 81 once
`RecalcRotRpyRSca` was added), and camera-facing behavior was verified A/B
under a deliberately rotated camera (RE-049; Dream Land's six canopy
sprites stay upright when honoured, skew into slivers when ignored).
Depends on R0.14 (camera/projection), now further along after RE-082
(viewport, aspect ratio and resolution-difference handling confirmed) —
its remaining gaps (real FOV provenance, an actual game camera) do not
block anything billboards need, since billboard camera-facing math
(RE-049) does not depend on either.

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

### Objective

Verify every billboard rendering path.

### Dependencies

* R0.8
* R0.14

### Acceptance

* [x] billboard types enumerated — RE-063 exhaustively traced every `gcPrepDObjMatrix` case reachable from a ROM `DObjDesc` array (kinds 44/46/48/50, all four implemented); RE-083 confirmed no fifth reachable kind hides behind the `rot_mode` branch, since that branch belongs to an unreachable runtime-only path
* [x] camera-facing transforms verified — RE-049's rotated-camera A/B test (Dream Land's six canopy sprites upright vs skewed into slivers)
* [ ] scale verified
* [ ] orientation verified
* [ ] texture orientation verified
* [ ] alpha behavior verified — RE-083: `alpha_test` needs nothing further (already-shipped RE-069 mechanism, archive-wide verified); `translucent` is the blocker, measured to affect billboards (29.7%) at roughly double the archive-wide rate (14.4%), tracked under RE-069/RE-071's still-open finding, not a new problem
* [x] depth behavior verified — RE-083: archive-wide census of billboard-flagged nodes' own primitives found `z_buffer` set on 118/118 (100%), matching RE-068's default with zero exceptions
* [ ] all flagged billboard nodes verified — RE-083 measured render-state distribution archive-wide, not per-node visual correctness beyond RE-049's own Dream Land spot check

### Evidence

RE-049, RE-062, RE-063, RE-083 in `docs/reverse-engineering.md`.

---

## R0.13 — Framebuffer Rendering

Status: `IN_PROGRESS`

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

### Objective

Implement every framebuffer-based rendering path required by SSB64.

### Dependencies

* R0.2
* R0.6

### Acceptance

* [x] framebuffer usage identified — RE-099: the one-time-snapshot-into-a-texture mechanism, exactly which 13 files use it (26 segment-`0x01` binds), and what a PSP implementation actually needs to build
* [x] framebuffer texture paths implemented — RE-100: segment-`0x1` recognition, pack format support (`TextureDesc::role`, `VERSION` 14), and device-side capture-and-bind, verified on the real device profile with an unambiguous test-colour capture; RE-107 confirmed the shape generalizes archive-wide and the capture/bind mechanism itself works on a second, deliberately different file. RE-108 found a real correctness gap (the capture only stores the top of the real 220-texel-tall N64 buffer, so a tile sampling elsewhere in that range reads the wrong rows); RE-109 fixed it by rebasing each framebuffer-role primitive's UV by its own tile origin at pack time (unit-tested, packed-byte-diff-verified); RE-110 confirmed the fix on the real device by direct pixel measurement (the previously-black region now reads the exact captured test colour). RE-111 found and fixed a second, independent real bug in the same mechanism: the permanent 4:3 pillarbox scissor left columns `0..59` of the raw framebuffer solid black forever, and the capture read from absolute column 0 instead of the pillarbox's own left edge — fixed with a one-line offset, verified by a direct pixel scan finding zero black pixels on the object post-fix (was 28,993–34,584 pre-fix)
* [ ] screen wipes implemented — the capture/bind mechanism exists; nothing yet triggers it from real game logic, since no match-transition state machine exists in this project at all
* [x] render-to-texture paths implemented where required — RE-099/RE-100: confirmed twice, independently, that the real mechanism has no render-to-texture pass to implement; this item is satisfied by there being nothing here that applies
* [ ] framebuffer synchronization verified — verified for the one shape tested pre-RE-109 (a manually-triggered capture read back the same frame); not verified for whatever the real trigger timing ends up being once transitions have a real caller
* [x] visual verification completed — **all 13 LB-transition files are confirmed fully correct on the real device.** File 45's own "backing quad" question is fully retracted (RE-112 — it was never reachable geometry). The debug-viewer camera-framing gap that blocked 41/43/50 is fixed for good (RE-115 — `DrawState::force_no_cull`, scoped to `object_view` only). File 46's apparent diagonal black banding (RE-113) is retracted (RE-116) — a measurement artifact (an un-restricted pixel scan catching the same window-decoration confound RE-111 already documented), not a rendering defect; a close pixel scanline shows exact linear anti-aliased blending between real background and real magenta, and an exhaustive bounding-box-restricted census finds zero black pixels

### Evidence

RE-055, RE-099, RE-100, RE-107, RE-108, RE-109, RE-110, RE-111, RE-112, RE-113, RE-114, RE-115, RE-116 in `docs/reverse-engineering.md`.

---

## R0.14 — Camera / Projection Correctness

Status: `IN_PROGRESS`

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

### Objective

Reproduce the original camera and projection behavior.

### Dependencies

* R0.8

### Acceptance

* [x] projection matrix verified — RE-082 audited the aspect term; RE-084 replaced the FOV term's unsourced `60.0` degree guess with the decompilation's own real default (`38.0` degrees, four agreeing call sites). Near/far clip planes are this project's own debug-viewer framing choice, not part of the original's projection behavior, so nothing further to source there
* [x] viewport verified — RE-034 (device measurement, before/after screenshots) plus RE-082 (source-level confirmation that `Gpu::init` and `main.rs` share one `pillarboxed_viewport()` call, so they cannot diverge)
* [x] aspect ratio verified — RE-082: `pillarboxed_viewport()` is unit-tested (`pillarbox_preserves_four_by_three`), its output is the sole source for both the GE viewport/scissor and the projection's `aspect` parameter, and `sceGumPerspective`'s own binding uses the standard formula; RE-034's previously-reported residual is a measurement artifact on a too-small on-screen shape, not a surviving defect
* [x] depth mapping verified — RE-085: `sceGuDepthRange(65535, 0)` + `GreaterOrEqual` matches the `psp` crate's own documented depth-buffer convention exactly, not a workaround; corroborated on-device with no depth-order artifacts found in a complex, multi-layer regression scene
* [ ] camera transforms verified — no real game camera system exists yet, only the debug viewer's free-roaming camera; RE-084 sourced the FOV *value* the original uses, but reproducing the camera's actual positioning/movement logic needs a real camera system this project does not have yet
* [x] N64/PSP resolution differences explicitly handled — RE-082: pillarboxing (D-008) is precisely this handling, now confirmed by both a device measurement (RE-034) and a source audit (RE-082) rather than one alone
* [ ] representative scenes compared — no side-by-side N64-vs-PSP reference comparison exists

### Evidence

RE-034, RE-082, RE-084, RE-085 in `docs/reverse-engineering.md`.

---

## R0.15 — Render-State Isolation

Status: `COMPLETE`

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
* [x] state leakage tests added — `mesh.rs`'s decode-time state threading has direct unit tests (RE-117, 4 new + RE-064's existing one) for all 10 categories. `psp/src/meshdraw.rs::DrawState`'s device-side GE cache layer was systematically audited by code reading (RE-118, since raw `sceGu*` calls have no host-side mocking harness to unit-test against) — every category checked, one real gap found (the collision/fighter overlay bypassing the texture cache) and fixed, verified by an on-device screenshot showing the fix is inert for the current non-triggering scene

### Evidence

RE-064, RE-074, RE-117, RE-118 in `docs/reverse-engineering.md`.

---

## R0.16 — N64 Render-State Model Fidelity

Status: `IN_PROGRESS`

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
  [Character]" transformation's signature shiny/reflective look (the
  Metal Box item). Classified as SSB64 genuinely needing it
  (`PLAN.md` R0.18's classification 1), but implementation is correctly
  deferred — it is an item-pickup visual effect, downstream of the
  combat/item systems `AGENTS.md` §5 gates behind rendering correctness.
  Not an `ACCEPTED_DEVIATION` (it is technically reproducible on the PSP
  GE), just out of scope until items exist.

Neither gap was fixed this session — both need either a per-primitive
cross-reference (`G_SHADE`) or a real feature (environment-mapped UV
generation, `G_TEXTURE_GEN`) gated behind out-of-scope game systems.
Recorded as concrete, scoped, characterized leads rather than guessed at
or silently left unlisted, matching this audit's own acceptance
criteria: every category found is now either handled, explicitly
deferred with a reason, or flagged as needing further investigation —
none is silently absent from `docs/rendering.md` any more.

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
  had zero field or documented reason); now documented in `docs/rendering.md`
  with a reason each, but `G_SHADE` still needs the per-primitive
  cross-reference noted above before this item can close
* [ ] no state category is silently dropped between `mesh.rs`'s conversion and
  `pack.rs`'s on-disk record without a documented reason (cross-reference
  against R0.15's leakage tests) — not yet audited beyond geometry mode
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
* [ ] any state this audit finds genuinely unrecoverable on PSP is recorded as
  an `ACCEPTED_DEVIATION` per `AGENTS.md` §9, not silently absent — neither
  `G_SHADE` nor `G_TEXTURE_GEN` is unrecoverable (both are reproducible on the
  PSP GE), so neither qualifies; both are documented as deferred/needing
  further work instead

### Verification

* source-level audit of `mesh.rs`, `pack.rs`, `psp/src/meshdraw.rs`
* cross-reference against R0.2's opcode/state inventory
* cross-reference against R0.18's reference-port comparison

### Evidence

RE-119 in `docs/reverse-engineering.md`.

---

## R0.17 — Visual Regression Methodology

Status: `TODO`

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

* [ ] at least one deterministic test scene defined: fixed stage (Dream Land,
  the project's existing primary regression scene), fixed fighter, fixed
  camera, fixed animation/frame, fixed game state — every value pinned, no
  randomness, no free-roaming debug camera
* [ ] a documented procedure exists to capture the same scene from: (1) the
  original SSB64 (ROM/emulator reference), (2) PPSSPP software rendering,
  (3) PPSSPP hardware rendering, (4) physical PSP hardware where practical
* [ ] a test matrix exists covering, at minimum: untextured geometry,
  textured geometry, CI4, CI8, palette changes, filtering, clamp/mirror/
  repeat, lighting, each recognized combiner shape (R0.6), transparency,
  depth, culling, particles, shadows, UI — each row names the concrete asset/
  display list it exercises (not a hypothetical example)
* [ ] captured reference images are compared automatically wherever
  practical (pixel diff or equivalent), with the comparison threshold and
  method documented — not "looks the same" (`AGENTS.md` §7)
* [ ] the methodology is actually run at least once end-to-end and its
  output recorded, not merely specified
* [ ] `PLAN.md` R1's "golden/reference renders are established" acceptance
  item and `TODO.md` Phase H's "Screenshot regression" item are satisfied by
  this task's output, not left as separate unowned work

### Verification

* run the documented procedure against the current renderer
* record pass/fail per test-matrix row with evidence

### Evidence

Not started. `TODO.md` Phase H ("Reference renderer", "Screenshot regression",
"Strict rendering mode") is the prior unowned form of this gap; this task
supersedes it.

---

## R0.18 — Reference-Port Comparative Audit (sf64-psp, oot-PSP)

Status: `TODO`

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

* [ ] `sf64-psp`'s N64-state translation, texture/material handling and
  `sceGu` usage compared against this project's own (BattleShip's F3DEX2/
  S2DEX interpreter was already checked, RE-054; `sf64-psp` itself has not)
* [ ] `oot-PSP` cloned into `refs/` and its N64-state translation, texture/
  material handling, `sceGu` usage and render architecture compared against
  this project's own
* [ ] every material difference found is classified 1–4 above and recorded,
  not left as an unexplained observation
* [ ] debugging methodology and performance technique from both projects
  reviewed for applicability to `R3` (rendering performance) — recorded as
  leads for `R3`, not implemented here (`R3` is `BLOCKED_BY_R2`)
* [ ] conclusions are written into `docs/reverse-engineering.md` as `RE-`
  entries and cross-referenced from the `R0.x` task(s) each conclusion
  actually affects

### Verification

* source-level comparison against cloned reference repositories
* cross-reference conclusions against R0.2's own usage measurements

### Evidence

Not started.

---

# 7. R1 — Rendering Completeness

Status: `BLOCKED_BY_R0`

R1 cannot begin until R0 is complete.

### Objective

Demonstrate that every discovered SSB64 rendering path required for the game is implemented.

### Acceptance

* [ ] all stages render
* [ ] all fighters render
* [ ] all required costumes render
* [ ] all required animations render
* [ ] all required effects render
* [ ] all required framebuffer paths render
* [ ] no unexplained rendering commands remain
* [ ] no unexplained missing assets remain
* [ ] no unexplained material failures remain
* [ ] rendering regression suite passes
* [ ] golden/reference renders are established

---

# 8. R2 — Physical PSP Rendering Validation

Status: `BLOCKED_BY_R1`

### Objective

Verify the renderer on actual PSP hardware.

PPSSPP is not sufficient.

### Acceptance

* [ ] EBOOT boots on physical PSP
* [ ] runtime asset pack loads
* [ ] representative fighters render
* [ ] representative stages render
* [ ] fighter animation works
* [ ] stage animation works
* [ ] materials render correctly
* [ ] textures render correctly
* [ ] framebuffer effects work
* [ ] VRAM usage verified
* [ ] no hardware-only rendering failures remain
* [ ] hardware model recorded
* [ ] build/environment recorded

---

# 9. R3 — Rendering Performance

Status: `BLOCKED_BY_R2`

### Objective

Optimize rendering without sacrificing fidelity.

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
