# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T6 — Tile-state and lighting audit` (next up; not started)
- Status: `TODO`
- Last complete: `RE-229` (2026-09-11), `R2.1/T5 -- linear integer
  conversion`. Determined truncate-vs-round for the float-curve-to-S10.5
  conversion shared by `linear_texgen_uv`/`regular_texgen_uv`
  (`texgen_s10_5_addressed`), per T5's own priority order (microcode, then
  faithful HLE, then original-ROM output). No RSP microcode source exists in
  `refs/ssb-decomp-re` (binary ucode, not decompiled), so this settled on the
  HLE tier: `refs/n64psp`'s `n64psp_texgen_to_s10_5` (`tnl_scalar.c:279-289`,
  `(int16_t)scaled`) and `refs/BattleShip`'s RSP-interpreter texgen path
  (`interpreter.cpp:2868-2869`, `(int32_t)(dotx * texture_scaling_factor.s)`)
  both cast straight to an integer with **no** `+ 0.5` -- truncation, not
  rounding. **Found a second dormant bug**: this project's own
  `texgen_s10_5_addressed` added `0.5` before casting (round-half-up), never
  checked against either reference for this specific step. Fixed by removing
  the `+ 0.5`. Added
  `texgen_s10_5_addressed_truncates_rather_than_rounds_at_half_unit_boundaries`
  (`N+0.49`/`N+0.50`/`N+0.51` boundary cases at every real ROM texgen scale,
  RE-214's census, several integer bases). The two existing real-ROM-data
  regression tests were unaffected -- both exercise `dot = ±1`/`0` cases
  whose curve values land on exact scale-fraction boundaries (`scale/2`,
  `scale/4`), where truncation and rounding agree by construction; the bug
  was real but dormant against existing real-archive coverage, which is why
  T5 needed purpose-built boundary cases rather than relying on those tests.
  **Changed real rendered output for the linear-texgen path only**:
  `linear_texgen_uv` drives actual pack UVs for `G_TEXTURE_GEN_LINEAR`
  primitives (D-040); the ordinary path renders through the GE's own
  hardware texture matrix and never calls `texgen_s10_5_addressed` at
  runtime -- `regular_texgen_uv` is reference-only (RE-228's property test).
  Rebuilt and re-captured `regression_capture_scene11`/`_12`/`_13`:
  scene11/scene12 (regular/GE path) measured **0 differing pixels**,
  confirming the fix stayed scoped away from them; scene13 (the one
  linear-texgen primitive) measured 4,696 differing pixels against its
  pre-fix golden, reconfirmed deterministic (fresh re-capture of the same
  build, 0 differing pixels against the updated golden).
  `tests/golden/r2-metal-texgen-linear.png` updated.
- Next: `R2.1`/T6 -- tile-state and lighting audit. Depends on `R2.0`/P1's
  archive-wide `shift_s`/`shift_t` census -- consume that result for
  texgen-bound tiles rather than re-deriving it. Add `shift_s`/`shift_t` to
  `TileState` and report render tile, masks, shifts, `cms`/`cmt`, origins,
  dimensions and `gSPTexture` scale per texgen mode. At each texgen `G_VTX`,
  report raw `G_LIGHTING` on/off. If all shifts are zero, pin that ROM-backed
  invariant; otherwise implement N64 shifting before completion. Read
  `PLAN.md`'s full `R2.1` section (T1-T10) before starting; T7-T10 remain
  queued behind it in order.
- Blockers: none for starting T6. `R2.1`/T1's own finding (164 cross-node
  differing-transform vertex reuses) is an open, tracked, *known* gap --
  not a blocker for T6. `R2.2`/C1-C7 renderer corrective gate remains behind
  all of `R2.1`. Combat remains gated behind `R2.2`.
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
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-229.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1 measured,
  T2/T3/T4/T5 complete, T6 next).
- Decisions: `DECISIONS.md` -- no new revision this task (D-038/D-040 cover
  the semantics; this task's bug was purely in the integer-conversion step,
  not a decision premise).
- Subsystem: `docs/porting-status.md` -- PSP mesh drawing; `docs/rendering.md`
  -- texgen rows now reflect the corrected S10.5 truncation behavior
  (renderer behavior changed: `regression_capture_scene13` golden updated,
  `_11`/`_12` confirmed unaffected).
- Verification: 1 new host test (`crates/ssb-rom/src/psp_texture.rs`); full
  `cargo test --workspace --all-targets` (pinned 1.98.0 toolchain,
  `SSB64_ROM` set) -- passing (574 prior + 1 new = 575, 0 failed); `rustfmt
  --check` clean on the touched file (pre-existing drift in untouched files,
  e.g. `mesh.rs`, `n64_addressing.rs`, left alone -- out of scope); `cargo
  psp --release` (default features) builds clean; goldens rebuilt and
  re-measured for scene13, confirmed unaffected for scenes 11/12, not
  assumed either way; `psp/`'s own `clippy` is not part of this project's
  gate (native clippy cannot cross-compile to `mipsel-sony-psp`).
- Documentation: RE-229, `PLAN.md` `R2.1`/T5, this snapshot.
- Commit: `1c5b583`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
