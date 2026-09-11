# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T2 — Raw signed-byte normal semantics` (next up; not started)
- Status: `TODO`
- Last complete: `RE-225` (2026-09-11), `R2.1/T1 -- G_VTX model-space
  invariance`. Threaded `plan_draw_order`'s already-computed `PlannedList::
  space`/`::world` (previously discarded) into `tools/romtool`'s `texgen`
  walker: `VtxLoadState` and `TexgenWalk` now carry the scene-graph node
  whose matrix is in force at a vertex load and that node's rest-pose
  `Mat4`. `tri()` classifies every earlier-step vertex a triangle reuses as
  same-node/same-list, same-node/cross-list, cross-node with an *equivalent*
  normal-relevant transform, or cross-node *differing* -- new
  `normal_transform_equivalent` compares the 3x3 linear part only (ignores
  translation), exact match or a positive-uniform-scale-only difference
  counting as equivalent.
  **Measured against the real ROM: NOT invariant.** 0 same-node, 0
  cross-list same-node, 142 cross-node equivalent, **164 cross-node
  differing** -- 15 sites, 7 fighter files (300/301/304/305/306/307/312),
  every single instance a `Gfx *dls[2]` pre/post-matrix pair sharing one
  vertex across a parent/child joint boundary (a limb-socket vertex loaded
  under the parent's matrix, drawn again under the child's). This revises
  D-042 (which called for exactly this audit) rather than confirming D-039
  (mode/scale invariance, which is unaffected and still holds).
  **Remedy deferred to T2 -> T3 -> T4 by the plan's own explicit ordering**:
  fixing it (CPU-generating the affected vertices' texcoords from their own
  load-time transform, the way D-040 already does for linear texgen) needs
  a validated regular-texgen CPU reference this project has not built yet --
  T2's raw-normal semantics, T3's LookAt quantization, T4's proven shared
  regular/linear math. Building an un-validated CPU curve now would risk the
  2,848 currently-correct GE-path triangles. Two new host tests:
  `normal_transform_equivalence_ignores_translation_and_uniform_scale` (all
  seven named cases: same-node, cross-list, translation-only,
  identical-rotation, different-rotation, uniform-scale, non-uniform-scale)
  and `texgen_reuse_classifies_same_node_cross_list_and_cross_node`.
- Next: `R2.1`/T2 -- raw signed-byte normal semantics. Use PPSSPP source plus
  a diagnostic PPSSPP/physical-PSP scene to determine how `GU_NORMAL_8BIT`
  feeds `TextureProjectionMapMode::Normal` (`/127`, `/128`, or another
  mapping). Make regular texgen implement `(normal . LookAt) / 127` without
  normalizing the quantized normal, compensating the matrix only from
  measured GE behavior. Test `[127,0,0]`, `[64,0,0]`, `[-128,0,0]`,
  `[90,90,0]` and `[73,-41,99]` against several bases; `[64,0,0]` must
  detect the old normalized-normal behavior. Record the hardware
  measurement. Read `PLAN.md`'s full `R2.1` section (T1-T10) before
  starting; T3-T10 remain queued behind it in order.
- Blockers: none for starting T2. `R2.1`/T1's own finding (164 cross-node
  differing-transform vertex reuses) is an open, tracked, *known* gap --
  not a blocker for T2/T3, which are prerequisites for fixing it, not
  blocked by it. `R2.2`/C1-C7 renderer corrective gate remains behind all of
  `R2.1`. Combat remains gated behind `R2.2`.
  Separately (not blocking): a real bug was found and flagged (not fixed)
  in the `debug_overlay` PSP viewer -- object-view HUD text renders
  corrupted/double-exposed in every capture. See the spawned follow-up task
  (`task_1bf9bc35`). Also open, non-blocking: a before/after PPSSPP TEXVIEW
  screenshot confirming RE-224's CI4 palette-bank fix (global texture
  indices 187/194/195, object indices 60-102 in file 86) was never obtained
  -- manual follow-up for whoever next has hands on the interactive build.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-225.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1 measured,
  T2 next).
- Decisions: `DECISIONS.md` D-042 revised by RE-225's measurement (load-
  space/normal-transform provenance audited: not invariant).
- Subsystem: `docs/porting-status.md` -- PSP mesh drawing; `docs/rendering.md`
  -- "Texture addressing" row (`VERIFIED`); texgen rows unaffected by this
  task (no renderer behaviour changed, only a diagnostic census).
- Verification: `cargo run -p romtool --release -- texgen ROM` (and
  `--lines` for per-site detail) against the real ROM; `cargo test -p
  romtool -- normal_transform_equivalence_ignores_translation_and_uniform_scale
  texgen_reuse_classifies_same_node_cross_list_and_cross_node`; full `cargo
  test --workspace --all-targets` (pinned 1.98.0 toolchain, `SSB64_ROM` set
  to an absolute path) -- 560 passing, 0 failed; `cargo clippy --workspace
  --all-targets` clean; `cargo fmt -p romtool -- --check` clean on touched
  code.
- Documentation: RE-225, `PLAN.md` `R2.1`/T1, `DECISIONS.md` D-042, this
  snapshot.
- Commit: `35be19c`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
