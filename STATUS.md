# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T7 — Texgen addressing phase` (next up; not started)
- Status: `TODO`
- Last complete: `RE-230` (2026-09-11), `R2.1/T6 -- tile-state and lighting
  audit`. Consumed RE-223's (`R2.0`/P1) archive-wide `shift_s`/`shift_t`
  census rather than re-deriving it, and re-checked that invariant
  specifically against the texgen-bound tile subset (not just all tile-0
  binds generally). Extended `tools/romtool/src/main.rs`'s existing
  `TexgenWalk`/`TexgenCensus` (RE-225-RE-229's own `R2.1`/T1-T5
  infrastructure) with `shift_s`/`shift_t` on the walker's local
  `TileState`, and three new per-mode census maps: tile setup, `G_TEXTURE`
  scale, and raw `G_LIGHTING` observed at each texgen `G_VTX` -- `Regular`
  and `Linear` generate different UVs against the same bound tile, so
  pooling them together would have hidden mode-specific structure.
  Extracted the walk-building loop into `build_texgen_census` so both the
  `texgen` CLI report and a new `SSB64_ROM`-gated test
  (`texgen_tile_state_and_lighting_audit`) measure the identical real
  archive-wide walk. **Measured, archive-wide, real ROM** (1,640
  graph-planned lists, 555 discovered root lists, 3,012 texgen triangles):
  every texgen-bound tile setup carries `shift == (0, 0)` -- pinned with a
  regression assertion, no N64 shifting implementation needed. `Regular`
  uses 5 distinct `G_TEXTURE` scales across 16 distinct tile setups (CI4/CI8
  formats, masks 3-6, `cm` combinations `(2,2)`/`(3,2)`/`(2,3)`, dimensions
  `8x8` through `384x192`, several nonzero tile origins); `Linear` uses 2
  scales (both a subset of `Regular`'s) across 4 tile setups. Raw
  `G_LIGHTING` at a texgen `G_VTX`: `Regular` splits 336 unlit / 12 lit;
  every observed `Linear` `G_VTX` (34) was unlit -- recorded for T7/T8's
  use, not acted on by this task. No renderer/pack code was touched (audit
  only), so no PPSSPP/physical capture was needed and no golden changed.
- Next: `R2.1`/T7 -- texgen addressing phase. Consumes `R2.0`/P0b's general
  N64 tile-addressing reference model rather than building a second one;
  narrows to verifying the texgen-specific vertex-load/scale wiring against
  that shared model. Add host/reference cases for zero/nonzero origins on
  each axis, repeat+mask, mirror+repeat, mirror+clamp, padded PSP dimensions
  and partial uploaded-texture scale. Compare `RSP coordinate -> scale ->
  shift -> origin -> mask -> mirror/clamp` with the PSP-lowered path for
  every real texgen material. RE-230's per-mode tile/scale catalogue (above)
  is real content to build those cases from.
- Blockers: none for starting T7. `R2.1`/T1's own finding (164 cross-node
  differing-transform vertex reuses) is an open, tracked, *known* gap --
  not a blocker for T7. `R2.2`/C1-C7 renderer corrective gate remains behind
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
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-230.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1 measured,
  T2/T3/T4/T5/T6 complete, T7 next).
- Decisions: `DECISIONS.md` -- no new revision this task (this task was a
  measurement/audit against already-decided semantics, not a decision
  premise change).
- Subsystem: `docs/porting-status.md` -- PSP mesh drawing row updated to
  record RE-230's shift-invariant confirmation and per-mode state catalogue.
- Verification: 1 new host test
  (`tools/romtool/src/main.rs::tests::texgen_tile_state_and_lighting_audit`,
  `SSB64_ROM`-gated); full `cargo test --workspace --all-targets` (pinned
  1.98.0 toolchain, `SSB64_ROM` set) -- passing (575 prior + 1 new = 576, 0
  failed); `rustfmt --check` clean on the touched file; `cargo psp --release`
  (default features) builds clean; no goldens touched (no renderer/pack code
  changed this task).
- Documentation: RE-230, `PLAN.md` `R2.1`/T6, `docs/porting-status.md`, this
  snapshot.
- Commit: (pending).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
