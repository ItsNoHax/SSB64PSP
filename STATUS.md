# Project Status

**Last updated:** 2026-09-10 (RE-208 seventh-golden-scene session)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `R2 — Physical PSP Rendering Validation`

**Status:** `IN_PROGRESS`

**Last completed:** RE-208. Added a seventh golden scene,
`regression_capture_scene7` (`psp/Cargo.toml`, `psp/src/main.rs`), the
second fighter-bearing golden besides Mario. Selects Captain Falcon's own
model graph (file 332, offset `0x3BE0` — the lower-offset of the file's two
symmetric graphs, matching the convention scene 6 used for Fox), chosen
because RE-102 already named Falcon alongside Fox and Kirby as one of three
fighters with a real UV-scale/clamp texture bug. Reuses scenes 2-4/6's
object-viewer freeze/spin-suppression/stage-view-disable pattern exactly.
PPSSPP: two captures byte-identical, plain `regression_capture` still
matches `r0-dream-land-default.png` exactly (0 diff), `cargo test
--workspace` unchanged at 506 passing. New golden committed:
`tests/golden/r2-falcon-fighter.png`. Physical PSP: no stale module was
loaded this session, `ldstart`ed the new PRX over `host0:`, zero exceptions
(`exlist` empty, `main_thread` alive), native capture matches the PPSSPP
golden with only the same expected edge-antialiasing/overlay divergence
RE-203/204/205/207 already documented. Full account in
`docs/reverse-engineering.md` RE-208.

**Dependencies:** R0.5 and R1 complete. R2's one remaining hardware checklist
row is live analog-stick input (needs a human operator); "no hardware-only
rendering failures remain" stays open pending broader coverage — 9 of 12
playable fighters and 39 of 41 stages remain hardware-untested.

**Relevant files:** `PLAN.md` R2; `docs/reverse-engineering.md` RE-201–208;
`docs/psplink.md`; `docs/visual-regression.md`; `tests/golden/*.png`;
`tools/compare-screenshot.sh`; `psp/src/main.rs`; `psp/Cargo.toml`.

**First checks:** physical PSP hardware is present and PSPLink-reachable
this session (`lsusb` shows `054c:01c9`, `pspsh -e ver` → `PSPLink v3.2.1`
once `usbhostfs_pc -v "$PWD"` is running). Re-check this at the start of the
next session; if hardware is no longer attached, R2 has no further eligible
hardware task, but a new PPSSPP-only golden scene (still useful groundwork)
remains possible.

**Acceptance:** `PLAN.md` R2.

**Next:** the same `regression_capture_sceneN` pattern RE-199/200/205/207/208
established scales directly to the next untouched fighter or stage — Kirby
is a natural pick (the third fighter RE-102 named alongside Fox and Falcon),
or another fighter/stage RE-101/102 already found a real UV-scale/clamp bug
on, per `docs/reverse-engineering.md`. Find its model graph via
`romtool scene --file <id> --list`, add a `regression_capture_sceneN`
feature following scene 7's exact structure, and repeat the
PPSSPP-then-hardware verification. Separately, still open: whether the
analog nub correctly drives the fighter now that RE-202's HUD-crash fix is
live — this requires a human physically operating the device with PSPLink
attached; `pspsh` has no controller-injection command, so an agent session
cannot resolve it alone.

## Current state

- R0.5: `COMPLETE`; RE-201's PSPLink capture resolves its physical Dream Land
  canopy comparison.
- R1: `COMPLETE`; every acceptance bullet is checked through RE-200 and its
  R0.5 prerequisite is now satisfied.
- R2: `IN_PROGRESS`; all seven golden regression scenes (Dream Land/Mario,
  `MVOpeningRoom`, `StageSectorFile2`, `CatchSwirl`, Saffron City stage
  animation, Fox, Captain Falcon) boot, pack loads, stage/fighter/material/
  texture content matches PPSSPP goldens, and no hardware exception remains
  across any of them (RE-203, RE-205, RE-207, RE-208). The real
  framebuffer-effect `SObj` sprite path, VRAM usage, and stage animation are
  now hardware-verified too (RE-204, RE-205). Fox (RE-207) and Captain
  Falcon (RE-208) are the first two hardware-verified fighters besides
  Mario. Exhaustive no-failures-remain coverage (9 of 12 fighters, 39 of 41
  stages still untested) and live analog-stick input remain open.
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

**RE-208 — Seventh golden scene (Captain Falcon) verified on physical PSP hardware**

- Added `regression_capture_scene7` (`psp/Cargo.toml`, `psp/src/main.rs`),
  following scenes 2-4/6's object-viewer pattern exactly: overrides
  `object_index` to file 332 offset `0x3BE0` (Captain Falcon's own model
  graph), disables default `stage_view`, reuses the tick-240 freeze,
  idle-spin freeze and HUD suppression.
- Chose Captain Falcon specifically because RE-102 already named him,
  alongside Fox and Kirby, as one of three fighters with a real UV-scale/
  clamp texture bug — extending RE-207's Fox regression lineage to a second
  fighter, not an arbitrary pick.
- PPSSPP: two captures byte-identical; plain `regression_capture` (no
  scene-7 feature) still matches `r0-dream-land-default.png` exactly (0
  diff), confirming the new wiring is inert elsewhere. `cargo test
  --workspace` unchanged at 506 passing (no crate logic touched). New golden
  committed: `tests/golden/r2-falcon-fighter.png`.
- Physical PSP: no stale module was loaded this session, `ldstart`ed the new
  PRX over `host0:`, confirmed `exlist` empty and `main_thread` alive in
  `thlist`, captured a native 480x272 `scrshot`. Diffed 2x-upscaled against
  the PPSSPP golden: only the same edge-antialiasing/overlay divergence
  RE-203/204/205/207 already documented, no solid-interior content
  difference. Killed the module and rebuilt the plain default EBOOT
  afterward.
- Evidence: `docs/reverse-engineering.md` RE-208.

## Verification

RE-208 ran the full PPSSPP-then-hardware procedure: PPSSPP determinism
(two captures byte-identical), no-regression check against the existing
Dream Land golden, `cargo test --workspace` (506 passing, unchanged), then
physical-PSP `exlist`/`thlist` state checks and a native framebuffer capture
diffed against the new PPSSPP golden.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–208.
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
