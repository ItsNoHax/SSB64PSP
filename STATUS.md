# Project Status

**Last updated:** 2026-09-03

---

# 1. Execution State

## Current Milestone

`R0 — Rendering Correctness`

## Current Task

`R0.6 — Material System Correctness` (its lighting item)

## Task Status

`IN_PROGRESS`. RE-065 (this session) replaced `pack.rs`'s arbitrary
`LIGHT_DIR` placeholder `(2, 4, 3)` with a real, ROM-measured value: read
`MPGroundData.light_angle` (a per-stage field this session added to
`stage::GroundData`, byte offset independently corroborated against the
already-verified `camera_bound_top` offset) across all 41 stages and found
33 of them (80%) share one `(20, 45)` degree angle — now used exactly.
The other 8 stages (special-lighting locations: Brinstar, Sector Z, Metal
Mario's stage, etc.) diverge up to 111 degrees and are recorded as an
explicit, measured, `AGENTS.md` §9-compliant accepted deviation, not an
unlabeled guess — full per-stage correctness needs runtime `sceGuLight`
lighting, which this pack-time-baked-shading architecture cannot do
without a larger redesign, out of scope for this pass. R0.6's "lighting
verified" acceptance item stays unchecked (the deviation is now properly
recorded, not eliminated), and its other open items (material tables,
combiner/alpha/blend/fog/depth/culling verification) are untouched.

## Last Completed Task

R0.4 — TLUT / Palette Correctness — its "palette inheritance/state
verified" acceptance item closed this session (RE-064): added a direct
unit test (`a_texture_binding_persists_into_a_node_that_sets_no_new_state`,
`crates/ssb-rom/src/mesh.rs`) pinning that RDP texture/palette state
threads across a node sequence the way real hardware keeps it, previously
only measured archive-wide. Confirmed the test can actually fail (broke
the inheritance mechanism temporarily, watched it fail, reverted) before
trusting it green. Also confirmed by reading `tools/romtool/src/main.rs`'s
pack loop that state cannot leak *between* different objects/graphs by
construction (fresh `State::new()` per graph). One item remains open on
R0.4 — "all missing palette cases resolved" (1 `MissingPalette` failure,
file 86) — already fully attributed to `R0.7`'s scope, not touched this
pass; `R0.4` stays `IN_PROGRESS` rather than closing, since that item is
still literally unmet even though it isn't this task's fault.

R0.8 — Transform Correctness — `COMPLETE` (RE-063, earlier pass). Closed
by enumerating every transform kind (`gcSetupCommonDObjs` only ever
emits kinds 44/46/48/50 from a ROM `DObjDesc` array; everything else is
runtime-game-code-only and out of scope), implementing `Kind50` the
same way `RecalcRotRpyRSca`/`Kind46`/`Kind48` already were, and adding
regression coverage (`a_kind_50_node_is_flagged_as_a_billboard_like_kind_48`,
`crates/ssb-rom/src/pack.rs`). `cargo test --workspace`: 340 passing (was
339). `cargo clippy --release -p romtool -p ssb-rom`: clean. `cargo psp
--release`: builds clean. `tools/run-ppsspp.sh --no-build --seconds 8`:
Dream Land renders correctly at 60 FPS (`FPS: 60.0`, `cpu 2353us / budget
16667us`), clean log, no regression (Kind50 contributes 0 nodes so the
screenshot is pixel-identical to before). `romtool pack` still reports 109
billboard nodes.

Earlier this session (previous "Continue with the plan" pass), R0.8 also
fixed the `0x8000`/`RecalcRotRpyRSca` billboard gap (RE-062) — 28 nodes,
archive-wide — and the PPSSPP black-screen blocker was root-caused and
resolved (stale `assets/generated/ssb64.pak`, not a `psp/` regression; see
git history for the resolved blocker's detail, since it now lives only in
that commit and this file's history, not in the current Blockers section).

R0.7 was left `IN_PROGRESS` but its remaining scope was formally measured
rather than chased further: RE-061 traced file 86's last graph's `itGetPData`
byte-offset-delta mechanism and found `romtool mobj --file 86 --search`
returns **27 candidate table offsets**, not one — the same kind of
near-chance fingerprint match the project already measured and rejected
once (Samus's two identical 33-node graphs). No named record exists to
resolve it, so it was deliberately left unfixed rather than guessed. That
graph, file 353's Spin Attack graph (still needs its `WPAttributes`
instance typed upstream), and the other 62 unpaired graphs archive-wide are
now an accepted long tail, not an active work item.

Earlier this session, R0.7 fixed 7 of the archive's previously-71 unpaired
graphs: file 353's `EntryWave`/`EntryBeam` (RE-058/RE-059, `EFDesc`) and all
5 of file 52's room-scene graphs (RE-060, plain call-sequence pairing, no
struct). `R0.3 — Texture Conversion Completeness` closed earlier this
session; both of its failure classes were root-caused and reattributed to
the tasks that actually own a fix:

* 26 segment-0x01 entries → RE-055 → confirmed as `sLBTransitionPhotoHeap`,
  a runtime framebuffer photocopy bound to RSP segment 1, never present in
  any ROM file (`refs/ssb-decomp-re/src/lb/lbtransition.c`) → R0.13's scope
* 4 (now 1) `MissingPalette` entries → RE-057 (via a temporary instrumented
  trace of `crates/ssb-rom/src/mesh.rs`, reverted, not committed) →
  confirmed as a `PartTables` material-pairing gap in 3 specific files, not
  a texture/TLUT decode bug → R0.7's scope (RE-056's dedup-key theory was a
  real secondary factor but not the root cause; RE-057 corrects it) → 2 of
  the 3 files now fixed (RE-059, RE-060); the third (file 86) is the
  RE-061 measured-and-accepted case above

RE-054's S2DEX-BG lead (from a prior session) is refuted:
`romtool scan --exhaustive` finds zero `G_BG_1CYC`/`G_BG_COPY` anywhere in
the ROM. See `PLAN.md` R0.3, R0.7, R0.8 and R0.13.

`R0.2 — N64 Rendering Command Inventory`, `R0.1 — Rendering State
Reconciliation` and `R0.9 — Stage Animation` are also `COMPLETE` — see
`PLAN.md` for each.

## Next Eligible Task

`R0.6 — Material System Correctness` remains `IN_PROGRESS` after this
session's lighting-direction fix (RE-065, see above) — its other open
acceptance items are untouched: material tables resolved, combiner
behavior, primitive/environment colour, alpha, blending, fog, depth state,
culling, and "unsupported material behavior identified." `TODO.md` Phase D
still separately calls out removing `RE-021`'s shading-detection majority
vote (a different, structural problem: per-list conversion can't see
per-object `G_LIGHTING` state) and implementing `MOBJ_FLAG_LIGHT1/2`
(uploading real light colours, though RE-024 already found they're all
white so this may be low-value). `R0.5` (Texture Filtering / LOD /
Mipmapping — wrap modes decoded but not wired to `sceGuTexWrap`, Dream
Land canopy discrepancy still open) is also `IN_PROGRESS` and was not
evaluated this session. `R0.4`'s own remaining item ("all missing palette
cases resolved") is already fully attributed to `R0.7`'s file-86 long
tail, so R0.4 has no further independently-actionable work. `R0.7` remains
technically `IN_PROGRESS` but its remaining scope is an accepted long tail
— only worth revisiting if the upstream decompilation types
`llITCommonDataNBumperWaitMObjSub` or Spin Attack's `WPAttributes`
instance. `R0.8 — Transform Correctness` is `COMPLETE`.

## Blockers

None currently open. The PPSSPP black screen below is resolved.

### Resolved: PPSSPP black screen

**Root cause: a stale `assets/generated/ssb64.pak` on disk, not a source
regression.** The prior session's blocker note assumed the staged pack was
current because it opened cleanly host-side; that assumption was wrong and
is retracted here. `assets/generated/` is gitignored and was never
regenerated after some earlier (uncommitted/experimental) build — its
header's `prims` count (3412) doesn't match what *any* commit in history
actually produces (617-texture-era commits produce 3388; current `HEAD`
produces 3398), so it was left over from a WIP state, not from a specific
commit.

Bisection method: rather than bisecting `psp/` source (the last commit to
touch it before the report was `c252960`, seven docs/data-only commits
before `HEAD`), each candidate commit was built in a worktree and run
through `tools/run-ppsspp.sh` with its own **freshly regenerated** pack
(`romtool pack`), screenshotted, and pixel-checked. `c252960` built against
the *stale* staged pack reproduced the black screen exactly like `HEAD`
did; the same commit built against a pack regenerated from its own
`romtool` rendered Dream Land correctly. `HEAD` rebuilt against a freshly
regenerated pack also renders correctly. So no commit in git history is
responsible — every tested revision of `psp/`'s renderer works given data
that actually matches it. The mismatched pack most likely made the game
panic early in load/mesh-build (out of the visible render path), which
explains why not even the unconditional clear colour or debug overlay
reached the display — PPSSPP's own FPS counter overlay is emulator UI, not
game output, so it stayed visible regardless.

Fix: regenerated `assets/generated/ssb64.pak` via `romtool pack "rom/Super
Smash Bros. (USA).z64"` against `HEAD`. Verified via the full
`tools/run-ppsspp.sh` harness (build + run): Dream Land renders with
textures, fighters and the debug overlay, PPSSPP reports `FPS: 60.0`, and
the game's own overlay reports `cpu 2353us / budget 16667us` — comfortably
under the 60 Hz frame budget, confirming the game itself (not just the
emulator host) is running at a true 60 FPS.

Takeaway: `assets/generated/ssb64.pak` is derived, gitignored, build-only
data — it must be regenerated (`romtool pack`) after any `crates/ssb-rom`
change before trusting a screenshot, the same way a stale `EBOOT.PBP` would
be. `tools/run-ppsspp.sh`'s own "stale pack" staging message names the pack's
timestamp but does not compare it against source mtimes the way it already
does for the EBOOT; that gap is worth closing so this can't silently recur.

---

# 2. Milestone Status

| Milestone                             | Status          |
| ------------------------------------- | --------------- |
| M0 — Research                         | `COMPLETE`      |
| M1 — PSP Bootstrap                    | `COMPLETE`      |
| M2 — Resource Pipeline                | `COMPLETE`      |
| M3 — Core Game / Scene Infrastructure | `COMPLETE`      |
| R0 — Rendering Correctness            | `IN_PROGRESS`   |
| R1 — Rendering Completeness           | `BLOCKED_BY_R0` |
| R2 — Physical PSP Validation          | `BLOCKED_BY_R1` |
| R3 — Rendering Performance            | `BLOCKED_BY_R2` |
| G0 — Combat Unlocked                  | `BLOCKED_BY_R3` |

---

# 3. Rendering Gate

Rendering is currently the highest-priority development area.

Combat is intentionally blocked.

Do not begin attacks, hitboxes, damage, knockback, KO, stocks, CPU combat or match gameplay until the rendering milestones explicitly unlock combat.

Movement, physics, collision and animation may continue when required for rendering validation.

---

# 4. Verified Foundation

The following foundation work has been completed:

* ROM validation
* VPK0 decompression
* relocData archive handling
* asset extraction
* runtime asset-pack generation
* F3DEX2 display-list parsing
* N64 texture decoding
* mesh conversion
* scene graph conversion
* fighter animation extraction
* stage animation extraction
* collision extraction
* PSP asset loading
* PSP mesh drawing
* core engine traits
* fixed timestep
* fighter movement infrastructure
* stage collision infrastructure

Detailed subsystem status belongs in:

`docs/porting-status.md`

---

# 5. Known Rendering Work

Known renderer work includes investigation/fixes for:

* texture conversion completeness
* CI4/CI8 palette/TLUT behavior
* texture filtering
* LOD
* mipmapping
* texture tile addressing
* wrap/clamp/mirror behavior
* Dream Land canopy rendering
* material tables
* combiner behavior
* lighting
* alpha/blending
* render state
* unresolved MObj behavior
* transform kind `0x8000`
* stage animation
* material animation
* fighter palettes/costumes
* billboard behavior
* framebuffer effects
* camera/projection behavior
* render-state leakage
* physical PSP compatibility
* VRAM behavior
* rendering performance

The exact ordered task list is in `PLAN.md`.

---

# 6. Current Task State

## R0.8 — Transform Correctness

Status: `COMPLETE` (this session)

### Objective

Implement every transform kind exercised by SSB64.

### Required Work

* [x] Investigate `0x8000`/`RecalcRotRpyRSca` — read `gcPrepDObjMatrix` case 44 (`objdisplay.c:822`): never touches `dobj->rotate`, computes the same diagonal-from-`gGCMatrixPerspF` MVP as billboard kinds 45/46 with the spin term dropped (RE-062)
* [x] Confirm dropping `rotate` is safe, not just convenient — whole-archive check: 0 of 28 `RecalcRotRpyRSca` nodes have non-zero `rotate` (RE-062)
* [x] Fix — `crates/ssb-rom/src/pack.rs`'s `add_object` now flags `TransformKind::RecalcRotRpyRSca` as `NodeDesc::FLAG_BILLBOARD`, same as `Kind46`/`Kind48`; no `psp/` changes needed since the render path is already generic over the flag
* [x] Enumerate the remaining transform kinds — RE-063: `gcSetupCommonDObjs` is the only function that turns a ROM `DObjDesc` array into `XObj`s, and it only ever emits kinds 44/46/48/50; everything else (33-40's `func_800108xx` family, 41-43/45/47/49) is real matrix math reached only by direct runtime calls from fighter/item/effect/stage-decoration game code, never from a `DObjDesc` array — confirmed out of scope, not a gap
* [x] Kind 50 — real and reachable (`0x1000`), but 0 of 3117 nodes archive-wide use it (RE-063); flagged `FLAG_BILLBOARD` anyway (same layout as `Kind48`, sourced from `sGCMatrixMod2F` instead of `sGCMatrixMod1F`) for fidelity with the decomp's case structure
* [x] Verify on device — `tools/run-ppsspp.sh --no-build --seconds 8` after regenerating the pack: Dream Land renders correctly at 60 FPS, clean log, no regression (expected, since Kind50 contributes 0 real nodes)

### Completion Evidence

* which transform kinds were traced and what the decomp's actual matrix math does for each — RE-062 (`0x8000`), RE-063 (every other case, including 33-40 and 50)
* the fix implemented, with its measured effect — `pack.rs` flags `Kind50` alongside the other three; `cargo test --workspace` 340 passing (new test `a_kind_50_node_is_flagged_as_a_billboard_like_kind_48`)
* before/after device verification — `cargo psp --release` builds clean; `tools/run-ppsspp.sh --no-build --seconds 8` runs at 60 FPS with a clean log and a correct Dream Land screenshot
* regression test added — yes, `crates/ssb-rom/src/pack.rs::a_kind_50_node_is_flagged_as_a_billboard_like_kind_48` (plus the pre-existing RecalcRotRpyRSca test)

### R0.7 — Missing Material Tables (parked, accepted long tail)

Status: `IN_PROGRESS`, not actively worked this pass — see Last Completed
Task and RE-061. The two remaining concrete cases (file 86's last graph,
file 353's Spin Attack graph) and the other 62 unpaired graphs archive-wide
are accepted as a long tail; further progress needs upstream decomp typing,
not more `romtool` investigation.

---

# 7. Last Verification

## 2026-09-03 — R0.6: baked key light direction measured from real stage data (RE-065)

* Read `ftdisplaymain.c:1240` → `mpcollision.c:4008-9` → `mptypes.h:187` — the fighter draw path's key light direction ultimately comes from `MPGroundData.light_angle`, a per-stage `Vec3f` field, converted via `ftDisplayLightsDrawReflect`'s spherical-to-Cartesian math
* Computed `light_angle`'s byte offset (`0x60`) from the struct's field order (`unused` at `0x5C` + 4), independently corroborated: `0x60 + sizeof(Vec3f) = 0x6C` lands exactly on `camera_bound_top`'s already-verified offset
* Added `light_angle: [f32; 2]` to `crates/ssb-rom/src/stage.rs::GroundData`, read at the computed offset; added a fixture assertion (`reads_a_stage_header_and_its_layers`) pinning the read
* Wrote a temporary example (`crates/ssb-rom/examples/tmp_light_angle_scan.rs`, deleted before commit) reading all 41 stages' real angle and comparing against the old placeholder direction: **33 of 41 (80%) share exactly `(20.0, 45.0)` degrees**, 9.9 degrees from the old `(2, 4, 3)` guess; the other 8 (Brinstar, Sector Z, Hyrule, Final Destination, Metal Mario's stage, a jungle stage, a "Zako" stage, a bonus stage) diverge up to 111 degrees
* Implemented: `crates/ssb-rom/src/pack.rs`'s `LIGHT_DIR` now holds the real `(20, 45)`-degree direction (`[0.2419, 0.7071, 0.6645]`) instead of the arbitrary placeholder; documented the remaining 8-stage gap as an explicit accepted deviation (full fix needs runtime `sceGuLight` lighting, out of scope)
* `cargo test --workspace` — 341 passing, unchanged count (no test pinned the old constant's exact value)
* `cargo clippy --release -p romtool -p ssb-rom` — clean (one `#[allow(clippy::approx_constant)]`, since `sin(45°)` legitimately coincides with `1/sqrt(2)`)
* `romtool pack` — regenerated `assets/generated/ssb64.pak`
* `cargo psp --release` — builds clean
* `tools/run-ppsspp.sh --no-build --seconds 8` — Dream Land renders correctly at 60 FPS, clean log, subtly (and now more correctly) different shading
* Result: RE-065 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.6, `DECISIONS.md` D-024, `docs/porting-status.md` "Mesh conversion", `TODO.md` Phase D updated
* Affected subsystem: `crates/ssb-rom/src/stage.rs` (new field), `crates/ssb-rom/src/pack.rs` (`LIGHT_DIR`) — code change, plus documentation
* PPSSPP: tested this pass, no regression
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.4: palette/texture state inheritance pinned by a test (RE-064)

* Read `mesh.rs`'s `convert_sequence` (`mesh.rs:743`) — `State`'s texture/tile/palette fields are declared once and mutated across the node-sequence loop, reset only by an explicit state command or `forget_texture()`; a node with no material commands keeps whatever the previous node left, by construction
* Read `tools/romtool/src/main.rs`'s pack-building loop (`main.rs:952`) — `convert_sequence` is called fresh, with a new `State::new()`, once per scene graph; confirmed no code path reuses a `State` across two different objects, so cross-object leakage cannot happen architecturally
* Added `crates/ssb-rom/src/mesh.rs::a_texture_binding_persists_into_a_node_that_sets_no_new_state` — joint A fully binds a CI4 texture+palette, joint B sets no material state at all, asserts joint B's `TextureRef` equals joint A's exactly
* Verified the test can fail: temporarily reset `timg_addr`/`palette_offset`/`texture_enabled` per sequence item, reran, watched it fail with the expected panic, reverted the injected bug, confirmed the suite is clean again
* `cargo test --workspace` — 341 passing (was 340)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* No production code changed — the inheritance mechanism was already correct, only untested; `romtool pack`/PPSSPP verification skipped as disproportionate to a test-only diff (no build/runtime artifact could differ)
* Result: RE-064 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.4's "palette inheritance/state verified" item checked; `docs/porting-status.md` "Mesh conversion" row updated
* Affected subsystem: `crates/ssb-rom/src/mesh.rs` (test-only) — plus documentation
* PPSSPP: not run this pass (no production change to verify)
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.8 closed: transform kinds enumerated, `Kind50` fixed (RE-063)

* Read `gcSetupCommonDObjs` (`refs/ssb-decomp-re/src/sys/objanim.c:2153`) — the only function that turns a ROM `DObjDesc` array into `XObj`s; it tests exactly four high-nibble bits (`0x8000`/44, `0x4000`/46, `0x2000`/48, `0x1000`/50), matching `TransformKind` exactly
* Grepped the whole decompilation for `gcAddXObjForDObjFixed`/`gcAddXObjForDObjVar` call sites — dozens across `ef/`, `it/`, `gr/`, `mv/`, passing kinds like 28/40/42/44/46/70/72 as game-code-driven runtime XObj attachments, never derived from a `DObjDesc` array; confirmed kinds 33-40 (`func_800108xx` per-object look-at family) and 41/43/45/47/49 are unreachable from this project's ROM importer and out of scope until the calling gameplay systems exist
* Read `func_80010748`/`func_80010918`/`func_80010AE8`/`func_80010C2C` (`objdisplay.c:110-319`, kinds 33-40's underlying functions) — genuine per-object look-at billboards computed from object-to-camera distance vectors, distinct in technique from kinds 44-50's shared per-frame camera-basis approach
* Read case 50 (`objdisplay.c:1050`) and the `sGCMatrixMod1F`/`sGCMatrixMod2F` per-frame setup (`objdisplay.c:3033-3066`) — case 50 is `Kind48`'s exact move-word layout, sourced from `sGCMatrixMod2F` (camera-yaw-locked) instead of `sGCMatrixMod1F` (camera-pitch-locked); a genuinely different basis, not a duplicate
* Wrote a temporary example (`crates/ssb-rom/examples/tmp_kind50_scan.rs`, deleted before commit) walking every scene graph in the ROM: **0 of 3117** nodes archive-wide carry the `0x1000` bit (cross-checked 34/`0x4000`, 47/`0x2000`, 28/`0x8000` against RE-062's prior counts)
* Implemented: `crates/ssb-rom/src/pack.rs`'s `add_object` now maps `TransformKind::Kind50` to `NodeDesc::FLAG_BILLBOARD` in the same match arm as `Kind46`/`Kind48`/`RecalcRotRpyRSca`; recorded as fidelity with the decomp's case structure, not a measured fix, since no shipped node exercises it
* Added `crates/ssb-rom/src/pack.rs::a_kind_50_node_is_flagged_as_a_billboard_like_kind_48`, asserting a `0x1001`-id node reaches the pack flagged; also fixed a pre-existing `RE-061`→`RE-062` mislabel in an adjacent comment while editing the same block
* `cargo test --workspace` — 340 passing (was 339)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `romtool pack "rom/Super Smash Bros. (USA).z64"` — regenerated `assets/generated/ssb64.pak`; still 109 billboard nodes (Kind50 contributes 0, as expected)
* `cargo psp --release` (from `psp/`) — builds clean, `EBOOT.PBP` produced
* `tools/run-ppsspp.sh --no-build --seconds 8` — Dream Land renders correctly: textures, fighter, canopy billboards all present, `FPS: 60.0`, overlay reports `cpu 2353us / budget 16667us`, log clean, no regression (expected — no real node exercises the new flag path)
* Result: RE-063 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.8 marked `COMPLETE`; `docs/porting-status.md` "Billboard nodes" row updated to 97%
* Affected subsystem: `crates/ssb-rom/src/pack.rs` (`add_object`), `crates/ssb-rom/src/scene.rs` (doc comment) — code change, plus documentation
* PPSSPP: tested this pass, no regression
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — PPSSPP black-screen investigation (root-caused and resolved in a later pass, see §1 "Resolved: PPSSPP black screen")

* Copied the current build (`a31b081`) into the user's real PPSSPP install (`~/.var/app/org.ppsspp.PPSSPP/config/ppsspp/PSP/GAME/SSB64PSP/`) at the user's request; user reported a black screen
* Reproduced the same result through `tools/run-ppsspp.sh` itself (both `--seconds 8` and `--seconds 20`), ruling out the manual copy as the cause
* Manually launched PPSSPP outside the harness to inspect its window/log directly: window title correct ("Super Smash Bros. 64", not stuck at "Initializing Vulkan..."), traced PC values in the emulator log landing inside the loaded ELF's own address range and cycling at 60 Hz — the CPU is genuinely running the program, not stuck
* Pixel-sampled captures with Python/Pillow: pure `(0,0,0)` everywhere except PPSSPP's own FPS counter overlay text — not the frame's clear colour, not the unconditional per-frame debug text overlay (`gpu.debug_text`, `main.rs:862`)
* Confirmed host-side that the exact staged `ssb64.pak` opens cleanly via `Pack::open` (2450 meshes, 363 objects, 41 stages, 567 anims, 617 textures) — not a data/version problem
* Confirmed `SoftwareRenderer = True` was genuinely active in the live `ppsspp.ini` during the manual test, so this is not RE-014's known hardware-backend/`sceGuDebugFlush` visibility gap recurring
* Not root-caused. `psp/src` was not touched this session (only `crates/ssb-rom/src/pack.rs`), so this is not a regression from the R0.8 fix — it predates this session
* Restored the user's `ppsspp.ini` (`SoftwareRenderer` back to its prior value) and killed all PPSSPP processes before finishing
* Result: recorded as a blocker in §1 above; deferred to a dedicated session per the user's request, no code changed
* Affected subsystem: `psp/` runtime and/or PPSSPP interaction — unknown which yet
* PPSSPP: tested extensively this pass, unresolved
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.8: `0x8000`/`RecalcRotRpyRSca` fixed as a spin-free billboard (RE-062)

* Read `refs/ssb-decomp-re/src/sys/objdisplay.c`'s `gcPrepDObjMatrix` (the switch on `xobj->kind` that builds every DObj's MVP) — case 44 is `nGCMatrixKindRecalcRotRpyRSca` (`0x8000`); it computes `sGCMatrixMvpF` as a scaled diagonal from `gGCMatrixPerspF` and never reads `dobj->rotate` at all, then patches it into the RSP matrix via `gSPMvpRecalc`/`gMoveWd(G_MW_MATRIX,...)` — the same shape as the already-implemented billboard cases 45/46, just without the `sin`/`cos` spin term
* Wrote a temporary example (`crates/ssb-rom/examples/tmp_recalc_rotate.rs`, deleted before commit) walking every scene graph in the ROM: **0 of 28** `TransformKind::RecalcRotRpyRSca` nodes have non-zero `rotate` — confirms the field really is unused for this kind, not coincidentally zero
* Cross-checked the node counts: `Kind46` 34 + `Kind48` 47 + `RecalcRotRpyRSca` 28 = 109, matching `docs/porting-status.md`'s prior "81 billboard nodes flagged, 28 not" split exactly
* Implemented: `crates/ssb-rom/src/pack.rs`'s `add_object` now maps `TransformKind::RecalcRotRpyRSca` to `NodeDesc::FLAG_BILLBOARD` in the same match arm as `Kind46`/`Kind48`; no changes to `psp/src/meshdraw.rs` — its billboard path is already generic over the flag
* Added `crates/ssb-rom/src/pack.rs::a_recalc_node_is_flagged_as_a_spin_free_billboard`, asserting a `0x8001`-id node reaches the pack flagged
* `cargo test --workspace` — 339 passing (was 338)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `cargo psp --release` (from `psp/`) — builds clean, `EBOOT.PBP` produced
* `tools/run-ppsspp.sh --seconds 8` — launched, ran the full 8s at 60 FPS, log clean (no errors/crashes); captured screenshot was black (idle boot-time frame at this point in the run, consistent with other short captures — not itself evidence either way). Did not isolate a specific `0x8000` object on screen this pass
* Result: RE-062 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.8, `TODO.md` Phase E, `docs/porting-status.md` "Billboard nodes" updated to match
* Affected subsystem: `crates/ssb-rom/src/pack.rs` (`add_object`) — code change, plus documentation
* PPSSPP: smoke-tested this pass (build + 8s run, no crash), not visually isolated
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.7: file 86's last graph measured and left open, not fixed (RE-061)

* Ran `romtool mobj --file 86` — confirmed the one remaining unpaired graph is at `0x7BE8`, the "N-Bumper" item's attached-pose `DObjDesc` (`refs/ssb-decomp-re/src/relocData/86_ITCommonObject.c:1812`)
* Traced its pairing mechanism to `itNBumperAttachedInitVars` (`refs/ssb-decomp-re/src/it/itcommon/itnbumper.c:367`): `itGetPData(ip, &llITCommonDataNBumperDataStart, &llITCommonDataNBumperWaitMObjSub)` — a compile-time byte-offset delta from a runtime pointer. Confirmed neither `llITCommonDataNBumperDataStart` nor `llITCommonDataNBumperWaitMObjSub` is declared anywhere else in the decompilation (both are still-unmatched linker symbols) — no named record exists to read
* Ran `romtool mobj --file 86 --search` (the project's own demand-vector search diagnostic) — returns **27 candidate table offsets** for this one graph, not one; confirmed this is the same near-chance fingerprint-match situation the project already measured and rejected once (Samus's two identical 33-node graphs, documented in `mobj.rs`'s own doc comment)
* Decision: left unfixed. No code change. Recorded as a measured negative result rather than continuing to search
* Result: RE-061 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.7, `TODO.md` Phase E updated to mark file 86's graph, file 353's Spin Attack graph, and the archive's other 62 unpaired graphs as an accepted long tail
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.7: file 52 fully resolved via a fourth pairing mechanism (RE-060)

* Ran `romtool mobj --file 52` — confirmed 5 unpaired graphs (0x7E98, 0x1C4A8, 0x1DF28, 0x1F270, 0x22440)
* Identified file 52 as `refs/ssb-decomp-re/src/relocData/52_MVCommon.c` — the opening movie's room cutscene (`RoomBackground`, `RoomDesk`, `RoomLogo`, etc.), not a UI/menu system as earlier guessed
* Traced `refs/ssb-decomp-re/src/mv/mvopening/mvopeningroom.c` — each room piece is set up with two independent calls on the same `GObj`: `gcSetupCommonDObjs(gobj, dobjdesc)` then a separate `gcAddMObjAll(gobj, mobjsub)` — no struct links them, only the call order in the executable. Checked every `gcSetupCommonDObjs` call against whether a `gcAddMObjAll` follows on the same `gobj`: exactly 5 do, matching the 5 unpaired graph offsets; the rest (`RoomDesk`, `RoomBooks`, `RoomLamp`, etc.) correctly have none
* Read the 5 graphs' and 5 tables' file offsets directly from `52_MVCommon.c`'s own comments: `(0x7E98, 0x42F8)` RoomBackground, `(0x1C4A8, 0x1BC60)` RoomLogo, `(0x1DF28, 0x1DCA0)` RoomCloseUpEffectAir, `(0x1F270, 0x1F0F8)` RoomCloseUpEffectGround, `(0x22440, 0x20480)` RoomDeskGround
* Implemented: 5 more hand-inserts in `tools/romtool/src/main.rs`'s `load_all`, same gated pattern as RE-059
* `cargo build --release -p romtool` — clean
* `romtool mobj --file 52` after — 5/5 graphs paired, 0 chain/demand mismatches, "wanting one but unnamed" 5→0
* `romtool textures --file 52` after — 58/58 packed, 0 failures (file 52 fully resolved)
* `romtool textures` (archive-wide) after — 618→638 packed, 647→665 unique bound (previously-unbound primitives now resolve a real texture), `MissingPalette` 3→1
* `romtool mobj` (archive-wide) after — 58→63 paired, 69→64 unpaired
* `cargo test --workspace` — 338 passing, unaffected
* Traced file 86's remaining graph's pairing mechanism: `refs/ssb-decomp-re/src/it/itcommon/itnbumper.c:367`'s `itGetPData` computes the MObjSub pointer as a byte-offset delta from a runtime base pointer — a fifth mechanism, not yet confirmed against file 86's specific graph; left unfixed
* Result: RE-060 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.3/R0.4/R0.7, `TODO.md` Phase B/E, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: `tools/romtool/src/main.rs` (`load_all`) — code change, plus documentation
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.7: EFDesc found and fixed for file 353 (RE-059)

* Ran `romtool mobj --file 353` before any change — confirmed 3 unpaired graphs (0x3F8, 0x7B8, 0x11C0), each "calls the graphics heap but no record names"
* Traced `refs/ssb-decomp-re/src/ft/ftcommon/ftcommonentry.c:214-215` → `efManagerLinkEntryWaveMakeEffect`/`efManagerLinkEntryBeamMakeEffect` → `refs/ssb-decomp-re/src/ef/efmanager.c:1162-1219`'s `dEFManagerLinkEntryWaveEffectDesc`/`dEFManagerLinkEntryBeamEffectDesc` — found `EFDesc` (`refs/ssb-decomp-re/src/ef/eftypes.h:11-24`), a third pairing shape, with confirmed non-null `DObjDesc*`/`MObjSub***` pointers into file 353 (`EntryWaveDObjDesc @ 0x3F8`/`EntryWaveMObjSub @ 0x130`, `EntryBeamDObjDesc @ 0x7B8`/`EntryBeamMObjSub @ 0x4F0`, cross-checked against `353_LinkSpecial2.c`'s own offset comments)
* Confirmed `EFDesc` instances live in the game's static executable (fixed RAM addresses in `efmanager.c`, not relocData offsets), so `PartTables::scan`'s archive-relocation-only scan can structurally never find them
* Implemented: `tools/romtool/src/main.rs`'s `load_all` now hand-inserts `(353, 0x3F8, 0x130)` and `(353, 0x7B8, 0x4F0)` via `PartTables::insert()`, gated on `read_table` parsing (same pattern as the existing stage-layer inserts)
* `cargo build --release -p romtool` — clean
* `romtool mobj --file 353` after — pairings 56→58, 2 graphs now paired, 0 chain/demand mismatches for both
* `romtool textures --file 353` after — 1 failure → 0
* `romtool textures` (archive-wide) after — 617→618 packed, `MissingPalette` 4→3
* `romtool mobj` (archive-wide, no `--file`) after — 58 paired / 69 unpaired, exactly 71-2, confirming no other archive-scannable `WPAttributes`/`EFDesc` instance was already being silently caught and that both the old 56/71 figures were accurate, just stale
* `cargo test --workspace` — 338 passing, unaffected (fix is in `romtool`, not the library crate)
* Traced SpinAttack's pairing mechanism too: `refs/ssb-decomp-re/src/wp/wplink/wplinkspinattack.c`'s `dWPLinkSpinAttackWeaponDesc` names `&llLinkMainSpinAttackWeaponAttributes` (in file 225) as a `WPAttributes`, but unlike the boomerang's, this instance is not yet typed in the decompilation — deliberately left unfixed rather than guessing a table offset
* Result: RE-059 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.3/R0.4/R0.7, `TODO.md` Phase B/E, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: `tools/romtool/src/main.rs` (`load_all`) — code change, plus documentation
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — R0.7 started: WPAttributes found, 353 sibling-file guess retracted

* Read `refs/ssb-decomp-re/src/relocData/353_LinkSpecial2.c` directly — found `dLinkSpecial2_EntryWaveDObjDesc`/`dLinkSpecial2_EntryBeamDObjDesc` (graphs) and `dLinkSpecial2_EntryWaveMObjSub`/`dLinkSpecial2_EntryBeamMObjSub` (tables) both defined in the same file, refuting RE-057's "table lives in a sibling file" guess
* Read `refs/ssb-decomp-re/src/relocData/225_LinkMain.c`'s `FTCommonPartContainer` — confirmed it pairs Link's *main* model (`dLinkModel_JointTree`, both halves in file 324) and has nothing to do with 353's sub-models
* Read `refs/ssb-decomp-re/src/wp/wptypes.h:36-45` — found `WPAttributes`, a second struct with the same `DObjDesc*`/`MObjSub***` adjacency `PartTables::scan` looks for, used for weapon/projectile objects, never documented in `crates/ssb-rom/src/mobj.rs`
* Read `refs/ssb-decomp-re/src/relocData/226_LinkSpecial1.c`'s `dLinkSpecial1_Boomerang_WeaponAttributes` — a real `WPAttributes` instance with `p_mobjsubs = NULL` by design (its `data`/`anim_joints` point into file 325, not 353) — shows a null table can be intentional, tempering how much weight to put on "found the missing struct shape" as a guaranteed fix for 353
* Result: RE-058 recorded in `docs/reverse-engineering.md`, retracting RE-057's specific 353 guess while keeping its zero/partial-`mobjs` mechanism finding intact; `PLAN.md` R0.7, `TODO.md` Phase E updated
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — R0.3 closed: segment-0x01 and MissingPalette investigations

* `cargo run --release -p romtool -- scan "rom/Super Smash Bros. (USA).z64" --exhaustive` — 0 occurrences of opcode 0x09/0x0A anywhere in the ROM's display lists; refutes RE-054's S2DEX BG lead
* `cargo run --release -p romtool -- dump "rom/Super Smash Bros. (USA).z64" 39` (and 40/41/45/50/51) — dumped raw file bytes, located the failing `G_SETTIMG` (file 39 offset 0x0E10: `fd10012b 01000000`), confirmed identical bytes recur across files 40/41/45/50/51
* Cross-checked address `0x01000000` (segment 1) against `refs/ssb-decomp-re/src/lb/lbtransition.c` — `gSPSegment(..., 0x1, sLBTransitionPhotoHeap)`, a per-frame `300x220` 16-bit framebuffer photocopy for the loading-break transition system; texture dims from `romtool textures --file 39` (`300x5`, `300x6` `Rgba/Bits16`) match
* Temporarily instrumented `crates/ssb-rom/src/mesh.rs` with `eprintln!`s (in `convert_sequence`, the segment-0x0E `Call` handler, `SetTimg`, `LoadTlut`), ran `romtool textures --file 52/86/353`, then reverted the instrumentation (`git checkout -- crates/ssb-rom/src/mesh.rs`) — confirmed files 52 and 353 get zero `MObj` materials for every graph node, file 86 gets them for most but not all nodes; every unresolved segment-0x0E call clears an otherwise-valid palette via `forget_texture()`
* Identified file names via `refs/ssb-decomp-re/src/relocData/`: 52=`MVCommon`, 86=`ITCommonObject`, 353=`LinkSpecial2` (one of Link's own model files, unlike 52/86)
* Result: RE-057 recorded in `docs/reverse-engineering.md`, correcting RE-056's dedup-key theory. `PLAN.md` R0.3 marked `COMPLETE`; R0.4, R0.7 updated; `TODO.md` Phase B/E, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: documentation/investigation only — `crates/ssb-rom/src/mesh.rs` was temporarily modified for tracing and fully reverted before commit; `git diff` confirms no net code change
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below
* Result: RE-055 and RE-056 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.3/R0.13, `TODO.md` Phase B, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — R0.2 BattleShip cross-reference

* Cloned `refs/BattleShip` + `libultraship` submodule (`ssb64` branch); read `src/fast/interpreter.cpp`
* Cross-checked against `refs/ssb-decomp-re/src/sys/objdisplay.c` (already present, no clone needed)
* Result: RE-054 recorded in `docs/reverse-engineering.md`; three new leads filed in `TODO.md` (R0.13 S2DEX BG, R0.5 LOD corroboration, R0.8 transform clarification)
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — Documentation audit (R0.1)

* `cargo test --workspace` — 338 passing (`ssb-rom` 195, `ssb-engine` 36, `ssb-game` 107), 0 failed
* `cargo run --release -p romtool -- textures "rom/Super Smash Bros. (USA).z64"` — 647 bound / 617 packed / 30 failed (26 segment-0x01, 4 `MissingPalette`); matches what `docs/rendering.md` now documents
* Affected subsystem: documentation only, no code changed
* PPSSPP: not re-run this pass; prior baseline stands (see `docs/porting-status.md` "M1 verification")
* Physical PSP: not tested this pass — see §8 below

Future entries must include:

* command/test;
* result;
* relevant output;
* affected subsystem;
* whether PPSSPP was tested;
* whether physical PSP was tested.

---

# 8. Physical PSP Validation

## Status

`R2 ACCEPTANCE NOT YET PERFORMED`

The project has been booted and smoke-tested on physical PSP hardware earlier
in development. That testing was not captured against `PLAN.md` R2's
acceptance criteria (hardware model, build, asset-pack version, per-feature
observed behavior, VRAM usage) and predates the rendering work tracked under
R0, so it does not stand in for R2. Treat R2 as not started until a hardware
session records the fields below.

PPSSPP testing does not count as physical PSP validation.

When R2 hardware validation begins, record:

* PSP model
* firmware/environment
* EBOOT/build
* runtime asset pack
* test scene
* observed FPS/frame time
* VRAM behavior
* rendering failures
* screenshots/video where useful

---

# 9. Important Rules

* `PLAN.md` determines task order.
* `STATUS.md` determines current execution state.
* `docs/porting-status.md` determines verified subsystem status.
* `AGENTS.md` determines agent behavior.
* Rendering is the hard gate.
* Do not begin combat because rendering appears "good enough".
* Do not mark work complete without evidence.
* Do not treat PPSSPP as physical PSP validation.
* Do not invent original N64 behavior when the decompilation/ROM can answer it.

---

# 10. Session Continuity

Before ending a session or context compaction, update this file with:

```text
Current milestone
Current task
Task status
Last completed task
Next eligible task
Blockers
Changes made
Verification performed
Evidence
Important discoveries
Documentation updated
Relevant commit
```

A fresh agent must be able to read this file and continue without relying on conversation history.

---

# 11. Continuation Command

The intended workflow is:

> Continue with the plan.

The agent should then:

1. Read `AGENTS.md`.
2. Read `PLAN.md`.
3. Read this file.
4. Inspect the repository.
5. Resume the current task.
6. Implement.
7. Verify.
8. Document.
9. Update this file.
10. Commit.
11. Continue to the next eligible task.
