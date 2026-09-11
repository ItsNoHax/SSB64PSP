# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T9 — Physical PSP matrix` (not yet started)
- Status: `IN_PROGRESS`
- In progress: `RE-236` (2026-09-11) -- physical-PSP leg of T8 captured,
  closing `R2.1`/T8. The USB permission blocker RE-235 hit was a stale
  device-file permission, not a missing udev rule: replugging the PSP
  re-enumerated it and `/dev/bus/usb/001/008` came up `crw-rw-rw-`;
  `usbhostfs_pc`/`pspsh` connected immediately after. Captured all three
  metal-texgen scenes (11/12/13) on the same PSP Slim, 6.61, ARK/Infinity,
  PSPLink v3.2.1 hardware RE-214/RE-215/RE-233 used, following
  `docs/psplink.md`'s kill/reset-before-`ldstart` methodology between each.
  `exlist` empty throughout. Diffed each native 480x272 capture (2x
  nearest-neighbour upscaled) against its RE-235-refreshed golden: 36,607 /
  27,076 / 30,345 differing pixels -- the same noise-floor order as RE-214's
  own PPSSPP/hardware baseline (25,977 / 28,866, non-texgen Dream Land at
  61,362), not a regression. Visual inspection confirms actual content
  match, including scene 13's corrected linear-texgen gradient bar
  rendering identically on real hardware. `R2.1`/T8 acceptance (original-N64
  RE-234, PPSSPP RE-235, physical-PSP this entry, all three at matching
  content/two rotations, reflection-response and ordinary-vs-linear checked
  directly) is met -- `R2.1`/T8 is `COMPLETE`. Full account in RE-236.
- Previously in progress: `RE-235` (2026-09-11) -- PPSSPP leg of T8 done.
  The `StageMetalFile2` PPSSPP goldens (`tests/golden/r2-metal-texgen{,
  -rotated,-linear}.png`, scenes 11-13) were stale since `R2.1`/T7a
  (RE-232, which fixed real addressing on exactly this file); rebuilding
  found 41,852 / 26,765 / 31,834 differing pixels against the pre-T7a
  goldens. Refreshed all three, each reconfirmed deterministic across a
  second rebuild (`0` diff), and confirmed via `git status` that no other
  golden was touched. Cross-checked qualitatively against RE-234's
  original-ROM captures: crystal silhouette, facet colour pattern and the
  fence/railing texture all match; scene 11 vs 12's quarter-turn confirms
  the reflection still responds to rotation post-fix.
- Last complete: `RE-234` (2026-09-11) -- original-ROM leg of T8 captured. A
  one-byte RAM stage-select warp (`gSCManagerSceneData.spgame_stage` @
  `0x800a4ae7` = `10`/`nSC1PGameStageMMario`) was derived from the decomp
  for the scripted Mupen64Plus harness but not used or measured live; the
  user instead played the real ROM through M64Py's real 1P Mode route to
  stage 8 (VS Metal Mario, Meta Crystal stage) and captured three
  screenshots spanning match start through mid-fight, saved out-of-Git
  under `~/ppsspp-test/re151/stage8-metal/`. Stage/fighter identity
  confirmed visually against `dSC1PGameStageDesc`'s stage-8 entry.
- Previously complete: `RE-233` (2026-09-11) -- fixed a real-hardware-only
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
  changes needed.
- Next: `R2.1`/T9 -- physical PSP matrix. After semantics are fixed, capture
  regular rotations A/B, linear texgen, a raw normal diagnostic and a
  camera-rotation case. Record PSP model, firmware, commit, pack hash,
  EBOOT identity, scene and capture hash. Rebuild and explain goldens after
  semantic changes; do not reuse them silently. Scenes 11-13's physical
  captures (RE-236) already cover part of this (regular rotations A/B via
  scenes 11/12, linear texgen via scene 13) on the current hardware/commit;
  T9 should confirm what remains (raw normal diagnostic, camera-rotation
  case) and consolidate the record rather than re-deriving what RE-236
  already has.
- Blockers: none currently known for T9. `R2.1`/T1's own finding (164
  cross-node differing-transform vertex reuses) is an open, tracked,
  *known* gap -- not a blocker. `R2.2`/C1-C7 renderer corrective gate
  remains behind all of `R2.1`. Combat remains gated behind `R2.2`.
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
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  (`root:root` instead of the udev rule's group-writable mode) after a
  reconnect; replugging the device re-enumerates it and reapplies the rule
  (seen this session, RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-236.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1
  measured, T2/T3/T4/T5/T6/T7a/T8 complete, T7 measured (T7a closed it), T9
  next). RE-233 is not a `PLAN.md` line item -- an interleaved
  hardware-only rendering bug found and fixed via a physical PSPLink run,
  orthogonal to the `R2.1` texgen queue.
- Decisions: `DECISIONS.md` -- no new revision for RE-234/RE-235/RE-236
  (original-ROM/golden-refresh/physical-recapture evidence for an
  already-decided task, not a new decision).
- Subsystem: `docs/porting-status.md` -- unchanged this session; RE-232's
  fix/re-measurement and RE-233's `draw_line_strip` fix are already
  recorded in the PSP mesh drawing row.
- Verification (RE-236): see RE-236 for full hardware/hash record. Summary:
  three physical-PSP `scrshot` captures (sha256 recorded per scene in
  RE-236), `exlist` empty across all three `ldstart`s, diffed against the
  RE-235-refreshed PPSSPP goldens at the same noise-floor order RE-214
  established. No source/pipeline code changed this session -- only
  `tests/golden/r2-metal-texgen*.png` and documentation.
- Documentation: RE-234, RE-235, RE-236, `docs/visual-regression.md`,
  `PLAN.md`, this snapshot.
- Commit: `5895bc5` (RE-233); `af04b05` (RE-234, docs-only, T8 in progress);
  `8c26aa4`/`6639011` (RE-235, golden refresh + docs, T8 in progress);
  `8e29273` (RE-236, physical-PSP capture + T8 closure, docs-only).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
