# Project Status

**Last updated:** 2026-09-10 (RE-212 tenth-golden-scene session)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `Rendering fidelity follow-up — texgen, mipmap selection, and combined alpha gates`

**Status:** `IN_PROGRESS`

**Last completed:** RE-212. Added a tenth golden scene,
`regression_capture_scene10` (`psp/Cargo.toml`, `psp/src/main.rs`), the fifth
fighter-bearing golden besides Mario. Selects Donkey Kong's own model graph
(file 317, offset `0x39A8` — the lower-offset of the file's symmetric
26-node graph pair, matching the convention scenes 6-9 used), the next
untested fighter in `FIGHTER_COSTUME_COUNTS` order once both RE-102's and
RE-103's named fighter sets were exhausted. Reuses scenes 2-4/6-9's
object-viewer freeze/spin-suppression/stage-view-disable pattern exactly.
PPSSPP: two captures byte-identical, plain `regression_capture` still
matches `r0-dream-land-default.png` exactly (0 diff), `cargo test
--workspace` unchanged at 506 passing. New golden committed:
`tests/golden/r2-dk-fighter.png`. Physical PSP: the first two `ldstart`
attempts (after a plain `kill` of a prior module, then after `kill` + `cd`)
both produced the built-in fallback tetrahedron instead of DK, despite
`exlist` empty and `main_thread` alive — a silently failed asset-pack open
invisible to the usual health checks. `pspsh -e reset` before the next
`ldstart` fixed it; DK then rendered correctly with the same clean
`exlist`/`thlist` results the two failing attempts also showed. Native
capture matches the PPSSPP golden with only the same expected
edge-antialiasing/overlay divergence RE-203–210 already documented. Updated
`docs/psplink.md` to recommend `reset` after every `kill`, not only after
an observed fault. Full account in `docs/reverse-engineering.md` RE-212.

**Active follow-up (2026-09-10):** User explicitly authorized implementation
of three documented fidelity gaps before further R2 coverage: preserve and
render `G_TEXTURE_GEN`/`G_TEXTURE_GEN_LINEAR`, remove PSP automatic mip
selection where it disagrees with SSB64's `G_TL_TILE`, and resolve the
`alpha_test` + `G_AC_THRESHOLD` overlap. Then investigate the two live
Yoshi's Island `G_SHADE` cases from original-output evidence. This does not
authorize combat or item gameplay.

**Progress:** `MeshMaterial::texture_gen` now preserves `None`/ordinary/
linear texgen independently through conversion; pack flags retain both source
forms. PSP drawing uses native `sceGuTexMapMode(EnvironmentMap, 0, 1)` for
ordinary `G_TEXTURE_GEN`, restoring ordinary UVs for all other primitives.
The existing packed normal attribute is used directly. Exact
`G_TEXTURE_GEN_LINEAR` is now a documented remaining gap: original formula
is `acos(projected_normal_component) * 1024/pi`, not GE's ordinary linear
normal mapping. PSP texture binding now binds level 0 only, constant LOD plus
bilinear filter, matching `G_TL_TILE`; old lower levels remain in generated
pack but inert. Alpha threshold/reference now survive overlap with
`ALPHA_TEST`; nonzero threshold implies existing `alpha > 0` approximation,
zero retains `Greater`. `cargo test -p ssb-rom`: 351 pass; release host check
passes. PPSSPP captures: Metal scene and changed level-0 Dream Land; two Dream
Land runs byte-identical. Physical PSP Slim/6.61 ARK/Infinity/PSPLink 3.2.1
captures obtained for both. Files/hashes and exact evidence: RE-213. Remaining:
original-output investigation for Yoshi `G_SHADE`; exact CPU linear texgen
still requires model/view normal transform plumbing.

**Dependencies:** R0.5 and R1 complete. R2's one remaining hardware checklist
row is live analog-stick input (needs a human operator); "no hardware-only
rendering failures remain" stays open pending broader coverage — 6 of 12
playable fighters and 39 of 41 stages remain hardware-untested.

**Relevant files:** `PLAN.md` R2; `docs/reverse-engineering.md` RE-201–212;
`docs/psplink.md`; `docs/visual-regression.md`; `tests/golden/*.png`;
`tools/compare-screenshot.sh`; `psp/src/main.rs`; `psp/Cargo.toml`;
`tools/romtool/src/main.rs`'s `FIGHTER_COSTUME_COUNTS` (fighter name to
model-graph file id).

**First checks:** physical PSP hardware is present and PSPLink-reachable
this session (`lsusb` shows `054c:01c9`, `pspsh -e ver` → `PSPLink v3.2.1`
once `usbhostfs_pc -v "$PWD"` is running). Re-check this at the start of the
next session; if hardware is no longer attached, R2 has no further eligible
hardware task, but a new PPSSPP-only golden scene (still useful groundwork)
remains possible.

**Acceptance:** `PLAN.md` R2.

**Next:** the same `regression_capture_sceneN` pattern RE-199/200/205/207/
208/209/210/212 established scales directly to the next untouched fighter
or stage — the next pick is any of the other 6 untested playable fighters
(Samus 320, Luigi 323, Link 324, Jigglypuff 330, Yoshi 338, Pikachu 341 —
model file ids from `FIGHTER_COSTUME_COUNTS`) or 39 untested stages. Find
its model graph via `romtool scene --file <id> --list`, add a
`regression_capture_sceneN` feature following scene 10's exact structure,
and repeat the PPSSPP-then-hardware verification — remembering RE-212's
finding: `reset` PSPLink after any `kill`, before the next `ldstart`, even
when `exlist`/`thlist` look clean. Separately, still open: whether the
analog nub correctly drives the fighter now that RE-202's HUD-crash fix is
live — this requires a human physically operating the device with PSPLink
attached; `pspsh` has no controller-injection command, so an agent session
cannot resolve it alone.

**RE-211 rendering-gap audit:** current decomp/runtime comparison found that
R0.10's `COMPLETE` claim is premature. Runtime supports the 33 packed
palette-cycling material scripts, but not the decomp's stage `TextureIDCurrent`
and UV material tracks; 200/441 fighter costume scripts also carry an ignored
`PaletteID` track. The decomp-confirmed renderer gaps are fighter shadows,
general SObj/UI rendering, and original GObj/display-link scheduling for
multi-pass content. Combined alpha gates and rare/two-cycle combiner formulas
remain bounded fidelity gaps. Current ROM census corrected stale docs: 134/134
material graphs paired, 475 matching nodes, zero mismatches; texture failures
are only the 26 runtime framebuffer references. See RE-211.

## Current state

- R0.5: `COMPLETE`; RE-201's PSPLink capture resolves its physical Dream Land
  canopy comparison.
- R1: `COMPLETE`; every acceptance bullet is checked through RE-200 and its
  R0.5 prerequisite is now satisfied.
- R2: `IN_PROGRESS`; all ten golden regression scenes (Dream Land/Mario,
  `MVOpeningRoom`, `StageSectorFile2`, `CatchSwirl`, Saffron City stage
  animation, Fox, Captain Falcon, Kirby, Ness, Donkey Kong) boot, pack loads,
  stage/fighter/material/texture content matches PPSSPP goldens, and no
  hardware exception remains across any of them (RE-203, RE-205, RE-207,
  RE-208, RE-209, RE-210, RE-212). The real framebuffer-effect `SObj` sprite
  path, VRAM usage, and stage animation are now hardware-verified too
  (RE-204, RE-205). Fox (RE-207), Captain Falcon (RE-208), Kirby (RE-209),
  Ness (RE-210) and Donkey Kong (RE-212) are the first five hardware-verified
  fighters besides Mario — every fighter RE-102/RE-103 named for their
  respective UV-scale/clamp and lit/literal bugs is covered, and DK extends
  coverage past both named sets. RE-212 also found that a bare `kill` of a
  prior PSPLink module can silently break the next module's asset-pack load
  with no symptom `exlist`/`thlist` catch — `docs/psplink.md` now recommends
  `reset` after every `kill`, not only after an observed fault. Exhaustive
  no-failures-remain coverage (6 of 12 fighters, 39 of 41 stages still
  untested) and live analog-stick input remain open.
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

**RE-212 — Tenth golden scene (Donkey Kong) verified on physical PSP
hardware; stale PSPLink module-manager state found to silently break the
next module's asset-pack load**

- Added `regression_capture_scene10` (`psp/Cargo.toml`, `psp/src/main.rs`),
  following scenes 2-4/6-9's object-viewer pattern exactly: overrides
  `object_index` to file 317 offset `0x39A8` (DK's own model graph),
  disables default `stage_view`, reuses the tick-240 freeze, idle-spin
  freeze and HUD suppression.
- Chose DK as the next untested fighter in `FIGHTER_COSTUME_COUNTS` order
  once both RE-102's and RE-103's own named fighter sets were exhausted by
  RE-207–210.
- PPSSPP: two captures byte-identical; plain `regression_capture` (no
  scene-10 feature) still matches `r0-dream-land-default.png` exactly (0
  diff), confirming the new wiring is inert elsewhere. `cargo test
  --workspace` unchanged at 506 passing (no crate logic touched). New golden
  committed: `tests/golden/r2-dk-fighter.png`.
- Physical PSP: the first two `ldstart` attempts (a plain `kill` of a prior
  module, then `kill` + `cd` into the EBOOT's directory) both produced the
  built-in fallback tetrahedron instead of DK, despite `exlist` empty and
  `main_thread` alive in `thlist` — the pack open had silently failed with
  no symptom this project's usual health checks catch. `pspsh -e reset`
  before the next `ldstart` fixed it; DK then rendered correctly with the
  same clean `exlist`/`thlist` results. Diffed 2x-upscaled against the
  PPSSPP golden: only the same edge-antialiasing/overlay divergence
  RE-203–210 already documented, no solid-interior content difference.
  Killed the module and rebuilt the plain default EBOOT afterward.
- Updated `docs/psplink.md` to recommend `reset` after every `kill`, not
  only after an observed fault.
- Evidence: `docs/reverse-engineering.md` RE-212.

## Verification

RE-212 ran the full PPSSPP-then-hardware procedure: PPSSPP determinism
(two captures byte-identical), no-regression check against the existing
Dream Land golden, `cargo test --workspace` (506 passing, unchanged), then
physical-PSP `exlist`/`thlist` state checks and a native framebuffer capture
diffed against the new PPSSPP golden — plus, after the first two attempts
silently failed, a `pspsh -e reset` recovery step re-verified twice to
confirm it reliably fixes the fault before it was documented as the new
default step.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–212.
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
