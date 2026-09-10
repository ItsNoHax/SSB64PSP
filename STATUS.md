# Project Status

**Last updated:** 2026-09-10 (RE-215 exact `G_TEXTURE_GEN_LINEAR`)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `R2 — broaden physical-PSP golden coverage` (both texgen
fidelity follow-ups from RE-214 are now resolved or correctly scoped; see
"Last completed task")

**Status:** `IN_PROGRESS`

**Last completed:** RE-215 — exact `G_TEXTURE_GEN_LINEAR`, and a correction to
an RE-214 claim about which scene carries it. Full account:
`docs/reverse-engineering.md` RE-215.

1. **The fix.** `G_TEXTURE_GEN_LINEAR`'s `acos(-dot)/(2*pi)` curve is now
   generated exactly, per vertex, on the CPU (`ssb_rom::psp_texture::
   linear_texgen_uv`), using the RSP's own dot product (`normal / 127`, not a
   true renormalisation) — cross-checked against two independent reference
   implementations (`refs/BattleShip`, `refs/n64psp`) that agree exactly. The
   result is written into the pack's existing raw S10.5 authored-UV unit, so
   the primitive draws through the *ordinary* authored-UV pipeline
   (`draw_mesh`'s dynamic-vertex branch, previously only used for runtime
   material-colour animation) rather than a second GE coordinate-generation
   mode. A lookup table was considered and rejected: the vertex normal is
   quantised but the look-at basis it is dotted against is a continuous
   per-frame float, so a LUT keyed on the normal alone cannot be exact either
   (D-040).
2. **The scene-11 correction.** Verifying the fix, `regression_capture_scene11`
   came back byte-identical pre- and post-fix. A debug-marker reachability
   probe plus a direct read of the pack found why: file 117 has four
   `StageMetalFile2` graphs, and the archive's one packed linear primitive
   belongs to the graph at `0x2EE0`, not `0x1B10` (scenes 11/12's graph, which
   turns out to carry only ordinary texgen). This corrects RE-214's own "24 of
   them in scene 11" claim, which was written from an archive-wide geometry-mode
   census rather than checked against the packed graph scenes 11/12 actually
   select. New `regression_capture_scene13` targets the correct graph.

**Verification.** `cargo test --workspace` 531 pass (was 524), `cargo fmt
--check` clean in workspace and `psp/`. Pack unchanged (draw-time-only fix),
SHA-256 `295b62dc...`. PPSSPP: scene 13 differs from a pre-fix A/B build by
10,766 pixels (fix is not inert), two captures of the fixed build are
byte-identical; scenes 11/12, Dream Land, and the Fox fighter golden are all
still byte-identical to their existing goldens. Physical PSP (Slim, 6.61
ARK/Infinity, PSPLink 3.2.1): scene 13 renders with `exlist` empty,
`main_thread` alive, native capture
`~/ppsspp-test/re214-linear-hw/psp-hw-scene13.bmp`
(`9f187e53...`) visually matching the PPSSPP capture.

**Commits:** see `git log` for RE-215's commit (implementation + docs, made
after this update).

**Documentation updated:** `docs/reverse-engineering.md` (RE-215),
`docs/rendering.md` (texgen section and status row), `docs/porting-status.md`,
`docs/visual-regression.md` (thirteenth scene section), `PLAN.md` R2,
`DECISIONS.md` (D-040), `psp/Cargo.toml` (scene 11's comment corrected), this
file.

**Remaining deviation (recorded in `PLAN.md` R2's acceptance list):**

* **No original-N64 comparison for texgen output** (both ordinary and linear
  curves). The only Metal content this port can show is `StageMetalFile2`
  (Meta Crystal), reachable in SSB64 only through 1P mode stage 8 — VS Mode
  cannot select it. RE-151's scripted original-ROM harness (a temporary
  out-of-Git Mupen64Plus input plugin plus a Python Core API driver) no longer
  exists on disk; only its screenshots under `~/ppsspp-test/re151/` remain.
  Rebuilding it and scripting a route to stage 8 is the prerequisite. Metal
  Box on a fighter in VS Mode, reachable through files 300/301/303, is a
  shorter route to the *same* material and is the recommended first attempt —
  see RE-214 §10 for the route options and the reasoning. Texgen (both
  curves) is therefore `VERIFYING`, not `COMPLETE`.

**Dependencies:** R0.5 and R1 complete. R2's remaining hardware checklist rows
are live analog-stick input (needs a human operator), exhaustive
no-failures-remain coverage, and the texgen original-comparison row above.

**Relevant files:** `PLAN.md` R2; `docs/reverse-engineering.md` RE-201–215;
`docs/psplink.md`; `docs/visual-regression.md`; `tests/golden/*.png`;
`tools/compare-screenshot.sh`; `psp/src/main.rs`; `psp/src/meshdraw.rs`
(`apply_texture_mapping`, `draw_mesh`, `texgen_object_basis`,
`note_model_matrix`); `psp/Cargo.toml`; `crates/ssb-rom/src/mesh.rs`
(`State::geometry_mode`, `TextureGen`); `crates/ssb-rom/src/pack.rs`
(`PrimDesc`, `alpha_gate`); `crates/ssb-rom/src/psp_texture.rs` (mapping math,
`linear_texgen_curve`, `texgen_dot`, `linear_texgen_uv`);
`tools/romtool/src/main.rs` (`texgen`, and `FIGHTER_COSTUME_COUNTS` for
fighter name to model-graph file id).

**First checks:** physical PSP hardware was present and PSPLink-reachable this
session (`lsusb` shows `054c:01c9`; `pspsh -e ver` reports `PSPLink v3.2.1`
once `usbhostfs_pc -v "$PWD"` runs). Re-check at the start of the next
session. Remember RE-212's finding: `pspsh -e reset` after **any** `kill`,
before the next `ldstart`, even when `exlist`/`thlist` look clean.

**Acceptance:** `PLAN.md` R2.

**Next:** the roadmap's own next item is the **original-N64 comparison
harness** (rebuild RE-151's Mupen64Plus driver; the Metal Box VS-Mode route is
the recommended first attempt — see RE-214 §10) — it now covers *both* texgen
rows in one pass (ordinary and linear both need it) and also unblocks the
**Yoshi's Island `G_SHADE` original-output investigation** (RE-120's two live
primitives), which needs the same harness and nothing else. If the harness
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
- R2: `IN_PROGRESS`; thirteen golden regression scenes (Dream Land/Mario,
  `MVOpeningRoom`, `StageSectorFile2`, `CatchSwirl`, Saffron City stage
  animation, Fox, Captain Falcon, Kirby, Ness, Donkey Kong, RE-214's two
  `StageMetalFile2` ordinary-texgen rotations, and RE-215's linear-texgen
  graph) boot, pack loads, stage/fighter/material/texture content matches
  PPSSPP goldens, and no hardware exception remains across any of them
  (RE-203, RE-205, RE-207, RE-208, RE-209, RE-210, RE-212, RE-214, RE-215).
  The real framebuffer-effect `SObj` sprite path, VRAM usage, and stage
  animation are hardware-verified too (RE-204, RE-205). Six fighters besides
  Mario are hardware-verified. RE-212 found that a bare `kill` of a prior
  PSPLink module can silently break the next module's asset-pack load with no
  symptom `exlist`/`thlist` catch — `docs/psplink.md` now recommends `reset`
  after every `kill`. RE-214 refreshed the nine goldens RE-213's mip change
  had left stale. RE-215 made `G_TEXTURE_GEN_LINEAR` exact and found scenes
  11/12 do not carry it (RE-214 had claimed otherwise). Exhaustive
  no-failures-remain coverage (6 of 12 fighters, 39 of 41 stages still
  untested), live analog-stick input, and the texgen original-output
  comparison (both curves) remain open.
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

**RE-215 — Exact `G_TEXTURE_GEN_LINEAR`, and scenes 11/12 never actually
exercised it**

- Generated `G_TEXTURE_GEN_LINEAR`'s `acos(-dot)/(2*pi)` curve exactly, per
  vertex, on the CPU (`ssb_rom::psp_texture::linear_texgen_curve`/
  `texgen_dot`/`linear_texgen_uv`), using the RSP's own `normal / 127` dot
  product — cross-checked against `refs/BattleShip` and `refs/n64psp`, which
  agree exactly and both skip true renormalisation.
- Reused the authored-UV pipeline instead of building a second GE
  coordinate-generation mode: the generated coordinate lands in the pack's
  existing raw S10.5 unit, and the render-tile origin shift reuses
  `push_vertex`'s own `* 8` rule, so a linear-texgen primitive draws through
  `draw_mesh`'s existing dynamic-vertex branch (previously only used for
  runtime material-colour animation).
- Rejected a lookup table: the vertex normal is quantised but the look-at
  basis is a continuous per-frame float, so a LUT keyed on the normal alone
  cannot be exact either (D-040).
- Found, while verifying, that `regression_capture_scene11`'s graph
  (`0x1B10`) carries no linear-texgen content at all — the archive's one
  packed linear primitive lives in a sibling graph (`0x2EE0`). Added
  `regression_capture_scene13` to target it, and corrected the stale "24 of
  them in scene 11" claim RE-214 had left in `STATUS.md` and
  `psp/Cargo.toml`.
- Evidence: `docs/reverse-engineering.md` RE-215.

## Verification

RE-215 ran the full escalation: seven new host tests (curve endpoints and
midpoint, complementary symmetry, differentiation from the ordinary curve,
measured `acos` polynomial error bound, the `/127` dot product, the S10.5
endpoint for all seven real ROM scales, a real-ROM reproduction of file 117's
own linear primitive), `cargo test --workspace` (531 pass), `cargo fmt --check`
in both the workspace and `psp/`, a pack rebuild (hash unchanged — draw-time
fix only), PPSSPP determinism on the new scene 13 (two captures
byte-identical), an A/B against the pre-fix code on that same scene (10,766
differing pixels — not inert), no-regression checks on scenes 11/12, Dream
Land, and the Fox fighter golden, and a physical-PSP capture of scene 13 with
`exlist` clean, visually matching the PPSSPP capture.

A deliberate control experiment was run rather than assumed: a debug colour
marker written into the linear-texgen branch produced no visible change on
scene 11 at all, which is what led to inspecting the pack directly and finding
the primitive lives in a different graph.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–215.
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
