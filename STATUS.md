# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T10 — Texgen documentation cleanup` (not yet started)
- Status: `IN_PROGRESS`
- In progress: `RE-237` (2026-09-11) -- closed `R2.1`/T9's two remaining
  matrix items on physical PSP hardware, consolidating with RE-236's own
  scenes 11-13 rather than re-deriving them. Raw normal diagnostic: re-ran
  `texgen_normal_diagnostic_6` (`[73,-41,99]`, raw `Normal` mode -- the fully
  general multi-axis, negative-component case) on the same PSP Slim, 6.61,
  ARK/Infinity, PSPLink v3.2.1 hardware RE-214/RE-215/RE-233/RE-236 used;
  the native 480x272 capture's equivalent sample point (240, 136), decoded,
  matched RE-226's predicted-and-headless-measured `(179, 99)` texel exactly
  -- the first `texgen_normal_diagnostic_*` case run against real hardware
  rather than only PPSSPP. Camera-rotation case: added
  `regression_capture_scene14` (same file-117 `0x1B10` graph as scenes
  11/12, but framed with a real `sceGumMatrixMode(View)` `look_at` orbiting
  the object at a fixed 35deg yaw/20deg pitch instead of the object viewer's
  usual identity-view placement, feeding `draw_state.texgen_basis` from that
  camera's own right/up -- mirroring `stage_view`'s existing real-camera
  branch (RE-131/RE-214) rather than a second convention). This is the
  non-identity LookAt basis `R2.1`/T3 (RE-227) left dormant: no prior scene
  had ever fed `quantize_lookat_basis` anything but world X/Y. New golden
  `tests/golden/r2-metal-texgen-camera-rotated.png` (PPSSPP headless,
  deterministic across two rebuilds); physical-PSP capture of the same scene
  matched it at 19,304 differing pixels (2x-upscaled) -- the smallest
  noise-floor gap of any texgen scene measured this way so far. `exlist`
  empty throughout both hardware runs; pack hash unchanged from RE-236 (no
  asset-pipeline code touched). `R2.1`/T9 acceptance (regular rotations A/B,
  linear texgen, raw normal diagnostic, camera-rotation case, all with PSP
  model/firmware/commit/pack hash/capture hash recorded) is met --
  `R2.1`/T9 is `COMPLETE`. Full account in RE-237.
- Previously in progress: `RE-236` (2026-09-11) -- physical-PSP leg of T8
  captured, closing `R2.1`/T8. Captured all three metal-texgen scenes
  (11/12/13) on physical PSP, diffed against RE-235's refreshed goldens at
  the same noise-floor order RE-214's own PPSSPP/hardware baseline
  established, not a regression. Visual inspection confirmed actual content
  match, including scene 13's corrected linear-texgen gradient bar
  rendering identically on real hardware.
- Last complete: `RE-235` (2026-09-11) -- PPSSPP leg of T8 done. The
  `StageMetalFile2` PPSSPP goldens (scenes 11-13), stale since `R2.1`/T7a
  (RE-232), were refreshed and cross-checked qualitatively against RE-234's
  original-ROM captures.
- Previously complete: `RE-234` (2026-09-11) -- original-ROM leg of T8: real
  1P Mode play reached stage 8 (VS Metal Mario, Meta Crystal stage), three
  screenshots captured out-of-Git.
- Next: `R2.1`/T10 -- texgen documentation cleanup. After T1-T9, reconcile
  `STATUS.md`, `PLAN.md`, `DECISIONS.md`, `docs/rendering.md`,
  `docs/reverse-engineering.md`, `docs/visual-regression.md` and
  `docs/porting-status.md`, including pack version, test counts, the actual
  linear scene, original-output status, normal semantics, LookAt
  quantization, tile shifts and accepted PSP deviations. `docs/
  porting-status.md` in particular is stale -- its "Known gaps" section
  still cites RE-201-215 and says physical-PSP validation "is still in
  progress," not reflecting RE-233-237's later hardware work. Texgen test
  minimum (all raw GEN/LINEAR combinations, provenance, normals, LookAt,
  scales, origin/clamp/repeat/mirror/mask/shift, mapping transitions) and
  `romtool texgen ROM --verify` per `PLAN.md`'s own T10 text.
- Blockers: none currently known for T10. `R2.1`/T1's own finding (164
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
  future task, not currently assigned. New, non-blocking, low-confidence
  (noticed this session): a fresh PPSSPP-headless rebuild of scene 11 on
  this machine diffs from the exact committed `r2-metal-texgen.png` bytes by
  31,737 pixels, even though two fresh rebuilds against each other are
  byte-identical (self-consistent) -- the same noise-floor-order pattern
  this project already treats as expected cross-build/cross-environment
  variance (RE-214/RE-236 precedent), not investigated further since it
  did not block this session's own work and reproduces identically on
  unmodified `main`.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-237.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1
  measured, T2/T3/T4/T5/T6/T7a/T8/T9 complete, T7 measured (T7a closed it),
  T10 next).
- Decisions: `DECISIONS.md` -- no new revision for RE-234/RE-235/RE-236/
  RE-237 (original-ROM/golden-refresh/physical-recapture/new-scene evidence
  for already-decided tasks, not a new decision).
- Subsystem: `docs/porting-status.md` -- unchanged this session (stale; see
  Next/Blockers above). `R2.1`/T10 owns reconciling it.
- Verification (RE-237): see RE-237 for the full hardware/hash record.
  Summary: one physical-PSP `scrshot` per new item (raw normal diagnostic
  case 6, scene 14), sha256 recorded per capture, `exlist` empty across both
  `ldstart`s, PPSSPP-headless golden added for scene 14 (deterministic
  across two rebuilds) and diffed against its own physical-PSP capture at
  the same noise-floor order established elsewhere in this project. Host
  workspace: `cargo test --workspace` -- 402 passed, 0 failed. `cargo fmt
  --check` clean in `psp/`. Code changed: `psp/src/main.rs` (new
  `regression_capture_scene14` branch), `psp/Cargo.toml` (new feature).
  Asset pack untouched (hash unchanged).
- Documentation: RE-237, `docs/visual-regression.md` (fourteenth deterministic
  scene section), `PLAN.md` (T9 closure), this snapshot.
- Commit: `16b8104` (RE-236 commit-hash record, prior session); this
  session's `R2.1`/T9 closure (RE-237) not yet committed as of this
  snapshot -- see the next commit and its own "record commit hash"
  follow-up.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
