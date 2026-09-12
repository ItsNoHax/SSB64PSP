# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.2 — Second Renderer Corrective Gate (C1-C7)` (`IN_PROGRESS`)
- Status: `IN_PROGRESS` -- C1 (`COMPLETE`); C2 (`COMPLETE`); C3 (`COMPLETE`,
  RE-244 through RE-251, 8 parts); C4 (`COMPLETE`, RE-252/RE-253); C5
  (`COMPLETE`, RE-254); C6 (`IN_PROGRESS`, unblocked by RE-256, all 15
  golden diffs explained by RE-257, not closed); C7 remains
- Last complete: `RE-257` (2026-09-12) -- `R2.2`/C6, per-scene explanation of
  RE-256's 15 golden diffs (C6's own "explain every change" requirement).
  12 of 15 (Fox/Falcon/Kirby/Ness/DK fighters, Stage Sector, Catch Swirl,
  Dream Land, all 3 metal-texgen camera/rotation variants) trace directly
  to RE-240's already-verified fix: Fox/Falcon/Kirby/Ness/DK/Stage-Sector's
  own source files (313/332/328/335/317/109) are all in RE-240's 23-file
  lit-vertex-colour-baking census (Falcon's boots visibly red->gold, DK's
  fur/tie gain real texture detail -- a hue change, not just brightness, but
  exactly what that fix predicts); Catch Swirl's flat-colour test
  (gray->yellow pinwheel) matches RE-240's *other* mechanism (unlit
  `prim_color` no longer double-scaled) since its file (84) isn't itself in
  the lit-vertex census; Dream Land and the 3 metal-texgen variants match
  RE-251's already-documented depth-order corrections. 1 scene
  (`depth_mask_diagnostic`) unaffected (pack-independent). **1 genuine open
  question:** `r2-metal-texgen-linear` differs by 25,552px (the other 3
  metal-texgen variants are 684-3312px) but the diff is confined to an
  incidental background prop (a sign/plank), not the `G_TEXTURE_GEN_LINEAR`
  crystal the scene actually tests (pixel-identical); not traced further
  this session. No new corruption found on any of the 15. See
  `docs/reverse-engineering.md` RE-257 for the full per-scene breakdown.
  User confirmed refreshing all 15 committed golden PNGs afterward
  (`61d417f`) -- 14 changed, `r2-depth-mask-diagnostic.png` byte-identical
  so untouched.
- Previously complete: `RE-256` (2026-09-12) -- `R2.2`/C6, **unblocks C6,
  does not close it**: fixed RE-255's pack-load-failure blocker via
  `MEMSIZE=1`, the standard PSP homebrew `PARAM.SFO` key requesting the full
  64MiB (instead of 32MiB) on PSP-2000/3000 (Slim/Brite) hardware --
  confirmed via PPSSPP source (`UseLargeMem`/`InitMemorySizeForGame`) and
  empirically (a temporary, reverted diagnostic found the default ceiling
  is 20-24MiB; `MEMSIZE=1` only takes effect when booted from an installed
  `PSP/GAME/<id>/` layout, not a loose `EBOOT.PBP`, which is why
  `tools/run-ppsspp-headless.sh` needed a matching fix -- it now stages into
  `~/.ppsspp/PSP/GAME/ssb64_regression`). Required edits in two repos: the
  user's own `rust-psp` fork (`mksfo`'s key whitelist, `cargo-psp`'s config)
  plus `psp/Psp.toml` (`memsize = 1`) here. A self-inflicted near-miss
  during this fix -- rebuilding `cargo-psp` from the fork's HEAD silently
  broke the globally-installed toolchain against this project's pinned
  nightly -- was caught immediately and fixed via a temporary `git
  worktree` at the pre-break commit, without touching the user's own
  checkout/branch state. See `docs/reverse-engineering.md` RE-256.
- Previously complete: `RE-255` (2026-09-12) -- `R2.2`/C6 attempt, found the
  blocker RE-256 then fixed: all 15 committed goldens differed from the
  on-device build by 60,000+ pixels each; traced to `assets::load_pack` ->
  `AlignedBuf::new` returning `None` (`LoadError::OutOfMemory`) at the
  pack's ~25.6MiB size, so every capture was silently the M1 milestone
  fallback tetrahedron (`psp/src/main.rs:164`), not ROM content -- RE-253's
  own flagged "real-PSP RAM headroom not yet checked" follow-up, confirmed
  to fail. See `docs/reverse-engineering.md` RE-255.
- Previously complete: `RE-254` (2026-09-12) -- `R2.2`/C5, **closes C5**:
  systematic inventory of every raw GU (`sceGu*`) mutation outside
  `apply_material`, checked against the one bar that actually matters
  (`DrawState::begin_frame` already clears the whole cache every frame, so
  only a raw mutation sitting *between* two `DrawState`-tracked draws in the
  same frame can corrupt anything). Found exactly one live case --
  `draw_collision`/`draw_fighter` via `draw_line_strip`/`draw_triangles`'s
  raw `Texture2D` disable -- already fixed by RE-118's `forget_texture`.
  Every other site (`draw_texture_quad`, `draw_particle`,
  `Gpu::draw_wallpaper_sprite`, `normal_diag::draw`, `depth_diag::draw`) is
  either structurally unreachable in the same frame as any `DrawState`-
  tracked draw (mutually exclusive debug-viewer `match` arms) or the last
  thing to touch the GE before that frame's `end_frame` -- a measured
  negative result. Added `DrawState::invalidate_all()` (a `forget_texture`
  superset covering `last_texture`/`last_flags`/`last_texture_blend`/
  `last_fighter_light_colors`/`last_fighter_material_color`/
  `last_texture_mapping`, deliberately excluding `runtime_fighter_light` --
  that is the caller's own explicit fighter-light context, not a cached
  comparison) and wired it at all five sites as hardening against future
  callers, not a fix for an observed bug. `draw_texture_quad`/`draw_particle`
  now take a `&mut DrawState` parameter; their three `main.rs` call sites
  updated to pass the frame's existing `draw_state`. See
  `docs/reverse-engineering.md` RE-254 for the full per-site inventory
  table.
- Previously complete: `RE-253` (2026-09-12) -- found and fixed while verifying
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
- Next: `R2.2`/C6 is unblocked (RE-256) and every golden diff is now
  individually explained (RE-257). Remaining C6 work: (1) a deliberate
  decision on formally refreshing the 15 committed golden PNGs (not done
  automatically -- `AGENTS.md`'s "never overwrite goldens blindly"); (2)
  the broader fighter/effect/lighting/texture-blend/translucency/billboard/
  framebuffer recheck `PLAN.md` C6 asks for, beyond the 5 fighters already
  covered by the existing golden set; (3) physical-PSP confirmation that
  `MEMSIZE=1` actually grants extra RAM on the real Slim unit (PPSSPP's
  memory model matches Slim/Brite hardware but is not proof of it); (4)
  `r2-metal-texgen-linear`'s own genuine open question (a 25,552px diff
  confined to an incidental background prop, not the tested texgen
  content) is not yet traced.
- Blockers: none. New non-blocking follow-ups from this session: (1) the
  globally-installed `cargo-psp`/`mksfo`/`pack-pbp`
  is now a hybrid build (the user's own fork's pre-`fix-panic-payload-
  nightly` base, plus this session's `MEMSIZE` patch) -- their
  `fix-panic-payload-nightly` branch's own `libunwind`/`PanicPayload` fixes
  are not in the currently-installed binary; reconciling that branch's
  nightly requirement (2026-08-26+) with this project's pinned one
  (2026-08-01, documented broken past 2026-08-25) is a pre-existing
  conflict this session did not create and did not resolve; (2)
  `r2-metal-texgen-linear`'s incidental-prop texture-clarity difference
  (RE-257), not traced this session. Prior non-blocking follow-ups remain
  open: (a) RE-251's environment/toolchain drift for `depth_mask_diagnostic`
  (the one pack-independent golden, still 0px so unaffected by RE-255/256);
  (b) physical-PSP confirmation of `tests/golden/r2-depth-mask-
  diagnostic.png` (RE-251); (c) RE-240/RE-241/RE-242/RE-245's older
  follow-ups (visual before/after for RE-240's 23 affected files is now
  substantially covered by RE-257 for the 5 fighters/Stage-Sector already
  in the golden set, 18 of the 23 files remain unchecked; the
  `debug_overlay` HUD text bug, `task_1bf9bc35`; RE-224's TEXVIEW
  screenshot; RE-228's clamp-boundary deviation; RE-214/RE-236's known
  screenshot noise floor; R2.1/T1's 164 cross-node differing-transform
  vertex reuses) -- see prior snapshot history for detail.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-257.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`COMPLETE`, T1-T10 all
  terminal); `R2.2` (`IN_PROGRESS`, C1 `COMPLETE`, C2 `COMPLETE`, C3
  `COMPLETE`, C4 `COMPLETE`, C5 `COMPLETE`, C6 `IN_PROGRESS` (unblocked,
  diffs explained, not closed), C7 remains).
- Decisions: `DECISIONS.md` -- no new revision for RE-257; D-042 ("renderer
  correctness claims stay provisional until R2.2 closes") still applies.
- Subsystem: `docs/porting-status.md` -- updated the "Mesh conversion" row
  (RE-256: C6 unblocked via `MEMSIZE=1`; per-scene diffs now explained by
  RE-257, formal refresh still open).
- Verification (RE-257): no code changed -- analysis only. Side-by-side
  comparison (`magick +append`) of every one of the 15 recaptured goldens
  (RE-256) against its committed PNG. Cross-referenced RE-240's own
  `census_lit_primitives_with_a_colour_baking_branch` file list (23 files)
  against each scene's source file: Fox (313), Falcon (332), Kirby (328),
  Ness (335), DK (317) and Stage Sector (109) are all in that list. Catch
  Swirl (file 84, not in the list) matches RE-240's separate unlit-
  double-scale mechanism instead. Dream Land and the 3 rotation/camera
  metal-texgen variants match RE-251's already-documented depth-order
  corrections. `depth_mask_diagnostic` unaffected (0px, pack-independent).
  `r2-metal-texgen-linear`'s 25,552px diff isolated to an incidental prop
  (a sign/plank) by visual inspection -- the scene's own
  `G_TEXTURE_GEN_LINEAR` crystal is pixel-identical between captures.
- Documentation: RE-257, `PLAN.md` (C6 section, lighting-correctness gate
  row), this snapshot.
- Commit: `038a548` (RE-257, per-scene diff explanation, docs only);
  `61d417f` (golden refresh, 14 of 15 `tests/golden/*.png` changed).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
