# Project Status

**Last updated:** 2026-09-10 (RE-214 texgen correctness recovery)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `R2 — broaden physical-PSP golden coverage` (the texgen
fidelity follow-up is finished; see "Last completed task")

**Status:** `IN_PROGRESS`

**Last completed:** RE-214 — texgen correctness recovery. RE-213's first
`G_TEXTURE_GEN` implementation proved only that GE environment mapping
*executes* safely on hardware; it did not establish coordinate fidelity, and
four things were wrong or unproven. All four are now resolved and measured.
Full account: `docs/reverse-engineering.md` RE-214.

1. **Raw geometry bits.** `G_TEXTURE_GEN` and `G_TEXTURE_GEN_LINEAR` were
   collapsed into one enum, and the linear bit could enable generation by
   itself. The walker now keeps the raw geometry-mode word and derives the
   mode from it; `TextureGen::Sphere` is renamed `Regular`. Measured impact on
   this ROM: none — the raw pair `(GEN=0, LINEAR=1)` never occurs.
2. **Vertex-load census.** New `romtool texgen` walks every graph's planned
   draw order and every discovered root list with an independent walker,
   replaying `MObj` state. 3,012 texgen triangles across 16 files; **zero**
   load a vertex under a different mode or `G_TEXTURE` scale than the draw.
   Primitive-level state is a measured invariant (D-039).
3. **Scale and tile origin.** `PrimDesc` now carries `texgen_scale_s`/`_t` and
   `texgen_origin_s`/`_t` (pack `VERSION` 27, `PrimDesc` 52 -> 60 bytes). All
   five real `G_TEXTURE` scales make the generated span exactly one period of
   their own tile, which independently corroborates the formula.
4. **The generator.** GE `EnvironmentMap` ignores `sceGuTexScale`/
   `sceGuTexOffset` — measured, by installing a 64x-larger factor and getting a
   byte-identical capture — so it cannot carry the source scale. Coordinates
   are generated through the GE's texture-**matrix** generator instead, from
   the normalised vertex normal, against the camera's world right/up basis
   (which is exactly what `syMatrixLookAtReflectF` writes into the RSP's
   look-at, in world space because SSB64 puts the view matrix in the
   *projection* matrix). No GE light is involved any more (D-038).

Also: both RDP alpha gates now resolve onto the GE's one alpha-test unit in
`pack::alpha_gate` with host regressions for every combination including the
two overlap cases; mip behaviour revalidated (level zero only, constant LOD);
and the nine golden scenes RE-213's mip change had left stale were refreshed.

**Verification.** `cargo test -p ssb-rom` 366 pass, `cargo test --workspace`
524 pass, `cargo fmt --check` clean in workspace and `psp/`. Pack rebuilt,
SHA-256 `295b62dc...`. PPSSPP: scene 11 two captures byte-identical; new
scene 12 (same graph, quarter turn) differs by RMSE 0.058, so the reflection
demonstrably responds to rotation; Dream Land byte-identical to RE-213's own
level-zero capture `08cc25cc...`. Physical PSP (Slim, 6.61 ARK/Infinity,
PSPLink 3.2.1): both texgen scenes render with `exlist` empty, captures
`~/ppsspp-test/re214/psp-hw-scene11.bmp` (`5cccb937...`) and
`psp-hw-scene12.bmp` (`4f66d8cc...`); they agree with their PPSSPP goldens
*better* than the long-accepted non-texgen Dream Land baseline does (2,652
and 8,443 strong-diff pixels versus 11,668).

**Commits:** `0248375` (raw geometry state, census, pack scale/origin, GE
basis, alpha gates), `502f760` (texture-matrix generator, scene 12),
`57ee696` (golden refresh, `romtool texgen --pack`).

**Documentation updated:** `docs/reverse-engineering.md` (RE-214),
`docs/rendering.md` (texgen section rewritten, new status row, alpha and mip
rows corrected), `docs/porting-status.md`, `PLAN.md` R2, `DECISIONS.md`
(D-038, D-039), this file.

**Remaining deviations (both recorded in `PLAN.md` R2's acceptance list):**

* **No original-N64 comparison for texgen output.** The only Metal content
  this port can show is `StageMetalFile2` (Meta Crystal), reachable in SSB64
  only through 1P mode stage 8 — VS Mode cannot select it. RE-151's scripted
  original-ROM harness (a temporary out-of-Git Mupen64Plus input plugin plus a
  Python Core API driver) no longer exists on disk; only its screenshots under
  `~/ppsspp-test/re151/` remain. Rebuilding it and scripting a route to stage
  8 is the prerequisite. Ordinary texgen is therefore `VERIFYING`, not
  `COMPLETE`.
* **`G_TEXTURE_GEN_LINEAR` still draws through the ordinary mapping.** 269
  triangles archive-wide, 24 of them in scene 11. The GE's generated
  coordinate is affine in the dot product; `acos(-dot)/(2*pi)` is not. Two
  candidate implementations are written up in RE-214 §10 (CPU/VFPU per-vertex
  generation into a scratch buffer, or a pack-time per-axis inverse-curve
  texture pre-warp). Measure both before choosing; do not start one blind.

**Dependencies:** R0.5 and R1 complete. R2's remaining hardware checklist rows
are live analog-stick input (needs a human operator), exhaustive
no-failures-remain coverage, and the two texgen rows above.

**Relevant files:** `PLAN.md` R2; `docs/reverse-engineering.md` RE-201–214;
`docs/psplink.md`; `docs/visual-regression.md`; `tests/golden/*.png`;
`tools/compare-screenshot.sh`; `psp/src/main.rs`; `psp/src/meshdraw.rs`
(`apply_texture_mapping`, `texgen_object_basis`, `note_model_matrix`);
`psp/Cargo.toml`; `crates/ssb-rom/src/mesh.rs` (`State::geometry_mode`,
`TextureGen`); `crates/ssb-rom/src/pack.rs` (`PrimDesc`, `alpha_gate`);
`crates/ssb-rom/src/psp_texture.rs` (mapping math);
`tools/romtool/src/main.rs` (`texgen`, and `FIGHTER_COSTUME_COUNTS` for
fighter name to model-graph file id).

**First checks:** physical PSP hardware was present and PSPLink-reachable this
session (`lsusb` shows `054c:01c9`; `pspsh -e ver` reports `PSPLink v3.2.1`
once `usbhostfs_pc -v "$PWD"` runs). Re-check at the start of the next
session. Remember RE-212's finding: `pspsh -e reset` after **any** `kill`,
before the next `ldstart`, even when `exlist`/`thlist` look clean.

**Acceptance:** `PLAN.md` R2.

**Next:** the roadmap's own next item is the **Yoshi's Island `G_SHADE`
original-output investigation** (RE-120's two live primitives). It needs the
same rebuilt original-ROM harness the texgen comparison above does, so doing
that harness work once unblocks both — build it first, then use it for
`G_SHADE` and for the texgen comparison in the same session. If the harness
turns out to be infeasible, the next eligible work is more
`regression_capture_sceneN` coverage: 6 of 12 playable fighters (Samus 320,
Luigi 323, Link 324, Jigglypuff 330, Yoshi 338, Pikachu 341 — model file ids
from `FIGHTER_COSTUME_COUNTS`) and 39 of 41 stages are still
hardware-untested. Find a model graph with `romtool scene --file <id> --list`,
add a feature following scene 10's structure, and repeat the
PPSSPP-then-hardware procedure.

**RE-211 rendering-gap audit:** current decomp/runtime comparison found that
R0.10's `COMPLETE` claim is premature. Runtime supports the 33 packed
palette-cycling material scripts, but not the decomp's stage `TextureIDCurrent`
and UV material tracks; 200/441 fighter costume scripts also carry an ignored
`PaletteID` track. The decomp-confirmed renderer gaps are fighter shadows,
general SObj/UI rendering, and original GObj/display-link scheduling for
multi-pass content. Rare/two-cycle combiner formulas remain a bounded fidelity
gap; the combined alpha gates listed there are now closed by RE-214. Current
ROM census corrected stale docs: 134/134 material graphs paired, 475 matching
nodes, zero mismatches; texture failures are only the 26 runtime framebuffer
references. See RE-211.

## Current state

- R0.5: `COMPLETE`; RE-201's PSPLink capture resolves its physical Dream Land
  canopy comparison.
- R1: `COMPLETE`; every acceptance bullet is checked through RE-200 and its
  R0.5 prerequisite is now satisfied.
- R2: `IN_PROGRESS`; twelve golden regression scenes (Dream Land/Mario,
  `MVOpeningRoom`, `StageSectorFile2`, `CatchSwirl`, Saffron City stage
  animation, Fox, Captain Falcon, Kirby, Ness, Donkey Kong, and RE-214's two
  `StageMetalFile2` texgen rotations) boot, pack loads, stage/fighter/
  material/texture content matches PPSSPP goldens, and no hardware exception
  remains across any of them (RE-203, RE-205, RE-207, RE-208, RE-209, RE-210,
  RE-212, RE-214). The real framebuffer-effect `SObj` sprite path, VRAM usage,
  and stage animation are hardware-verified too (RE-204, RE-205). Six fighters
  besides Mario are hardware-verified. RE-212 found that a bare `kill` of a
  prior PSPLink module can silently break the next module's asset-pack load
  with no symptom `exlist`/`thlist` catch — `docs/psplink.md` now recommends
  `reset` after every `kill`. RE-214 refreshed the nine goldens RE-213's mip
  change had left stale. Exhaustive no-failures-remain coverage (6 of 12
  fighters, 39 of 41 stages still untested), live analog-stick input, the
  texgen original-output comparison and exact `G_TEXTURE_GEN_LINEAR` remain
  open.
- Movement core: dash and run velocities now follow fighter facing, including
  after a left turn; regression coverage added for left-facing run/dash state.
- Effects: RE-172–189 cover manager descriptors, transforms, material/
  texture/colour animation, LBParticle decoding/packing, drawing, exhaustive
  audits, spawn-tree execution, `LBGenerator`, and a real manager-effect
  spawn event wired into the PSP runtime and PPSSPP-verified. `PLAN.md`
  R1's "all required effects render" acceptance item is checked off.
- Framebuffer paths: RE-190–193 cover the exhaustive census, the
  wallpaper-capture mechanism, and a minimal real `SObj` 2D-sprite render
  path (`Gpu::draw_wallpaper_sprite`) that draws the capture back through a
  real GE texture bind, device-verified bounded and correctly dimmed.
  `PLAN.md` R1's "all required framebuffer paths render" acceptance item is
  checked off. Only the real 1P-mode/results-screen G2 trigger remains
  unbuilt — accepted as out of R1 scope, the same split RE-149 already used
  to close R0.13.
- Golden coverage: RE-200's graph census identifies 119 texture-blend, 12
  flat-colour, and 252 classified translucent graph-backed primitives.
  Scene 3 (file 109 graph `0x44C8`) covers texture blend, translucency, and
  clean clamp; scene 4 (file 84 graph `0x2760`) covers flat colour.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-214 — Texgen correctness recovery: raw geometry bits, vertex-load
census, and the GE texture-matrix generator**

- Preserved the raw F3DEX geometry-mode word in the display-list walker and
  derived `TextureGen::{None, Regular, Linear}` from it, so
  `G_TEXTURE_GEN_LINEAR` is a modifier rather than an enabler and a retained
  linear bit survives `G_TEXTURE_GEN` being cleared. Exhaustive transition
  tests, including every partial clear/set.
- Added `romtool texgen`, an archive-wide census of the state every texgen
  vertex is loaded under versus drawn under, written independently of the
  converter. Result: 3,012 texgen triangles, zero load/draw mismatches, so
  primitive-level state is proven rather than assumed (D-039).
- Carried the `gSPTexture` scale and render-tile origin into `PrimDesc`
  (pack `VERSION` 27) — under `G_TEXTURE_GEN` the RSP never reads the
  authored UVs those were already baked into.
- Replaced the GE environment-map path with the texture-matrix generator
  (D-038), after measuring that `sceGuTexScale`/`sceGuTexOffset` have no
  effect in environment-map mode. The look-at basis is the camera's world
  right/up, from `syMatrixLookAtReflectF`, folded into object space per node
  exactly as the RSP's own `CalculateNormalDir` does.
- Resolved both RDP alpha gates onto the GE's single alpha-test unit in
  `pack::alpha_gate`, with host regressions covering the zero-reference and
  nonzero-reference overlaps documentation previously called unresolved.
- Added `regression_capture_scene12` (scene 11's graph, quarter turn) so the
  reflection's response to rotation is measurable rather than inferred from a
  single frozen frame.
- Refreshed the nine goldens RE-213's mip change had left stale, after
  proving with a scratch worktree build of `c8e7f13` that none of the delta
  is this branch's work.
- Evidence: `docs/reverse-engineering.md` RE-214.

## Verification

RE-214 ran the full escalation: targeted texgen tests, `cargo test -p ssb-rom`
(366 pass), `cargo test --workspace` (524 pass), `cargo fmt --check` in both
the workspace and `psp/`, the archive-wide `romtool texgen` census, a pack
rebuild, PPSSPP determinism on scene 11 (two captures byte-identical), the
scene 11/12 rotation pair, a Dream Land no-regression check that came out
byte-identical to RE-213's recorded level-zero capture, an A/B against a
`c8e7f13` worktree build to attribute the golden deltas, and physical-PSP
captures of both texgen scenes with `exlist` clean.

Two deliberate control experiments were run rather than assumed: installing
the authored-UV scale factor under environment mapping (byte-identical
capture, proving the scale was ignored) and rotating the basis vector
(every reflective facet changed, proving the basis was not).

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–214.
- Rendering methodology: `docs/visual-regression.md`.
- Hardware crash workflow: `docs/psplink.md`.
- Permanent decisions: `DECISIONS.md`.

## Blockers and caveats

- Physical PSP validation is incomplete; PPSSPP does not prove hardware
  correctness.
- Combat is prohibited until rendering gate passes.
- ROM-derived generated assets remain uncommitted; rebuild pack when asset
  pipeline code changes.

## State update contract

Keep this file as current snapshot, not append-only journal. Update only
current task/status, last completed task, next task, blockers, changes,
verification, evidence, documentation and commit. Put detailed investigations
in `docs/reverse-engineering.md`; keep PLAN acceptance entries as short
evidence links. Older session detail remains available through git history.

## Continuation command

For `Continue with the plan`: read `AGENTS.md`, this file, relevant `PLAN.md`
section, relevant `docs/porting-status.md` row and `RE-*` evidence entry; then
inspect git state/recent commits, resume this task or select first eligible
TODO, implement, verify, document, update this snapshot and commit focused
work.
