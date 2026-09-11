# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.2 — Second Renderer Corrective Gate (C1-C7)` (`IN_PROGRESS`)
- Status: `IN_PROGRESS` -- C1 (`COMPLETE`); C2 (`COMPLETE`); C3 (`IN_PROGRESS`,
  part 3 of an unknown number, RE-244/RE-245/RE-246); C4-C7 remain
- Last complete: `RE-246` (2026-09-11) -- `R2.2`/C3 part 3: the dominant
  remainder of RE-244/245's depth-state gap was not an object-category
  wrapper at all, but a *camera*-level default. Broke the archive-wide gap
  down by file id (a temporary diagnostic, not kept) rather than continuing
  to guess by object category: files 39-51 -- RE-099/100's already-known 13
  loading-break transition scenes -- diverged on effectively 100% of their
  own primitives, ~2500 of the 3891-primitive gap. Reading `refs/ssb-decomp-
  re/src/lb/lbtransition.c` directly found `lbTransitionProcDisplay` issues
  no render-mode call of its own -- just `gSPSegment` then
  `gcDrawDObjTreeForGObj`. Instead, `sys/objdisplay.c`'s `func_8001663C`,
  called first by every camera's per-frame display driver
  (`func_80017D3C`) before it walks that camera's own tagged `GObj` list,
  unconditionally sets `gDPSetRenderMode(G_RM_AA_ZB_OPA_SURF, ...)` --
  `Z_CMP | Z_UPD | ZMODE_OPA` -- for a buffer-0 camera, regardless of
  `COBJ_FLAG_ZBUFFER` (a separate depth-image-clear flag). `lbTransition
  MakeCamera` creates a dedicated buffer-0 camera carrying only the
  transition's own node list, so this camera-level default reaches its
  primitives uncorrupted -- unlike the general case, where which object a
  shared camera draws first is runtime, game-state-dependent, and not
  archive-attributable (the same shape `C4`'s submission-order concern
  covers). Added `InitialMaterial::LB_TRANSITION_EXTERNAL` (`lit: false`)
  and `tools/romtool`'s `lb_transition_graphs()`, built directly from the
  already-existing `ssb_rom::transition::ASSETS` (11 `dLBTransitionDescs`
  entries recovered for `R0.13`, no new discovery needed), wired into
  `initial_material_for` and all five production call sites. Measured
  (`census_lb_transition_seed_measured_impact`, kept permanently): of 1951
  primitives across the 11 named graphs, the seed flips **1951 (100%)**.
  Archive-wide, `depth_test`/`depth_write` rise from 1901/1885 to
  **3852/3836**; the `z_buffer`-vs-`depth_test` gap falls from 3891 to
  **1940** -- more than half of RE-244's original 4468-primitive gap from
  one seed. This camera-level mechanism is deliberately **not**
  generalized to the main battle camera or any other shared camera. Two
  files RE-099 already flagged as outside `dLBTransitionDescs` (39, 51)
  remain unattributed. `psp/src/meshdraw.rs` is **deliberately unchanged**
  -- still keys `GuState::DepthTest` off `z_buffer` (RE-068,
  device-validated). `assets/generated/ssb64.pak` rebuilt; mesh/triangle/
  object counts (2044/28993/top-5 list) verified identical to a pre-change
  rebuild of the same ROM (checksum itself differs, as expected). See
  `docs/reverse-engineering.md` RE-246 for the full entry.
- Previously complete: `RE-245` (2026-09-11) -- `R2.2`/C3 part 2: found and
  wired the same external depth-state wrapper C1/C2/C3-part-1 found for
  fighters, but for stage geometry (`grDisplayLayerNPriProcDisplay`/
  `SecProcDisplay`, varying by layer index -- only layer 1 needed
  `InitialMaterial::GROUND_LAYER1_EXTERNAL`). Flipped 577/776 (74%) of
  layer-1 primitives, shrinking the archive-wide gap from 4468 to 3891.
  Also read `it`/`ef` directly: their normal `ProcDisplay`s carry no
  external wrapper at all, unlike `ft`/`gr`. Left open: `PlannedList`
  discards `list_id`, so layer 1's 21 list-1 (translucent) entries are
  still seeded as opaque write-on.
- Previously complete: `RE-244` (2026-09-11) -- `R2.2`/C3 part 1: independent
  depth compare/write state data model (`MeshMaterial::{depth_test,
  depth_write, depth_mode: ZMode}`), plus the same external-per-object seed
  mechanism C2 used, but for `G_SETRENDERMODE` this time
  (`InitialMaterial::FIGHTER_EXTERNAL`). Archive-wide before any seed: of
  5872 primitives, `z_buffer` true for 5792 (98.6%) but `depth_test`/
  `depth_write` only 592/576 (10%) -- a real 90% divergence, unlike C2's
  null result. Measurably not redundant (732/771, 95%, fighter-skeleton
  primitives flip). `pack.rs` gained `flags::{DEPTH_TEST, DEPTH_WRITE,
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
- Next: `R2.2`/C3 continues. Remaining sub-problems, in order:
  1. Two files RE-099 flagged outside `dLBTransitionDescs` (39, 51) still
     carry unexplained depth-state divergence -- confirm their real draw
     path (file 39 is `IFCommonObject`, possibly `ifScreenFlashProcDisplay`
     or a similar `if`-prefixed wrapper, not yet traced) before seeding
     them one way or the other.
  2. Find (or rule out) any other object category's or camera's own
     external wrapper -- render layers 0/2/3 need none; items and effects
     carry no wrapper of their own; the remaining 1940-primitive
     archive-wide gap is not yet attributed to a specific cause, and part
     of it may be the main battle camera's own first-drawn-object default
     (RE-246), which is not statically attributable the way the transition
     cameras were. Until this is understood, do not change `psp/src/
     meshdraw.rs`'s depth-test signal away from `z_buffer`.
  3. Give `PlannedList` its own `list_id` field (`scene::DlLink::list_id`
     is parsed but discarded today) so stage render-layer 1's 21 list-1
     (translucent) `DObjDLLink` entries can be seeded with depth-test-
     without-write instead of today's uniform opaque write-on seed.
  4. Once the real per-primitive `depth_test`/`depth_write` state is fully
     accounted for archive-wide, wire `psp/src/meshdraw.rs`'s
     `apply_material` to use it: keep `GuState::DepthTest` correctness at
     least as good as today's `z_buffer` heuristic, and map `depth_write`
     through `sceGuDepthMask` (`true` disables PSP writes -- note the
     inversion at the call site). Add a depth-test/no-write translucent-
     front/opaque-behind scene and an ON->OFF->ON switch regression, with
     PPSSPP or physical-PSP evidence, per `PLAN.md`'s C3 acceptance
     criteria. `depth_mode` (`ZMode`) has no PSP GE equivalent hardware
     feature identified yet; kept as measured data only.
- Blockers: none for what RE-246 closed (it does not close C3). Same
  non-blocking follow-ups RE-240/RE-241/RE-242/RE-243/RE-245 already
  recorded remain open (visual before/after for RE-240's 23 affected
  files; the `debug_overlay` HUD text bug, `task_1bf9bc35`; RE-224's
  TEXVIEW screenshot; RE-228's clamp-boundary deviation; RE-214/RE-236's
  known screenshot noise floor; R2.1/T1's 164 cross-node differing-
  transform vertex reuses) -- none are new, see prior snapshot history for
  detail.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-246.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`COMPLETE`, T1-T10 all
  terminal); `R2.2` (`IN_PROGRESS`, C1 `COMPLETE`, C2 `COMPLETE`, C3
  `IN_PROGRESS`, C4-C7 remain).
- Decisions: `DECISIONS.md` -- no new revision for RE-246 (D-042 already
  covers "renderer correctness claims stay provisional until R2.2 closes").
- Subsystem: `docs/porting-status.md` -- updated the "Mesh conversion" row:
  the 11 loading-break transition scenes now carry a camera-level external
  depth seed, shrinking the archive-wide `z_buffer`/`depth_test` gap from
  3891 to 1940; the remainder (layers 0/2/3, items, effects, 2 unattributed
  transition-adjacent files, and any statically-visible fraction of the
  main-camera default) and PSP-side wiring remain open.
- Verification (RE-246): `cargo test --workspace --all-targets` (`SSB64_ROM`
  set): `ssb-rom` 419 passed (418 prior + 1 new unit test), `romtool` 22
  passed (21 prior + 1 new census test), `ssb-engine` 48, `ssb-game` 120,
  0 failed overall. `cargo fmt --check` clean. `cargo clippy --workspace
  --all-targets` clean (same pre-existing, unrelated warnings as prior
  sessions). Pack rebuilt (`romtool pack`): mesh/triangle/object counts
  verified identical to a pre-change rebuild of the same ROM (checksum
  itself differs, as expected -- real primitive material state changed).
  Code changed: `crates/ssb-rom/src/mesh.rs` (`InitialMaterial::
  LB_TRANSITION_EXTERNAL`, 1 new unit test), `tools/romtool/src/main.rs`
  (`lb_transition_graphs`, `initial_material_for` signature, all five call
  sites, 1 new permanent census test).
- Documentation: RE-246, `PLAN.md` (`R2.2`/C3 status, cross-reference
  table, acceptance checklist), `docs/porting-status.md`, this snapshot.
- Commit: `d84cbf8` (RE-246, `R2.2`/C3 part 3: camera-level depth seed for
  loading-break transitions).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
