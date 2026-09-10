# Project Status

**Last updated:** 2026-09-10 (RE-216 rebuilt RE-151's harness, corrected the
Metal Box route claim)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `R2 — original-N64 texgen comparison` (RE-216 rebuilt the
Mupen64Plus harness RE-151 left out of Git and proved it can drive frame-exact
scripted menu navigation; the harness is ready, but the route to the actual
Metal content turned out longer than believed — see below)

**Status:** `IN_PROGRESS`

**Last completed:** RE-216 — rebuilt RE-151's scripted original-ROM Mupen64Plus
harness (custom input plugin + Python Core API driver, verified end-to-end
including real scripted menu navigation), and corrected RE-214 §10's "Metal
Box item" route claim after the decomp showed no such item exists. Full
account: `docs/reverse-engineering.md` RE-216.

**Verification.** No Rust source changed, so `cargo test --workspace` was not
re-run. The harness was verified directly: ROM SHA-1 checked
(`e2929e10fccc0aa84e5776227e798abc07cedabf`), a 5-frame deterministic
`M64CMD_ADVANCE_FRAME` smoke test with clean shutdown, and a scripted
menu-navigation run (title skip → Mode Select → VS Mode → VS Options)
confirmed via `M64CMD_TAKE_NEXT_SCREENSHOT` captures at each step. The Metal
Box correction is decomp-sourced, not inferred (`src/it/itdef.h`,
`src/ft/ftdata.c`, `src/ft/ftmanager.c:693`, `src/ft/fttypes.h:906`).

**Commits:** see `git log` for RE-216's commit (docs only — the harness itself
is out-of-Git by the same convention RE-151 used,
`~/ppsspp-test/re151-harness/`).

**Documentation updated:** `docs/reverse-engineering.md` (RE-216, and RE-214
§10's Metal Box claim struck through and corrected), this file.

**Remaining deviation (recorded in `PLAN.md` R2's acceptance list):**

* **No original-N64 comparison for texgen output** (both ordinary and linear
  curves). The only Metal content this port can show is `StageMetalFile2`
  (Meta Crystal) plus `MMarioModel`/`NMarioModel`/`NFoxModel` (files
  300/301/303), all reachable in the original only through 1P mode stage 8 —
  VS Mode cannot select any of them. RE-216 rebuilt RE-151's scripted
  original-ROM harness (Mupen64Plus driven through its public Core API via a
  custom input plugin, `~/ppsspp-test/re151-harness/`, out-of-Git by the same
  convention as RE-151) and verified it end-to-end: deterministic frame
  stepping and real scripted menu navigation (title skip, Mode Select, VS
  Mode, VS Options) all work. RE-214 §10's recommended shortcut — a "Metal
  Box" item applying this material to a fighter in VS Mode — turned out not to
  exist: RE-216 found no metal-type item anywhere in `src/it/`; Metal
  Mario/Fox are a separate, permanent `FTKind` the original only constructs
  for the 1P-mode stage-8 boss fight, selected once at spawn via a fixed
  per-`FTKind` table (`src/ft/ftdata.c`/`ftmanager.c:693`), not something a
  live status-flag poke can retarget. The real remaining prerequisite is
  therefore the full 1P-mode route to stage 8 (substantially longer than
  previously believed — real combat through 7 preceding stages, not an item
  pickup), or a from-scratch RAM-level stage-warp investigation (no existing
  cheat code covers this for this ROM). Neither was attempted this session.
  Texgen (both curves) is therefore `VERIFYING`, not `COMPLETE`.

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

**Next:** the roadmap's own next item is finishing the **original-N64
comparison** now that RE-216 rebuilt and verified the harness: script the real
1P-mode route to stage 8 (or investigate a from-scratch RAM-level stage warp
that still drives `ftManagerMakeFighter` faithfully), reach the Meta Crystal
fight, and capture a model-rotation pair and a camera-rotation pair against
the original. It covers *both* texgen rows in one pass (ordinary and linear
both need it) and also unblocks the **Yoshi's Island `G_SHADE` original-output
investigation** (RE-120's two live primitives), which needs the same harness
and nothing else. If the stage-8 route turns out to be infeasible, the next
eligible work is more
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
  11/12 do not carry it (RE-214 had claimed otherwise). RE-216 rebuilt RE-151's
  original-ROM harness and found RE-214's recommended "Metal Box item" route
  to an original-output texgen comparison does not exist; the real route (1P
  mode to stage 8) remains unscripted. Exhaustive no-failures-remain coverage
  (6 of 12 fighters, 39 of 41 stages still untested), live analog-stick input,
  and the texgen original-output comparison (both curves) remain open.
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

**RE-216 — Rebuilt RE-151's original-ROM harness; corrected the Metal Box
route**

- Rebuilt RE-151's scripted Mupen64Plus harness from scratch (it no longer
  existed on disk): a minimal custom M64+ input plugin
  (`~/ppsspp-test/re151-harness/re151_input.c`, built against headers pulled
  from `mupen64plus-core`'s public `src/api/*.h`) exposing a plain
  `g_buttons[4]` array, plus a Python driver
  (`re151_driver.py`) that runs inside M64Py's Flatpak sandbox
  (`flatpak run --command=python3 net.sourceforge.m64py.M64Py`) and drives
  `libmupen64plus.so.2` through the public Core API — `CoreStartup`,
  `CoreDoCommand`, `CoreAttachPlugin` — never the M64Py GUI.
- Found and fixed a real bug while wiring it up: `ctypes` truncates a bare
  Python `int` to a 32-bit C `int` without explicit `argtypes`, corrupting the
  64-bit dynlib handle passed to `PluginStartup`/`CoreAttachPlugin` and
  segfaulting inside the video plugin. Explicit `c_void_p` argtypes on every
  handle/pointer parameter fixed it.
- Verified end-to-end: ROM identity checked (SHA-1
  `e2929e10fccc0aa84e5776227e798abc07cedabf`), deterministic
  `M64CMD_ADVANCE_FRAME` single-stepping confirmed, and a scripted button
  route (`hold:button+button;...`, decoded to the real `BUTTONS` bitfield)
  drove real menu navigation under full frame control: title skip, Mode
  Select, VS Mode, VS Options — discovering along the way that `Start` is a
  hardcoded shortcut straight into 1P Mode regardless of cursor position,
  while `A` is the actual menu-confirm button.
- Used the working harness to check RE-214 §10's recommended shortcut — a
  "Metal Box" item applying the metal `G_TEXTURE_GEN` material to a fighter in
  VS Mode — against the decomp, and found it does not exist: no metal-type
  item exists anywhere in `src/it/`. `MMarioModel`/`NMarioModel`/`NFoxModel`
  back a separate, permanent `FTKind` the original only constructs for the
  1P-mode stage-8 boss fight, selected once at spawn via a fixed per-`FTKind`
  table (`src/ft/ftdata.c`/`ftmanager.c:693`) — not something a live
  status-flag poke can retarget. RE-214 §10's claim is now struck through and
  corrected in place.
- Evidence: `docs/reverse-engineering.md` RE-216.

## Verification

RE-216 verified the harness itself, not a rendering change (no workspace code
changed): a 5-frame deterministic smoke test with clean shutdown and no leaked
process, then a longer scripted-menu run (300+ frames) with periodic
`M64CMD_TAKE_NEXT_SCREENSHOT` captures confirming each menu transition
(title → Mode Select → VS Mode → VS Options) matched the intended input. The
Metal Box correction is decomp-sourced (`src/it/itdef.h`, `src/ft/ftdata.c`,
`src/ft/ftmanager.c:693`, `src/ft/fttypes.h:906`), not inferred. `cargo test
--workspace` was not re-run since no Rust source changed.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–216.
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
