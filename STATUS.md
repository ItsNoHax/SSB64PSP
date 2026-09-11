# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T8 — Original-ROM Metal comparison` (in progress)
- Status: `IN_PROGRESS`
- In progress: `RE-235` (2026-09-11) -- PPSSPP leg of T8 done. The
  `StageMetalFile2` PPSSPP goldens (`tests/golden/r2-metal-texgen{,-rotated,
  -linear}.png`, scenes 11-13) were stale since `R2.1`/T7a (RE-232, which
  fixed real addressing on exactly this file); rebuilding found 41,852 /
  26,765 / 31,834 differing pixels against the pre-T7a goldens. Refreshed all
  three, each reconfirmed deterministic across a second rebuild (`0` diff),
  and confirmed via `git status` that no other golden was touched. Cross-
  checked qualitatively against RE-234's original-ROM captures: crystal
  silhouette, facet colour pattern and the fence/railing texture all match;
  scene 11 vs 12's quarter-turn confirms the reflection still responds to
  rotation post-fix. Blocked on the physical-PSP leg: `usbhostfs_pc`
  refused the connected PSP with a USB permission error even though
  `/etc/udev/rules.d/50-psplink.rules` is installed (`/dev/bus/usb/001/003`
  still `root:root`) -- needs a device replug or udev retrigger, not
  resolved this session. `docs/visual-regression.md` and `PLAN.md` updated;
  full account in RE-235.
- Previously in progress: `RE-234` (2026-09-11) -- original-ROM leg of T8
  captured. A one-byte RAM stage-select warp
  (`gSCManagerSceneData.spgame_stage` @ `0x800a4ae7` = `10`/
  `nSC1PGameStageMMario`) was derived from the decomp for the scripted
  Mupen64Plus harness but not used or measured live; the user instead played
  the real ROM through M64Py's real 1P Mode route to stage 8 (VS Metal
  Mario, Meta Crystal stage) and captured three screenshots spanning match
  start through mid-fight, saved out-of-Git under
  `~/ppsspp-test/re151/stage8-metal/`. Stage/fighter identity confirmed
  visually against `dSC1PGameStageDesc`'s stage-8 entry.
- Last complete: `RE-233` (2026-09-11) -- fixed a real-hardware-only
  collision/fighter debug-overlay corruption found during a routine
  physical-PSP PSPLink run of `766cb47`: the fighter's magenta collision
  diamond (`meshdraw::draw_fighter`) had one stray edge reaching all the way
  to a stage collision-line corner, reproducing from frame 0 on a fresh
  `ldstart`, on every stage tried; `exlist` stayed empty (no crash). Root
  cause: `Gpu::draw_line_strip` (`psp/src/gu.rs`), shared by
  `draw_collision` and `draw_fighter`, submitted GE vertex data straight
  from a `static mut LINE_BUF` that the CPU rewrites for every line segment
  in a frame, instead of copying into `sceGuGetMemory`-allocated
  display-list-arena memory the way `draw_object_posed`'s existing dynamic
  vertex path already does; the GE reads vertex data asynchronously and on
  real hardware was still reading an earlier segment's vertices out of
  `LINE_BUF` after the CPU had already overwritten them with a later
  segment's (matching the stray vertex landing exactly on a real stage
  collision-line corner, not at random). PPSSPP's GE emulation keeps pace
  with the CPU closely enough that this race never surfaces there. Fixed by
  making `draw_line_strip` copy into `sceGuGetMemory` memory before
  submitting, matching `draw_object_posed`'s established pattern; no caller
  changes needed. `draw_triangles` (the renderer's other raw-pointer GE
  draw) was checked and found already safe (its one caller reads a
  never-rewritten `static`, not `static mut`, array). Re-verified clean on
  the same physical PSP (Slim, 6.61, ARK/Infinity, PSPLink v3.2.1) across a
  fresh reset/reload and multiple frames per load. This was not part of the
  `R2.1` texgen queue; `R2.1/T8` (below) is still the next roadmap task.
- Previously complete: `RE-232` (2026-09-11), `R2.1/T7a -- fix mask-narrowed
  clamp-without-mirror texgen addressing divergence` (status `COMPLETE`).
  RE-231 (T7) had measured 9 of 34 real texgen axis instances diverging from
  the hardware addressing model at exactly the sweep's `dot = +1` extreme,
  always on a `Regular`-mode axis whose mask genuinely narrows below the
  tile's drawn rect, and pinned it as a regression baseline pending a fix.
  Root cause: `n64_addressing::psp_lowering_axis`'s `!mirror` clamp branch
  clamped the raw texel index to `period - 1` (the narrowed mask period's
  own last texel) directly, instead of clamping to the drawn rect's real
  far edge (`drawn - 1`) and only then folding through the mask period the
  way `address_axis` (the hardware model) does. `texture::mirror_extend`'s
  matching real bake had the identical bug: `mirror_axis_len` baked only
  `period` texels for every non-mirror axis regardless of the clamp bit, so
  `sceGuTexWrap(Clamp)` in `meshdraw::bind_texture` held at the same
  one-period-early edge on real hardware -- confirming RE-231's
  medium-confidence hypothesis that this was a real PSP rendering
  divergence, not only a host-model gap. Fixed both: `psp_lowering_axis`'s
  `!mirror` clamp branch now clamps to `drawn - 1` before folding by the
  mask period (a no-op when the mask does not narrow below the drawn rect,
  the common case), and `mirror_axis_len` now bakes to `drawn` for any
  clamped axis, mirrored or not, with `mirror_fold` generalized to always
  wrap by `% period` so the wider non-mirror bake still reads valid source
  texels. Mirrors the mirror+clamp fix RE-220/RE-221 already made for the
  mirrored case; applies uniformly to `Regular` and `Linear` texgen since
  the fix lives in the bound texture/wrap state, not the UV source.
  Re-running `texgen_addressing_census_against_real_archive_materials`
  against the real ROM measured the divergence at a strict `0`/34 (down
  from `9`/34), and the test's pinned baseline was dropped to `0`
  accordingly; no other real texgen or authored-UV tile shape regressed.
  `assets/generated/ssb64.pak` rebuilt (gitignored, not committed) since
  this changed asset-pipeline code (`texture::mirror_extend`). No new
  `DECISIONS.md` entry: this was a correctness fix to already-decided
  addressing semantics (D-038), not a new architectural decision, matching
  RE-220/RE-221's own precedent.
- Next: `R2.1`/T8 continuation -- original-ROM leg captured (RE-234), PPSSPP
  leg done and cross-checked (RE-235). Still needed: the physical-PSP leg
  (both the regular-rotation scenes 11/12 re-diff and the linear scene 13
  re-diff against the just-refreshed goldens), currently blocked on a USB
  permission error -- see Blockers below -- then T8 can formally close.
- Blockers: the physical-PSP leg of T8 needs the connected PSP
  (`054c:01c9`) reachable via `usbhostfs_pc`/PSPLink again. `/dev/bus/usb/
  001/003` is `root:root` despite the udev rule being installed; needs
  either the device replugged so the rule re-applies, `sudo udevadm control
  --reload-rules && sudo udevadm trigger`, or confirmation PSPLink (not mass
  storage) is actually open on the PSP -- a physical/`sudo` action outside
  this project's tree, not resolved this session.
  `R2.1`/T1's own finding (164 cross-node differing-transform vertex reuses)
  is an open, tracked, *known* gap -- not a blocker. `R2.2`/C1-C7 renderer
  corrective gate remains behind all of `R2.1`. Combat remains gated behind
  `R2.2`.
  Separately (not blocking): a real bug was found and flagged (not fixed)
  in the `debug_overlay` PSP viewer -- object-view HUD text renders
  corrupted/double-exposed in every capture. See the spawned follow-up task
  (`task_1bf9bc35`). Also open, non-blocking: a before/after PPSSPP TEXVIEW
  screenshot confirming RE-224's CI4 palette-bank fix (global texture
  indices 187/194/195, object indices 60-102 in file 86) was never obtained
  -- manual follow-up for whoever next has hands on the interactive build.
  Also open, non-blocking, low-confidence: RE-228's residual ~1.78-S10.5-unit
  clamp-boundary deviation is measured but not checked against real `sceGu`
  calls the way RE-226's normal-semantics question was -- minor lead for a
  future task, not currently assigned.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-235.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1
  measured, T2/T3/T4/T5/T6/T7a complete, T7 measured (T7a closed it), T8 in
  progress -- original-ROM leg captured per RE-234, PPSSPP leg done and
  cross-checked per RE-235, physical-PSP leg blocked/open). RE-233 is not a
  `PLAN.md` line item -- an interleaved hardware-only rendering bug found
  and fixed via a physical PSPLink run, orthogonal to the `R2.1` texgen
  queue.
- Decisions: `DECISIONS.md` -- no new revision for RE-233 (a lifetime/GE
  submission correctness fix, not a decision premise change) or for RE-232
  (correctness fix to already-decided addressing semantics, not a new
  decision).
- Subsystem: `docs/porting-status.md` -- PSP mesh drawing row updated to
  record both RE-232's fix/re-measurement and RE-233's `draw_line_strip`
  fix.
- Verification (RE-233): rebuilt (`cargo psp --release` from `psp/`, pinned
  nightly-2026-08-01) and reloaded via PSPLink on the same PSP Slim, 6.61,
  ARK/Infinity, PSPLink v3.2.1 hardware the bug was found on; native
  `scrshot` captures immediately after a fresh `reset`/`ldstart`, and again
  across three more frames several seconds apart, all show a clean fighter
  collision diamond with no stray edge (confirmed by pixel-sampling the
  exact stray-magenta color present before the fix and absent after);
  `exlist` stayed empty throughout. Host-side `cargo +1.98.0 fmt --all --
  --check`, strict `cargo +1.98.0 clippy --workspace --all-targets -- -D
  warnings`, and `cargo +1.98.0 test --workspace --all-targets` (`SSB64_ROM`
  set) all pass; this fix touches only `psp/`, outside the host workspace.
  No asset-pipeline code changed, so `assets/generated/ssb64.pak` did not
  need rebuilding.
- Documentation: RE-233, RE-235, `docs/visual-regression.md`,
  `docs/porting-status.md`, this snapshot.
- Commit: `5895bc5` (RE-233); `af04b05` (RE-234, docs-only, T8 in progress);
  `8c26aa4` (RE-235, golden refresh + docs, T8 still in progress).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
