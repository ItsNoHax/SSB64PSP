# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.2 — Second Renderer Corrective Gate (C1-C7)` (`IN_PROGRESS`)
- Status: `IN_PROGRESS` -- C1 (`COMPLETE`); C2 (`COMPLETE`); C3 (`COMPLETE`,
  RE-244 through RE-251, 8 parts); C4 (`COMPLETE`, RE-252/RE-253); C5-C7
  remain
- Last complete: `RE-253` (2026-09-12) -- found and fixed while verifying
  RE-252's own golden-scene impact, not a separately-planned task.
  `tools/romtool/src/main.rs`'s texture cache key (`TexKey`) omitted
  `width`/`height`/`drawn_width`/`drawn_height`/`format`/`size`/
  `palette_entries`/`palette` -- all fields `convert_texture` actually bakes
  into a texture's bytes -- so two primitives sharing one base
  image+palette+wrap but drawing a *different* crop of it (ordinary for a
  shared character texture sheet) silently shared one cache entry;
  whichever primitive converted first "won" and every other primitive got
  its crop instead. Found because rebuilding the pack after RE-252's own
  submission-order fix changed 11 of 15 golden scenes far more than a pure
  draw-order fix should (up to 6940 pixels on Fox's fighter scene, ears
  flipping brown-to-white) -- the reorder was just picking a different
  arbitrary "first" among keys that were already colliding, not a new
  problem. A first fix attempt keyed on the *entire* `TextureRef` struct
  (reasoning from `merge_by_material`'s own whole-struct key, RE-252) and
  overcorrected: `origin_s`/`origin_t`/`mask_s`/`mask_t`/`framebuffer` are
  real fields but `convert_texture` never reads them (draw-time-only
  inputs), so including them inflated the archive-wide texture count from
  1345 to 2011 (+49.5%) instead of closing the real gap; reverted in favor
  of the exhaustive-but-hand-picked field set `convert_texture` actually
  depends on. Also caught its own second wrong turn: assumed `format`/
  `size`/`palette_entries`/`palette` were safe to leave out ("same ROM
  address always decodes the same way") without measuring it -- a new
  permanent test,
  `texture_key_fields_never_vary_for_a_fixed_data_and_palette_location`
  (`SSB64_ROM`-gated), measured that assumption **false** (1 real
  format/size conflict, 46 real palette-shape conflicts archive-wide), so
  all four are in the final key too. `TexKey` is now a named 12-field
  struct (was a tuple; this field count exceeds Rust's blanket-impl arity
  of 12 for tuples). Verified archive-wide: textures 1345->**1762**
  (+417, +31.0%, measured identical regardless of whether RE-252's own
  mesh-order fix is applied, confirming the key is now genuinely
  order-independent); pack size 11422.3->**25639.3 KiB** (+124.5%, a large,
  expected jump from no longer collapsing real crop/format variants --
  flagged below as a new follow-up, PSP RAM headroom not yet checked). With
  both fixes applied, all 15 golden scenes are byte-identical to their own
  pre-RE-252 counterparts -- RE-252's fix has zero measured visual effect on
  the current corpus, so no goldens needed refreshing this session. See
  `docs/reverse-engineering.md` RE-253 for the full entry.
- Previously complete: `RE-252` (2026-09-12) -- `R2.2`/C4, **closes C4**:
  RE-217 had already flagged `merge_by_material`'s `BTreeMap<MeshMaterial,
  _>` grouping as a risk (can turn a submitted `A B A` material sequence
  into `A A B`, moving triangles across whatever a `B` primitive would
  test/blend against). Confirmed directly: `walk`'s own `Builder::flush`
  only pushes a primitive when the material actually *changes*, so
  consecutive primitives never already share a material -- meaning
  `merge_by_material`'s only real effect was reordering *non-adjacent* runs,
  never a true "coalesce adjacent duplicates" optimisation in practice.
  Rewrote it to merge only when the immediately preceding output primitive
  already has the same material, never regrouping across a gap; given the
  above invariant this is normally a no-op today, but it removes the
  archive-wide reordering and guards against a future change reintroducing
  it. Fixed two stale doc comments (`MeshMaterial`'s own "primitives are
  grouped by this key" claim; a test comment making the same claim) that
  described the old, now-wrong behaviour as current. Test
  `material_change_splits_then_merges_back` (which asserted the *old*, wrong
  `A B A` -> 2-primitives behaviour) renamed to
  `non_adjacent_same_material_runs_stay_separate_and_in_order` and flipped
  to assert 3 primitives in draw order; added
  `adjacent_same_material_runs_merge` to exercise the merge path directly,
  since the real pipeline no longer naturally produces adjacent duplicates
  to hit it through `convert`. Measured draw-call growth archive-wide: +89
  (`romtool mesh`, 3279->3368) to +113 (`romtool pack`, real pipeline,
  8056->8169, isolated from RE-253's own effect by holding its `TexKey` fix
  constant on both sides) -- recorded as an `R3` (Rendering Performance)
  lead, not a regression to chase. See `docs/reverse-engineering.md` RE-252
  for the full entry.
- Previously complete: `RE-251` (2026-09-11) -- `R2.2`/C3 part 8, **closes C3**:
  wired `psp/src/meshdraw.rs`'s `apply_material` to the independent
  `depth_test`/`depth_write` state RE-244-250 built and fully seeded,
  superseding the interim `z_buffer`-keyed `GuState::DepthTest` proxy
  (RE-068). Replaced the `flags::Z_BUFFER` check with `flags::DEPTH_TEST`
  and added `sys::sceGuDepthMask(if flags::DEPTH_WRITE { 0 } else { 1 })`
  (the argument is inverted from the source bit's own sense), both inside
  the same `st.last_flags` change-detection block `DepthTest` already used.
  Measured the golden-scene impact against a same-environment pre/post
  rebuild (not the possibly-stale committed PNGs directly -- see this
  entry's own environment-drift note): 9 of the 13 existing
  `regression_capture_scene{2..14}` goldens are byte-identical (confirmed
  directly for Fox's graph: all 30 packed primitives already read
  `z_buffer == depth_test == depth_write == true`); the other 4 (Dream
  Land, all three metal-texgen scenes) change by 220-13,008 pixels, each
  visually localized to overlapping translucent/reflective content (a
  canopy highlight triangle, a hull texture patch, one crystal facet's
  occlusion) and explainable as a depth-order correction, not corruption --
  refreshed after inspection. No existing golden's frozen camera puts a
  depth-test-without-write surface in front of later-drawn geometry at an
  overlapping screen position (confirmed even for stage 7/Sector Z,
  deliberately chosen for having the most layer-1 list-1 content of any
  stage but still byte-identical before/after), so added a synthetic,
  non-ROM `depth_mask_diagnostic` scene (`psp/src/depth_diag.rs`, same
  shape as `normal_diag.rs`'s `texgen_normal_diagnostic_*` rig): three
  overlapping quads -- opaque red far (write on), translucent green near
  (write **off**), opaque blue between the two in depth drawn last (write
  on again, the ON->OFF->ON switch) -- proving a depth-test-without-write
  surface does not block a later opaque draw the way a write-enabled one
  would (real-time translucency compositing, RE-244's `ZMODE_XLU`).
  Self-validated by temporarily forcing the green quad's write flag on
  (simulating a broken wire): the expected nested blue disc vanished
  entirely on a PPSSPP headless capture, confirming the regression actually
  discriminates; reverted, and the fixed build is deterministic (0
  differing pixels across two captures). New golden:
  `tests/golden/r2-depth-mask-diagnostic.png`. `PLAN.md` C3's acceptance
  line accepts PPSSPP *or* physical-PSP evidence; physical hardware was
  enumerated over USB but not in an active PSPLink session this session
  could establish (launching PSPLink from the device's own XMB needs
  physical interaction), so this closes C3 on PPSSPP evidence alone,
  leaving a hardware pass open for later. Also found and deliberately left
  open (not fixed by this entry): comparing rebuilt headless captures
  directly against the *committed* golden PNGs shows large diffs
  (tens of thousands of pixels) even for a pristine pre-change rebuild with
  zero code changes -- a pre-existing environment/toolchain drift between
  whatever produced the currently-committed goldens and this session's
  PPSSPP headless binary, unrelated to this change, caught only because the
  same-environment pre/post comparison above isolates the real effect. See
  `docs/reverse-engineering.md` RE-251 for the full entry.
- Previously complete: `RE-250` (2026-09-11) -- `R2.2`/C3 part 7: gave `PlannedList`
  its own `list_id` field (populated from each `DlLink`'s own `list_id`,
  which `scene::DlLink` already parsed but flattening discarded) and a new
  `SequenceItem::depth_seed` per-item override in `convert_sequence`, applied
  immediately before that item's own commands run and overriding whatever
  the previous item left behind -- unlike every other piece of `State` in a
  sequence, which genuinely does carry across. This models `refs/ssb-decomp-
  re/src/gr/grdisplay.c`'s `grDisplayLayer1{Pri,Sec}ProcDisplay`: it opens
  two separate `gSYTaskmanDLHeads[N]` task-list command streams and sets
  each one's own render mode before either is walked (`Z_CMP | Z_UPD |
  ZMODE_OPA` on list 0, `Z_CMP | !Z_UPD | ZMODE_XLU` on list 1), so a list-1
  `DObjDLLink` entry's real depth state never inherits from whatever list-0
  entry preceded it in `plan_draw_order`'s flattened draw order. New
  `tools/romtool::ground_layer1_list1_depth_seed(initial, list_id)` returns
  the corrective seed exactly when a graph is `GROUND_LAYER1_EXTERNAL` and
  the entry's `list_id == Some(1)`; wired into all three production/
  diagnostic `SequenceItem` builders (`pack`, `scene`, `file_meshes`) and
  `convert_graph_at`. New unit test `convert_sequence_depth_seed_overrides_
  state_inherited_from_a_prior_item` (`mesh.rs`) proves the override wins
  over inherited state rather than merging with it. Re-measured
  (`census_ground_layer_depth_state_vs_z_buffer`, kept permanently): layer
  1's `depth_write`-true count falls from 776/776 to **668/776** -- the
  other 108, from 21 of 163 `DObjDLLink` entries targeting list 1, now
  correctly read `depth_write` false; layers 0/2/3 and `links_list0`/
  `links_list1` counts (121/31, 142/21, 0/0, 1/1) are unchanged, confirming
  the fix is scoped to exactly the list-1 population RE-245/RE-249 already
  measured. `romtool pack` against the real ROM: mesh/triangle/draw/
  texture/object counts unchanged (2044/36772/8056/1345/374), transitions
  unchanged (77 animated nodes), pack size unchanged (10935.5 KiB) -- this
  only corrects `depth_write`/`depth_mode` values already captured by the
  existing `pack.rs` flags, not the pack's structure. This closes `PLAN.md`
  C3's remaining-work item 1 (the `list_id`/`PlannedList` gap); item 2
  (PSP-side `sceGuDepthMask` wiring and the depth-test/no-write regression
  scene with device evidence) remains open. See `docs/reverse-engineering.md`
  RE-250 for the full entry.
- Previously complete: `RE-249` (2026-09-11) -- `R2.2`/C3 part 6: ruled out any
  further external depth-state wrapper archive-wide -- the "find (or rule
  out)" half of C3's remaining item 1 is now answered. A per-file
  breakdown of the 1623 primitives RE-248 left unattributed found no
  dominant cluster (largest single file, 40/`LBTransitionAeroplane`, is
  only 3.3% of it -- nothing like RE-246's 2500/3891-primitive find).
  `grep`ing the breakdown against `relocFileDescriptions.us.txt`'s names
  did surface one real pattern: 37 `Stage*File2+`/`GRBonus*File2` files
  (each its own independent `GroundData` `find_ground_data` already
  discovers) account for 617 primitives (34%); no primary, non-`File2+`
  stage file appears in the divergence list at all. Traced file 112
  (`StageYamabukiFile2`) directly: its layer 1 (already seeded
  `GROUND_LAYER1_EXTERNAL` by RE-245, since the seed keys on layer *index*
  not file) shows **zero** divergence, while its default-seeded layers 0/3
  diverge **100%** -- and since `convert_sequence` only ever sets
  `z_buffer` from a seed or a decoded `G_SETGEOMETRYMODE` command, a `true`
  result from a `false` seed can only be the graph's *own* in-list
  `G_SETGEOMETRYMODE(G_ZBUFFER)` call, not a missing wrapper. Spot-checked
  six more non-cluster divergers (`KirbyModel`, `LinkModel`, `NessModel`,
  `CaptainSpecial2`, `FoxSpecial3`, `ITCommonObject`, `MVCommon`, none
  `fighter_skeleton_graphs()` members): same small, scattered, per-node
  shape, no shared wrapper. File 47 (already confirmed unregistered by
  RE-247) closes the same way RE-248 closed file 39 -- its 45 primitives
  need no seed either. **Conclusion**: the remaining ~1578 primitives
  (1808 minus files 39 and 47's 230) are real archive content -- the
  original ROM's own display lists setting `G_ZBUFFER` without a matching
  `Z_CMP`/`Z_UPD` bit -- exactly the corrective case C3 exists to capture,
  not an unfound seed. No further wrapper search is warranted. See
  `docs/reverse-engineering.md` RE-249 for the full entry (confidence:
  high for the rule-out itself, medium for generalizing the per-node
  shape to the untraced remainder).
- Previously complete: `RE-248` (2026-09-11) -- `R2.2`/C3 part 5: closed file 39
  (`IFCommonObject`), RE-247's one remaining unattributed file. Its own
  decomp header comment (`refs/ssb-decomp-re/src/relocData/
  39_IFCommonObject.c`) states it is orphaned interface geometry with no
  code path referencing it beyond its FileID extern. `grep -rn
  llIFCommonObjectFileID refs/ssb-decomp-re/src/` returns zero matches --
  confirmed by reading both `if/ifcommon.c` and `if/ifscreenflash.c` in
  full: every real `ProcDisplay` there either draws inline `Gfx` (no
  `DObjDesc` graph at all) or walks a *different* named `dIFCommon*`
  resource (damage/stock/timer/tag/arrows/pause HUD elements); none
  reference file 39's own symbols. Same shape as RE-247's unregistered file
  47, not the same shape as the transition scenes (each named by
  `dLBTransitionDescs`). Measured via a temporary probe (mirroring
  `census_independent_depth_state_vs_z_buffer_geometry_bit`, filtered to
  file 39, reverted after use): file 39 decodes to exactly 185 primitives,
  100% diverging (`z_buffer`=true, `depth_test`/`depth_write`=false for
  all). No seed applies -- there is no `ProcDisplay` to seed. This explains
  185 of the 1808-primitive archive-wide gap without any code change (no
  path draws file 39, so `romtool pack`/runtime are already correct to
  never render it). Remaining gap: ~1623 primitives (layers 0/2/3, items,
  effects, main-camera default). See `docs/reverse-engineering.md` RE-248
  for the full entry.
- Previously complete: `RE-247` (2026-09-11) -- `R2.2`/C3 part 4: tracing RE-246's
  two unattributed files (39, 51) found `ssb_rom::transition::ASSETS`'s
  `"camera"` entry pointed at the **wrong file** -- a real pack-content bug,
  not just a depth-state attribution gap. `dLBTransitionDescs`'s eighth
  entry ("Camera Shutter") resolves via `refs/ssb-decomp-re/symbols/
  reloc_data_symbols.us.txt` to `llLBTransitionCameraFileID = 0x33` (file
  **51**, not 47) with `DObjDesc`/`AnimJoint` offsets `0x3f90`/`0x4148` --
  both matching file 51's own declarations exactly. `ASSETS` instead had
  `file: 47, graph: 0x0F98`, silently pointing at file 47's own unrelated
  (and unregistered -- no other symbol anywhere references it) paper-
  airplane scene, which happened to validate structurally at that offset
  the same way `gakubuthi`/`rot_scale` coincidentally reuse it. Confirmed
  against the real ROM: file 51 graph `0x3F90` has 9 nodes, 8 scripts, 64
  frames, matching its own 8-panel camera-booth `DObjDesc`. Fixed
  `crates/ssb-rom/src/transition.rs`'s `"camera"` entry (`file`/`graph`/
  `anim_joints`) and replaced the unit test's blanket `40 + index`
  assertion with an explicit `EXPECTED_FILES` array so this can't silently
  regress. `ASSETS` is consumed by `romtool pack`'s live results-screen-
  wipe path (`psp/src/results_transition.rs`, RE-146/147, already shipped)
  -- before this fix the "Camera Shutter" wipe packed and would have drawn
  the wrong (paper-airplane) geometry with only 1 animated joint instead of
  the real 8-panel booth with 8. `romtool pack`: mesh/triangle/draw/texture/
  object counts unchanged (2044/36772/8056/1345/374); `transitions`
  animated-node count rises 70->**77**; pack size 10922.8->**10935.5 KiB**.
  Side effect on C3's own depth census:
  `census_lb_transition_seed_measured_impact` rises 1951/1951->**2083/2083**
  (100%); archive-wide `z_buffer`-vs-`depth_test` gap falls from 1940 to
  **1808** (`z_buffer=5792`, `depth_test=3984`, `depth_write=3968`). File 39
  (`IFCommonObject`) remains genuinely unattributed -- not read closely
  enough yet to confirm its draw path. See `docs/reverse-engineering.md`
  RE-247 for the full entry.
- Previously complete: `RE-246` (2026-09-11) -- `R2.2`/C3 part 3: the dominant
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
- Next: `R2.2`/C4 is closed. `R2.2`/C5 -- Systematic PSP GE cache isolation --
  is the next eligible task: inventory every raw GU mutation outside
  `apply_material` (mesh, collision/debug markers, particles, wallpaper/
  framebuffer, UI/debug, fighter-light paths), record function/changed
  state/cache field/invalidation/whether a draw follows, and add
  `DrawState::invalidate_all()` or centralize mutations (`PLAN.md` C5 for
  the full description). Not yet started.
- Blockers: none. New non-blocking follow-up from this session, not yet
  investigated: RE-253's `TexKey` fix grew the real pack from 11422.3 to
  25639.3 KiB (+124.5%, 1345->1762 textures) by no longer collapsing real
  crop/format variants that were previously silently sharing (and
  corrupting) one cache entry -- expected given the fix, but real-PSP RAM
  headroom for a pack this size has not been checked this session. Prior
  non-blocking follow-ups remain open: (1) the environment/toolchain drift
  making rebuilt PPSSPP headless captures differ from committed golden PNGs
  by tens of thousands of pixels even with zero code changes (RE-251); (2)
  physical-PSP confirmation of `tests/golden/r2-depth-mask-diagnostic.png`
  (RE-251); (3) RE-240/RE-241/RE-242/RE-245's older follow-ups (visual
  before/after for RE-240's 23 affected files; the `debug_overlay` HUD text
  bug, `task_1bf9bc35`; RE-224's TEXVIEW screenshot; RE-228's
  clamp-boundary deviation; RE-214/RE-236's known screenshot noise floor;
  R2.1/T1's 164 cross-node differing-transform vertex reuses) -- none are
  new, see prior snapshot history for detail.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-253.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`COMPLETE`, T1-T10 all
  terminal); `R2.2` (`IN_PROGRESS`, C1 `COMPLETE`, C2 `COMPLETE`, C3
  `COMPLETE`, C4 `COMPLETE`, C5-C7 remain). `R0.16`'s own D-036-ordering-rule
  acceptance item is now checked off: both flagged optimizations
  (`merge_by_material`, `TexKey`) confirmed keyed on their real dependency
  set.
- Decisions: `DECISIONS.md` -- no new revision for RE-252/RE-253 (D-042
  already covers "renderer correctness claims stay provisional until R2.2
  closes"; C4 closing does not close R2.2 itself, C5-C7 remain).
- Subsystem: `docs/porting-status.md` -- updated the "Mesh conversion" row:
  `merge_by_material` now preserves submission order (RE-252, `PLAN.md`
  R2.2/C4 `COMPLETE`); `tools/romtool`'s texture cache key widened to its
  full real dependency set (RE-253).
- Verification (RE-252/RE-253): `crates/ssb-rom/src/mesh.rs`'s
  `merge_by_material` now merges only adjacent identical-material
  primitives; `tools/romtool/src/main.rs`'s `TexKey` widened from an 8-field
  tuple to a 12-field named struct. `cargo test --workspace --all-targets`
  (`SSB64_ROM` set): `ssb-rom` 421 (+1), `romtool` 23 (+1), `ssb-engine` 48,
  `ssb-game` 120 passed, 0 failed. `cargo fmt --check`/`cargo clippy
  --all-targets --release` clean (no new warnings from either changed
  file). `romtool pack` against the real ROM (both fixes applied): meshes
  2044, triangles 36772, draws 8056->**8169** (+113), textures
  1345->**1762** (+417), pack size 11422.3->**25639.3 KiB**; object/costume/
  stage/fighter/animation counts all unchanged. PPSSPP headless: all 15
  existing golden scenes (`r0-dream-land-default` through
  `r2-depth-mask-diagnostic`) captured against packs built with only
  RE-253's fix applied (isolating RE-252's own effect) -- **byte-identical**
  in every pairing, so no goldens needed refreshing this session; an
  earlier, confounded comparison (RE-252's fix without RE-253's) had shown
  11/15 differing by up to 6940 pixels, fully explained and resolved by
  RE-253, not by any further change to RE-252 itself. Rebuilt the plain
  default (no-feature) EBOOT afterward per this project's own convention.
- Documentation: RE-252, RE-253, `PLAN.md` (`R2.2`/C4 status now
  `COMPLETE`, R0.16's D-036 acceptance item checked off, cross-reference
  table), `docs/porting-status.md`, this snapshot.
- Commit: `db2b0c2` (RE-252/RE-253, `R2.2`/C4: preserve primitive submission
  order in `merge_by_material`, and fix the `tools/romtool` texture-cache-
  key gap found while verifying it, closing C4).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
