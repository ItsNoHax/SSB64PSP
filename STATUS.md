# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.2 — Second Renderer Corrective Gate (C1-C7)` (`IN_PROGRESS`)
- Status: `IN_PROGRESS` -- C1 (`COMPLETE`); C2 (`COMPLETE`); C3 (`IN_PROGRESS`,
  part 1 of an unknown number, RE-244); C4-C7 remain
- Last complete: `RE-244` (2026-09-11) -- `R2.2`/C3 part 1: independent
  depth compare/write state data model, plus the same external-per-object
  seed mechanism C2 used, but for `G_SETRENDERMODE` this time. Added
  `MeshMaterial::{depth_test, depth_write, depth_mode: ZMode}`, derived from
  `G_SETRENDERMODE`'s `Z_CMP`/`Z_UPD`/`ZMODE` bits at the same render-mode
  site `alpha_test`/`translucent` already read; `z_buffer` (`G_ZBUFFER`) is
  untouched, still RSP geometry-mode state, not the RDP's own per-pixel
  decision. Archive-wide before any seed: of 5872 primitives, `z_buffer` is
  true for 5792 (98.6%) but `depth_test`/`depth_write` only 592/576 (10%) --
  a real 90% divergence (`census_independent_depth_state_vs_z_buffer_
  geometry_bit`, kept permanently), unlike C2's null result. Reading
  `ftdisplaymain.c` directly found `ftDisplayMainProcDisplay` also issues
  `gDPSetRenderMode(G_RM_FOG_PRIM_A, G_RM_AA_ZB_OPA_SURF2)` -- ORs to
  `Z_CMP | Z_UPD | ZMODE_OPA` -- at the same call site RE-241/242 found for
  `G_LIGHTING`. Generalized `mesh::State::new`'s `initial_lit: bool` into
  `InitialMaterial { lit, depth_test, depth_write, depth_mode }`, with a
  named `InitialMaterial::FIGHTER_EXTERNAL` constant bundling both signals,
  applied to the same `fighter_skeleton_graphs` scope C2 validated. Unlike
  C2, **not redundant**: `census_depth_seed_measured_impact_on_skeleton_
  graphs` (kept permanently) found the seed flips 732/771 (95%) of fighter-
  skeleton primitives from false to true. After seeding, archive-wide
  `depth_test`/`depth_write` reach only 1324/1308 -- still far short of
  `z_buffer`'s 5792; the remaining ~4468-primitive gap is non-fighter-
  skeleton geometry (stage/effects/other) with an as-yet-unlocated external
  wrapper of its own, the same open shape as C2's "other graphs"
  `looks_like_unit_normal` remainder. `pack.rs` gained `flags::{DEPTH_TEST,
  DEPTH_WRITE, DEPTH_MODE_BIT0, DEPTH_MODE_BIT1}` (`VERSION` 27->28),
  recorded for inspection only. `psp/src/meshdraw.rs` is **deliberately
  unchanged** -- still keys `GuState::DepthTest` off `z_buffer` (RE-068,
  device-validated); switching to `depth_test` now would regress the
  unexplained ~4468 primitives. `assets/generated/ssb64.pak` rebuilt; mesh/
  triangle/draw-call counts (2044/36772/8056) verified identical to a
  pre-change rebuild of the same ROM. See `docs/reverse-engineering.md`
  RE-244 for the full entry.
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
- Previously complete: `RE-241` (2026-09-11) -- `R2.2`/C2 part 1: fixed a
  real timing bug (vertex normal-vs-colour meaning now fixed at `G_VTX`
  load time, not triangle-draw time) and traced `ftDisplayMainProcDisplay`'s
  unconditional external `G_LIGHTING` as the cause of RE-021's remaining
  `looks_like_unit_normal` fallback gap.
- Next: `R2.2`/C3 continues. Two remaining sub-problems, in order:
  1. Find (or rule out) the external `G_SETRENDERMODE` wrapper for
     non-fighter-skeleton object categories (stage, effects, particles,
     UI/HUD) that RE-244's ~4468-primitive archive-wide gap implies must
     exist, the same way `ftDisplayMainProcDisplay` did for fighters. Until
     this is understood, do not change `psp/src/meshdraw.rs`'s depth-test
     signal away from `z_buffer`.
  2. Once the real per-primitive `depth_test`/`depth_write` state is fully
     accounted for archive-wide, wire `psp/src/meshdraw.rs`'s
     `apply_material` to use it: keep `GuState::DepthTest` correctness at
     least as good as today's `z_buffer` heuristic, and map `depth_write`
     through `sceGuDepthMask` (`true` disables PSP writes -- note the
     inversion at the call site). Add a depth-test/no-write translucent-
     front/opaque-behind scene and an ON->OFF->ON switch regression, with
     PPSSPP or physical-PSP evidence, per `PLAN.md`'s C3 acceptance
     criteria. `depth_mode` (`ZMode`) has no PSP GE equivalent hardware
     feature identified yet; kept as measured data only.
- Blockers: none for what RE-244 closed (it does not close C3). Same
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
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-244.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`COMPLETE`, T1-T10 all
  terminal); `R2.2` (`IN_PROGRESS`, C1 `COMPLETE`, C2 `COMPLETE`, C3
  `IN_PROGRESS`, C4-C7 remain).
- Decisions: `DECISIONS.md` -- no new revision for RE-244 (D-042 already
  covers "renderer correctness claims stay provisional until R2.2 closes").
- Subsystem: `docs/porting-status.md` -- updated the "Mesh conversion" row:
  independent `depth_test`/`depth_write`/`depth_mode` fields and the
  fighter-skeleton external-render-mode seed are in place and measured, but
  the wider archive gap and PSP-side wiring remain open.
- Verification (RE-244): `cargo test --workspace --all-targets` (`SSB64_ROM`
  set): `ssb-rom` 417 passed (414 prior + 3 new unit tests), `romtool` 19
  passed (17 prior + 2 new census tests), `ssb-engine` 48, `ssb-game` 120,
  0 failed overall. `cargo fmt --check` clean. `cargo clippy --workspace
  --all-targets` clean (same pre-existing, unrelated warnings as prior
  sessions). Pack rebuilt (`romtool pack`): mesh/triangle/draw-call counts
  verified identical to a pre-change rebuild of the same ROM (checksum
  itself differs, as expected -- real primitive material state changed).
  Code changed: `crates/ssb-rom/src/mesh.rs` (`ZMode`, `InitialMaterial`,
  `MeshMaterial::{depth_test, depth_write, depth_mode}`, derivation, 3 new
  unit tests), `crates/ssb-rom/src/pack.rs` (`flags::{DEPTH_TEST,
  DEPTH_WRITE, DEPTH_MODE_BIT0, DEPTH_MODE_BIT1}`, `VERSION` 27->28),
  `tools/romtool/src/main.rs` (`initial_material_for`, all `convert_
  sequence` call sites, 2 new census tests).
- Documentation: RE-244, `PLAN.md` (`R2.2`/C3 status, cross-reference
  table, acceptance checklist), `docs/porting-status.md`, this snapshot.
- Commit: `e53de84` (RE-244, `R2.2`/C3 part 1: independent depth state
  data model + fighter external-render-mode seed).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
