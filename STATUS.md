# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.2 — Second Renderer Corrective Gate (C1-C7)` (`IN_PROGRESS`)
- Status: `IN_PROGRESS` -- C1 (`COMPLETE`); C2 (`COMPLETE`); C3 (`IN_PROGRESS`,
  part 2 of an unknown number, RE-244/RE-245); C4-C7 remain
- Last complete: `RE-245` (2026-09-11) -- `R2.2`/C3 part 2: found and wired
  the same external depth-state wrapper C1/C2/C3-part-1 found for fighters,
  but for stage geometry. Reading `refs/ssb-decomp-re/src/gr/grdisplay.c`
  directly found every `grDisplayLayerNPriProcDisplay`/`SecProcDisplay`
  (`N` = 0..4, the four render layers `GroundLayer` already names) sets
  `G_ZBUFFER` and `gDPSetRenderMode` unconditionally before walking the
  layer's own node lists -- the same "wrap-and-walk" shape as
  `ftDisplayMainProcDisplay`, but varying **by layer index**: layers 0/2/3
  clear `G_ZBUFFER` and set a non-`ZB` render mode (already
  `InitialMaterial::default`), while layer 1 sets `G_ZBUFFER` and
  `G_RM_AA_ZB_OPA_SURF` -- `Z_CMP | Z_UPD | ZMODE_OPA`, identical to
  `FIGHTER_EXTERNAL`'s depth fields. A census confirmed the population
  first (`census_ground_layer_depth_state_vs_z_buffer`, kept permanently):
  1213 ground-layer primitives archive-wide (339/776/18/80 across layers
  0/1/2/3); layer 1 alone read `z_buffer`=776/776 but `depth_test`/
  `depth_write`=199/199 before any seed. Added `InitialMaterial::
  GROUND_LAYER1_EXTERNAL` (`lit: false` -- `gr`'s wrapper never sets
  `G_LIGHTING`, unlike `ft`'s) and `tools/romtool`'s `ground_layer1_graphs`
  (mirroring `fighter_skeleton_graphs`), wired into `initial_material_for`
  and every production call site. Unlike layers 0/2/3, **not redundant**:
  `census_ground_layer1_seed_measured_impact` (kept permanently) found the
  seed flips 577/776 (74%) of layer-1 primitives from false to true, across
  41 distinct graphs. Archive-wide, `depth_test`/`depth_write` rise from
  1324/1308 to 1901/1885; the `z_buffer`-vs-`depth_test` gap falls from 4468
  to **3891** (12.9% explained). Also read `refs/ssb-decomp-re/src/it/`
  (items) and `src/ef/` (effects) directly: their normal in-game
  `ProcDisplay`s call `gcDrawDObjTreeForGObj`/`DLLinksForGObj` with **no**
  external geometry-mode or render-mode wrapper at all, unlike `ft`/`gr` --
  so the remaining 3891-primitive gap (layers 0/2/3, items, effects) is not
  attributed to a specific cause yet. Also found and left open: the
  project's own node model parses which task `list_id` a `DObjDLLink` entry
  targets (`scene::DlLink::list_id`) but discards it when flattening to a
  `PlannedList` -- so layer 1's 21 list-1 (translucent, depth-test-without-
  write) entries out of 163 are seeded as opaque write-on rather than their
  real state, a small bounded overstatement, not yet fixed. `psp/src/
  meshdraw.rs` is **deliberately unchanged** -- still keys `GuState::
  DepthTest` off `z_buffer` (RE-068, device-validated). `assets/generated/
  ssb64.pak` rebuilt; mesh/triangle/draw-call counts (2044/36772/8056)
  verified identical to a pre-change rebuild of the same ROM (checksum
  itself differs, as expected). See `docs/reverse-engineering.md` RE-245
  for the full entry.
- Previously complete: `RE-244` (2026-09-11) -- `R2.2`/C3 part 1: independent
  depth compare/write state data model, plus the same external-per-object
  seed mechanism C2 used, but for `G_SETRENDERMODE` this time. Added
  `MeshMaterial::{depth_test, depth_write, depth_mode: ZMode}`, derived from
  `G_SETRENDERMODE`'s `Z_CMP`/`Z_UPD`/`ZMODE` bits at the same render-mode
  site `alpha_test`/`translucent` already read; `z_buffer` (`G_ZBUFFER`) is
  untouched, still RSP geometry-mode state, not the RDP's own per-pixel
  decision. Archive-wide before any seed: of 5872 primitives, `z_buffer` is
  true for 5792 (98.6%) but `depth_test`/`depth_write` only 592/576 (10%) --
  a real 90% divergence, unlike C2's null result. Traced
  `ftDisplayMainProcDisplay`'s `gDPSetRenderMode(G_RM_FOG_PRIM_A,
  G_RM_AA_ZB_OPA_SURF2)` -- ORs to `Z_CMP | Z_UPD | ZMODE_OPA` -- at the same
  call site RE-241/242 found for `G_LIGHTING`. Named `InitialMaterial::
  FIGHTER_EXTERNAL` bundling both signals, applied to `fighter_skeleton_
  graphs`; measurably not redundant (732/771, 95%, fighter-skeleton
  primitives flip). After seeding, archive-wide `depth_test`/`depth_write`
  reached only 1324/1308 -- the remaining ~4468-primitive gap RE-245 has now
  begun to close. `pack.rs` gained `flags::{DEPTH_TEST, DEPTH_WRITE,
  DEPTH_MODE_BIT0, DEPTH_MODE_BIT1}` (`VERSION` 27->28).
- Previously complete: `RE-243` (2026-09-11) -- `R2.2`/C2's remainder and
  closes C2. Wired `fighter::common_parts()` (RE-242) into `mesh::
  convert_sequence`'s `initial_lit: bool` seed for a fighter's two skeleton
  graphs, matching `ftDisplayMainProcDisplay`'s external `G_LIGHTING`.
  Measured **zero** of 10,958 vertices across 37 distinct skeleton graphs
  change `lit` state: RE-105's existing `G_MW_LIGHTCOL` in-list handler
  already resolves every one -- a genuine, permanently-censused null
  result, not a wiring bug. `assets/generated/ssb64.pak` rebuilt
  byte-identical.
- Previously complete: `RE-242` (2026-09-11) -- `R2.2`/C2, continuing
  RE-241's remainder. Found `fighter::common_parts()` already reads the
  archive mesh/costume-file-to-fighter mapping RE-241 said this project
  lacked; cross-checked against all 27 fighters via the decompilation's own
  `*Model.c` filenames (`tools/fighter-model-ground-truth.py`).
- Next: `R2.2`/C3 continues. Remaining sub-problems, in order:
  1. Find (or rule out) any other object category's own external
     `G_SETRENDERMODE`/geometry-mode wrapper -- render layers 0/2/3 need
     none (their own wrapper already matches `InitialMaterial::default`);
     items and effects were read directly and their normal in-game
     `ProcDisplay`s carry no such wrapper at all, so the remaining
     3891-primitive archive-wide gap is not yet attributed to a specific
     cause. Until this is understood, do not change `psp/src/meshdraw.rs`'s
     depth-test signal away from `z_buffer`.
  2. Give `PlannedList` its own `list_id` field (`scene::DlLink::list_id`
     is parsed but discarded today) so stage render-layer 1's 21 list-1
     (translucent) `DObjDLLink` entries can be seeded with depth-test-
     without-write instead of today's uniform opaque write-on seed.
  3. Once the real per-primitive `depth_test`/`depth_write` state is fully
     accounted for archive-wide, wire `psp/src/meshdraw.rs`'s
     `apply_material` to use it: keep `GuState::DepthTest` correctness at
     least as good as today's `z_buffer` heuristic, and map `depth_write`
     through `sceGuDepthMask` (`true` disables PSP writes -- note the
     inversion at the call site). Add a depth-test/no-write translucent-
     front/opaque-behind scene and an ON->OFF->ON switch regression, with
     PPSSPP or physical-PSP evidence, per `PLAN.md`'s C3 acceptance
     criteria. `depth_mode` (`ZMode`) has no PSP GE equivalent hardware
     feature identified yet; kept as measured data only.
- Blockers: none for what RE-245 closed (it does not close C3). Same
  non-blocking follow-ups RE-240/RE-241/RE-242/RE-243 already recorded
  remain open (visual before/after for RE-240's 23 affected files; the
  `debug_overlay` HUD text bug, `task_1bf9bc35`; RE-224's TEXVIEW
  screenshot; RE-228's clamp-boundary deviation; RE-214/RE-236's known
  screenshot noise floor; R2.1/T1's 164 cross-node differing-transform
  vertex reuses) -- none are new, see prior snapshot history for detail.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-245.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`COMPLETE`, T1-T10 all
  terminal); `R2.2` (`IN_PROGRESS`, C1 `COMPLETE`, C2 `COMPLETE`, C3
  `IN_PROGRESS`, C4-C7 remain).
- Decisions: `DECISIONS.md` -- no new revision for RE-245 (D-042 already
  covers "renderer correctness claims stay provisional until R2.2 closes").
- Subsystem: `docs/porting-status.md` -- updated the "Mesh conversion" row:
  stage render-layer 1 now carries the same measured external depth seed
  fighters do, shrinking the archive-wide `z_buffer`/`depth_test` gap from
  4468 to 3891; the remainder (layers 0/2/3, items, effects) and PSP-side
  wiring remain open.
- Verification (RE-245): `cargo test --workspace --all-targets` (`SSB64_ROM`
  set): `ssb-rom` 418 passed (417 prior + 1 new unit test), `romtool` 21
  passed (19 prior + 2 new census tests), `ssb-engine` 48, `ssb-game` 120,
  0 failed overall. `cargo fmt --check` clean. `cargo clippy --workspace
  --all-targets` clean (same pre-existing, unrelated warnings as prior
  sessions). Pack rebuilt (`romtool pack`): mesh/triangle/draw-call counts
  verified identical to a pre-change rebuild of the same ROM (checksum
  itself differs, as expected -- real primitive material state changed).
  Code changed: `crates/ssb-rom/src/mesh.rs` (`InitialMaterial::
  GROUND_LAYER1_EXTERNAL`, 1 new unit test), `tools/romtool/src/main.rs`
  (`ground_layer1_graphs`, `initial_material_for` signature, all call
  sites, 2 new census tests).
- Documentation: RE-245, `PLAN.md` (`R2.2`/C3 status, cross-reference
  table, acceptance checklist), `docs/porting-status.md`, this snapshot.
- Commit: pending (RE-245, `R2.2`/C3 part 2: stage render-layer-1 external
  depth seed).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
