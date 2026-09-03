# Project Status

**Last updated:** 2026-09-03

---

# 1. Execution State

## Current Milestone

`R0 — Rendering Correctness`

## Current Task

`R0.5 — Texture Filtering / LOD / Mipmapping`

## Task Status

`IN_PROGRESS`. RE-071 (this session) followed up on a natural question
RE-070 raised: now that Dream Land's canopy-highlight texture is
pre-blurred (RE-070), is RE-069's deferred `translucent` blend (deferred
because it produced a checkerboard on that same texture) safe to enable?
Re-tested directly, as a reversible experiment (re-enabled
`GuState::Blend`, rebuilt clean from a deleted `EBOOT.PBP` per RE-070's
own lesson about stale binaries): **no** — the result is different and
*worse*, blown-out oversaturated highlights erasing the flowers and most
other detail, not the earlier checkerboard. Objective pixel statistics
alone said "~4% less noise," which would have been a misleading
green-light if trusted without also looking at the actual image — a
second methodology lesson layered on RE-070's first one.

Tested one more specific hypothesis before stopping: unpremultiplied-alpha
blurring (`box_blur_wrapped` averages RGB and alpha independently, a known
way to leak "invisible" colours from transparent texels once alpha
changes). Implemented a premultiplied variant as a temporary experiment —
identical result, ruling this out too. Both experiments were reverted
before commit (`git status` clean of them); only the negative-result
record remains, in `docs/reverse-engineering.md` and a permanent, updated
comment in `psp/src/meshdraw.rs` so a future session doesn't re-run either
already-eliminated experiment. `PLAN.md` R0.6's "blending verified" item
stays open, narrowed but not closed — the real cause is still unknown.

## Last Completed Task

R0.5 — Texture Filtering / LOD / Mipmapping — RE-070 (earlier this
session) directly tested RE-053's own two suggested fixes for Dream
Land's canopy dither. Filtering alone: a reversible on-device
`Nearest`-vs-`Linear` A/B measured it helps a little but not enough
(bilinear only interpolates a 2x2 neighbourhood, narrower than the
dither's repeat). Resolving it at conversion time: box-blurring the two
canopy textures and requantizing back to their 16-entry CI4 palette
changed nothing visible (the blur mostly snaps back to the same two
palette entries); packing the same blur unquantized (`Psm8888`) instead
produced a real, measurable improvement — after catching and correcting a
methodology mistake (a stale `EBOOT.PBP` made the first look appear to be
a full fix; rebuilding clean and measuring pixel statistics objectively
showed a real but partial ~19-40% noise reduction instead). Implemented
as `crates/ssb-rom/src/texture.rs::box_blur_wrapped`, applied through a
named allowlist (`tools/romtool/src/main.rs`'s `NEEDS_DITHER_BLUR`) of
exactly the two Dream Land canopy textures. Costs +112 KiB VRAM
(1059.0→1170.9 KiB). `PLAN.md` R0.5's "Dream Land canopy discrepancy
resolved" item stays unchecked — real progress with numbers behind it,
not a claim of completion.

R0.6 — Material System Correctness — RE-069 (earlier this session)
decoded `G_SETOTHERMODE_L`'s render-mode field for the first time
(`mesh.rs` never read it at all) into `alpha_test`/`translucent`. A naive
"`FORCE_BL` bit means blending" signal is wrong — the RDP reset's own
opaque default sets it too — so the real signal (whether the blend
equation reads the framebuffer weighted by `1 - alpha`) was cross-checked
against `gbi.h`'s macros and `refs/BattleShip`'s interpreter. Measured
archive-wide: 36.1% of non-default render modes are cutout (`TEX_EDGE`-
family) surfaces, 14.4% genuinely translucent — both previously completely
unimplemented on the PSP side. Shipped `alpha_test` (matching
`refs/sf64-psp`'s validated real-hardware approximation) after finding and
fixing a bug where untextured lit primitives were alpha-tested against a
packed-normal byte and discarded themselves outright, visibly deleting
Dream Land's decorative flowers. Found a second, harder bug in
`translucent` — enabling real blending on the canopy-highlight surface
produced a checkerboard, the same open dithered-texture/coverage problem
RE-053 already found — and deliberately did not ship it, leaving
detection in place but `GuState::Blend` unwired on device pending that
investigation (which RE-070, above, then partially advanced). `PLAN.md`
R0.6's "alpha behavior verified" item is checked; "blending verified"
stays open.

R0.6 — Material System Correctness — RE-068 (earlier this session) found
and fixed a structural gap far bigger than the "depth state verified"
item it started from: read `refs/ssb-decomp-re/src/sys/rdp.c`'s
`sSYRdpResetDisplayList`, replayed once per frame (`taskman.c:308`) before
any object draws, and found it sets `G_ZBUFFER | G_SHADE | G_CULL_BACK |
G_SHADING_SMOOTH` **on** by default — not all-off. `crates/ssb-rom/src/
mesh.rs`'s `State::new()` seeded an all-off `MeshMaterial::default()`
instead, so any node whose own list never mentioned geometry mode (the
common case, since this state is normally set once per frame, not per
node — the same structural shape as RE-021's lighting finding, one level
up) converted as unculled, flat-shaded and non-depth-tested. Measured
archive-wide: `Z_BUFFER` went from 6/3426 packed primitives (0.17%) to
3384/3442 (98.3%) after seeding from a new `MeshMaterial::rdp_default()`
instead; `CULL_BACK` measured 86.3%, `SMOOTH` 76.5% post-fix — the shape a
real game's geometry should have. Also wired `psp/src/meshdraw.rs`'s
`apply_material` to actually toggle `GuState::DepthTest` per primitive
from the `Z_BUFFER` flag (it was already packed, just never read on the
device side). `PLAN.md` R0.6's "depth state verified" and "culling
verified" items are checked.

R0.5 — Texture Filtering / LOD / Mipmapping — RE-067 (earlier this
session) traced `G_TX_MIRROR` (RE-066's one real, quantified gap) directly
to Dream Land's still-open canopy discrepancy (RE-053): its exact display
list (file 104, offset `0x798`) sets `cm_s=3 cm_t=3 mask_s=6 mask_t=6`.
Confirmed the wrap boundary actually mattered *before* implementing
anything — a reversible on-device `Repeat`-vs-`Clamp` experiment (2-line
change, screenshotted, reverted) showed a dramatically different image.
Implemented the real fix: `crates/ssb-rom/src/texture.rs::mirror_extend`
pre-bakes a mirrored copy of each affected texture at pack time (exact,
not approximated, since `sceGuTexScale` already renormalises UVs against
the packed texture's own dimensions). Not scoped to Dream Land alone: 187
of 638 packed textures (29%) affected archive-wide, packed texture VRAM
rose from 763.2 KiB to **1059.0 KiB (1.5x the ~700 KiB budget)**. Given
the scale of that tradeoff, stopped and asked the user how to proceed
(ship fully / scope to paletted formats only / document without shipping
/ revert) rather than deciding unilaterally — the user chose to ship it
fully. `PLAN.md` R0.5's "Dream Land canopy discrepancy resolved" item
stays unchecked — this fixed one real contributing cause, but RE-053's
separate magnification/dithering diagnosis is untouched. Texture
streaming (`TODO.md` Phase G) is no longer optional headroom given the
new VRAM figure.

RE-066 (earlier still) closed R0.5's "wrap/clamp/mirror behavior
verified" and "texture tile parameters verified" items with a measurement,
not a code fix at the time: read every tile-0 `G_SETTILE` archive-wide
(754, not sampled) and found `psp/src/meshdraw.rs`'s hardcoded
`sceGuTexWrap(Repeat, Repeat)` is already correct for clamp — every axis
that requests clamp also has its own mask nonzero (0 counterexamples), and
`refs/BattleShip`'s reference RDP interpreter strips `G_TX_CLAMP` under
exactly that condition on real hardware, confirming `mesh.rs`'s existing
mask-narrowed texture sizing (RE-044) already reproduces correct periodic
addressing. Identified
`G_TX_MIRROR` (208/754 tile-0 lists, 27.6%) as the one real, quantified
gap — which RE-067 (above) then traced and fixed.

R0.6 — Material System Correctness — its lighting placeholder replaced
with a real, ROM-measured value this session (RE-065): read
`MPGroundData.light_angle` (a per-stage field added to `stage::GroundData`,
byte offset independently corroborated against the already-verified
`camera_bound_top` offset) across all 41 stages and found 33 of them (80%)
share one `(20, 45)` degree angle — now used exactly for `pack.rs`'s baked
`LIGHT_DIR`. The other 8 stages (special-lighting locations: Brinstar,
Sector Z, Metal Mario's stage, etc.) diverge up to 111 degrees and are
recorded as an explicit, measured, `AGENTS.md` §9-compliant accepted
deviation — full per-stage correctness needs runtime `sceGuLight` lighting,
out of scope for a pack-time-baked-shading architecture. R0.6's "lighting
verified" acceptance item stays unchecked (the deviation is now properly
recorded, not eliminated), and its other open items (material tables,
combiner/alpha/blend/fog/depth/culling verification) are untouched.

R0.4 — TLUT / Palette Correctness — its "palette inheritance/state
verified" acceptance item closed in an earlier pass (RE-064): added a
direct unit test (`a_texture_binding_persists_into_a_node_that_sets_no_new_state`,
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

RE-070 made real but partial progress on the dithered-texture/coverage
problem (~40% less local noise on the treated textures, not a full fix).
Three concrete, well-scoped options remain, each with a real starting
point already recorded rather than an open-ended search:

1. **Push further on the dither**, now that pre-blur-and-pack-unquantized
   is confirmed to be the right *kind* of fix, just not strong enough yet.
   Ideas not yet tried: a larger blur radius or multiple passes; blurring
   before the mirror-extend doubling instead of after (RE-067's
   `mirror_extend` runs first currently, so the blur's wrap-around samples
   already-mirrored, possibly-duplicated neighbours — check whether that
   matters); or reconsidering whether RE-053's separate magnification
   diagnosis (not attempted by RE-070) is actually the larger remaining
   contributor.
2. **`translucent` blend is a closed line of inquiry for now, not an open
   one** — RE-071 already re-tested it against the RE-070-blurred texture
   (worse, not better) and ruled out unpremultiplied-alpha blurring as the
   cause too. Do not re-run either experiment without new evidence; the
   next lead needs to be different, e.g. comparing this converter's raw
   decoded texture alpha against what the original combiner/`MObjSub`
   alpha path would actually produce for this surface, per `TODO.md`
   Phase D's updated entry.
3. **`R0.6`'s remaining, less-scoped items** — "material tables resolved",
   "primitive color verified", "environment color verified", "combiner
   behavior verified" (partially covered by RE-039/RE-043 already but not
   marked), "fog verified" (D-025 already found fog is used twice
   game-wide, likely low-value), "unsupported material behavior
   identified". Read `PLAN.md` R0.6's acceptance list fresh rather than
   assuming — the pattern that has paid off repeatedly this session
   (RE-065's lighting angle, RE-068's geometry mode, RE-069's render mode)
   is: find where `refs/ssb-decomp-re/src/sys/rdp.c`'s reset list sets the
   *default* for whatever's being checked before assuming `mesh.rs`'s
   current "unset means declined" fallback already matches it.

**Before trusting any on-device comparison**, delete
`psp/target/mipsel-sony-psp/release/EBOOT.PBP` (or otherwise confirm a
real rebuild happened) — RE-070 was fooled once by `--no-build` reusing a
stale binary from an earlier diagnostic. And don't stop at pixel
statistics either: RE-071 found a case where "less measured noise" was
paired with a visibly worse result (blown-out highlights) — look at the
actual image *and* the numbers, neither alone is sufficient.

`TODO.md` Phase G's "texture streaming" item is also a strong, independent
candidate — packed texture VRAM is at 1170.9 KiB (1.7x the ~700 KiB
budget, `docs/memory.md`) after RE-067's mirror fix and RE-070's dither
blur, no longer optional headroom. `R0.4`'s own remaining item ("all
missing palette cases resolved") is already fully attributed to `R0.7`'s
file-86 long tail, so R0.4 has no further independently-actionable work.
`R0.7` remains technically `IN_PROGRESS` but its remaining scope is an
accepted long tail — only worth revisiting if the upstream decompilation
types `llITCommonDataNBumperWaitMObjSub` or Spin Attack's `WPAttributes`
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

## 2026-09-03 — R0.6: RE-070's dither fix does not make blend safe, two leads ruled out (RE-071)

* Re-enabled `GuState::Blend` from `flags::TRANSLUCENT` (the exact code RE-069 deferred) as a reversible experiment, rebuilding clean from a deleted `EBOOT.PBP` first (per RE-070's own stale-binary lesson)
* Re-tested against Dream Land's canopy-highlight texture now that RE-070 pre-blurred it: objective pixel statistics showed ~4% less local noise, but the actual image was worse, not better — blown-out, oversaturated highlights erasing the flowers and most other detail, a different failure mode from RE-069's original checkerboard
* Hypothesized unpremultiplied-alpha blurring as the cause (`box_blur_wrapped` averages RGB/alpha independently, a known way to leak "invisible" colours from transparent texels); implemented a premultiplied variant as a temporary experiment and reran — identical blown-out result, ruling this out too
* Both experiments (blend-enable, premultiplied blur) fully reverted before commit; `git status` clean of them
* Updated `psp/src/meshdraw.rs`'s permanent comment on the deferred `TRANSLUCENT` flag to record both ruled-out hypotheses, so a future session doesn't re-run either
* `cargo test --workspace` — 354 passing, unchanged (no production code shipped this pass, only a comment update and documentation)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `cargo psp --release` — builds clean (comment-only change)
* Result: RE-071 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.6's "blending verified" item updated with the narrowed-but-still-open status; `TODO.md` Phase D updated to match
* Affected subsystem: `psp/src/meshdraw.rs` (comment only) — plus documentation; no functional/runtime change
* PPSSPP: tested extensively this pass (two reversible experiments, both reverted), no regression in the shipped (unchanged) behavior
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.5: canopy dither measurably softened, not fully fixed (RE-070)

* Ran a reversible on-device A/B (`sceGuTexFilter(Nearest, Nearest)` vs the existing `Linear`/`LinearMipmapLinear`, screenshotted, reverted): filtering alone measurably helps a little (Nearest is visibly blockier) but does not turn the dither into a smooth gradient
* Tested conversion-time dither resolution in two steps: box-blurred (3x3, wrapped) the two canopy textures (file 103, offsets `0xE20`/`0x5F0`) and requantized back to their 16-entry CI4 palette — no visible change (blur mostly snaps back to the same two entries); packed the same blurred image unquantized (`Psm8888`) instead — this is where the real effect is
* First on-device look at the unquantized-blur result appeared to fully fix the canopy (a smooth patch replacing the checkerboard); caught this as a methodology error before trusting it — `--no-build` had reused a stale `EBOOT.PBP` left over from the filtering A/B diagnostic, not the actual candidate build. Deleted the binary, rebuilt from scratch (confirmed via `git diff` showing zero change to `meshdraw.rs`), and re-measured
* Measured objectively this time (mean absolute difference between adjacent pixels, a dither-noise proxy) rather than judging a screenshot by eye: ~40% less local noise on a clean canopy patch (8.5→5.1), ~19% over the whole visible canopy region (9.4→7.6, diluted by untouched flowers/background) — real, but a partial improvement, not a full fix
* Implemented `crates/ssb-rom/src/texture.rs::box_blur_wrapped` (3 unit tests: flat image is a no-op, a properly-sized tiling checkerboard averages to the true midpoint, wrapping reaches across the border) and `tools/romtool/src/main.rs`'s `NEEDS_DITHER_BLUR` — a short, explicit, named allowlist of exactly the two Dream Land canopy textures, deliberately not a general "detect dithering" heuristic
* `cargo test --workspace` — 354 passing (was 351)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `romtool pack` — 4250.0 KiB (was 4138.2), packed texture VRAM 1170.9 KiB (was 1059.0, +112 KiB for the two named textures)
* `cargo psp --release` — builds clean, rebuilt from a deleted `EBOOT.PBP` this time, not trusted from cache
* `tools/run-ppsspp.sh` — Dream Land renders at 60 FPS, clean log
* Result: RE-070 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.5 (still unchecked, now with measured evidence), `TODO.md` Phase C, `docs/rendering.md`, `docs/porting-status.md`, `docs/memory.md`, `DECISIONS.md` D-003 updated with the new VRAM figure
* Affected subsystem: `crates/ssb-rom/src/texture.rs` (new function), `tools/romtool/src/main.rs` (`convert_texture`) — code change scoped to exactly two named textures
* PPSSPP: tested this pass extensively, including catching and correcting a stale-build methodology mistake
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.6: alpha test shipped, translucency detected but deferred (RE-069)

* Wrote a temporary probe (`crates/ssb-rom/examples/tmp_rendermode_scan.rs`, deleted before commit) decoding `G_SETOTHERMODE_L`'s alpha-compare and render-mode fields archive-wide: alpha compare nearly even (278 `G_AC_NONE` vs 269 `G_AC_THRESHOLD`); 12 distinct render-mode values across 360 non-default `G_SETRENDERMODE` commands
* Initially checked `FORCE_BL` as the "needs blending" signal — wrong: `G_RM_OPA_SURF` (the RDP reset's own opaque default) sets it too, with an equation that evaluates to no real blending; found the correct signal (blend equation reads the framebuffer weighted by `1 - alpha`) by reading `gbi.h`'s `GBL_c1`/`GBL_c2` macros directly and cross-checking against `refs/BattleShip`'s interpreter, which tests the identical bit positions
* Measured with the corrected signal: 130/360 (36.1%) cutout (`TEX_EDGE`-family) surfaces, 52/360 (14.4%) genuinely translucent
* Consulted `refs/sf64-psp` (a real, shipped N64-to-PSP port doing this same RDP-to-GE translation at runtime) for the actual approximation to use: `sceGuAlphaFunc(GU_GREATER, 0, 0xFF)` for cutouts, standard `sceGuBlendFunc(GU_ADD, GU_SRC_ALPHA, GU_ONE_MINUS_SRC_ALPHA)` for translucency
* Implemented `crates/ssb-rom/src/mesh.rs`'s `alpha_test`/`translucent` decode (`render_mode_is_translucent` mirrors the GBI macros' bit layout directly), `pack.rs`'s `flags::ALPHA_TEST`/`TRANSLUCENT` (pack version 8→9), and `psp/src/meshdraw.rs`'s `sceGuAlphaFunc`/`sceGuBlendFunc` wiring
* Found a real bug via on-device testing before shipping: 46/380 `alpha_test` and 7/362 `translucent` primitives had no texture bound, so they were testing/blending against a packed-normal byte (lit vertices' alpha is not a coverage value — `push_vertex`'s own doc comment) instead of real coverage; this visibly deleted Dream Land's decorative flower triangles. Reproduced the bug, fixed it (`material_now()` now gates both flags on `texture.is_some()`), and confirmed the flowers return with a targeted before/after diff
* Found a second bug specifically in `translucent`, isolated by toggling `AlphaTest`/`Blend` independently: enabling real blending on Dream Land's canopy-highlight surface (file 104 lists at `0x708`/`0x820`/`0xA78`, texture at file 103 `+0x5F0`) produced a checkerboard, not a soft highlight — the render mode itself is genuinely translucent per the decomp (confirmed bit-for-bit against `GBL_c1(CLR_IN, A_IN, CLR_MEM, G_BL_1MA)`), so this is not a detection bug but the same open dithered-CI4/coverage problem RE-053 already found for the canopy's opaque path
* Decision: shipped `alpha_test` (clean on device, matches a validated reference); left `translucent` detected and packed but **not** wired to `GuState::Blend` in `meshdraw.rs`, with a comment explaining why, rather than shipping an unverified visual change to the project's primary test scene
* `cargo test --workspace` — 351 passing (was 347)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `romtool pack` — 4138.2 KiB (was 4138.1), pack version 9
* `cargo psp --release` — builds clean
* `tools/run-ppsspp.sh` — Dream Land renders at 60 FPS, clean log, pixel-identical to the pre-alpha-test baseline in the canopy region
* Result: RE-069 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.6 ("alpha behavior verified" checked, "blending verified" stays open with a concrete pointer), `TODO.md` Phase D, `docs/porting-status.md` updated to match
* Affected subsystem: `crates/ssb-rom/src/mesh.rs`, `crates/ssb-rom/src/pack.rs`, `psp/src/meshdraw.rs` — code change, affects every object with a non-default render mode, not one file
* PPSSPP: tested this pass extensively (including deliberate bug reproduction), no regression in the shipped feature
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.6: the archive-wide geometry-mode default was backwards (RE-068)

* Wrote a temporary probe (`crates/ssb-rom/examples/tmp_zbuffer_scan.rs`, deleted before commit) reading the built pack's `PrimDesc::flags` archive-wide: `Z_BUFFER` set on only 6 of 3426 packed primitives (0.17%), most other flags similarly near-zero
* Read `refs/ssb-decomp-re/src/sys/rdp.c`'s `sSYRdpResetDisplayList` — clears every geometry mode bit then sets `G_ZBUFFER | G_SHADE | G_CULL_BACK | G_SHADING_SMOOTH`; `syRdpResetSettings` plays it, called from `taskman.c:308` (the per-frame graphics task scheduler) — confirmed this is a once-per-frame default, not per-object or per-list
* Recognized the same structural shape as RE-021's lighting finding, one level up: a per-list converter can't see state some other code set earlier (there, per-object; here, per-frame)
* Added `MeshMaterial::rdp_default()` (`cull_back: true, smooth: true, z_buffer: true`) and made `crates/ssb-rom/src/mesh.rs`'s `State::new()` seed from it instead of an all-off `Default`
* Wired `psp/src/meshdraw.rs`'s `apply_material` to toggle `GuState::DepthTest` per primitive from the `Z_BUFFER` flag (already packed by `pack.rs`, never read on the device side until now), mirroring the existing `CullFace` toggle
* Re-ran the archive-wide scan after the fix: `Z_BUFFER` 6→3384 of 3442 (98.3%), `CULL_BACK` 86.3%, `CULL_FRONT` 0.1%, `SMOOTH` 76.5% — the shape a real game's geometry should have
* Added `crates/ssb-rom/src/mesh.rs::a_list_with_no_geometry_mode_command_draws_under_the_rdp_reset_default`, pinning all four defaults (and that `cull_front`/`lit` stay off) from a display list with no geometry-mode command at all
* `cargo test --workspace` — 347 passing (was 346); none of the 346 pre-existing tests broke, since every existing geometry-mode test already set an explicit command
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `romtool pack` — 4138.1 KiB (was 4137.6)
* `cargo psp --release` — builds clean
* `tools/run-ppsspp.sh --no-build --seconds 8` — Dream Land renders correctly at 60 FPS, clean log; before/after pixel diff shows a small, localized change (2199 of 522240 pixels, ~0.4%) around thin/double-sided decorations, not a wholesale change or missing geometry
* Result: RE-068 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.6 ("depth state verified", "culling verified" checked, with leads recorded for alpha/blending/fog defaults from the same reset list), `TODO.md` Phase D, `docs/porting-status.md` updated to match
* Affected subsystem: `crates/ssb-rom/src/mesh.rs` (`State::new()`/`MeshMaterial::rdp_default()`), `psp/src/meshdraw.rs` (`apply_material`) — code change, affects every object this project converts, not one file
* PPSSPP: tested this pass, no crash/regression, small localized visual change as expected
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.5: `G_TX_MIRROR` traced to Dream Land's canopy and fixed (RE-067)

* Ran `romtool textures --file 104` — reproduced RE-053's exact canopy binding (`64x64 Ci/Bits4 <- file 103 +0xE20`, `3.70x1.36 repeats`)
* Wrote a temporary probe (`crates/ssb-rom/examples/tmp_canopy_tile.rs`, deleted before commit) decoding file 104's display lists directly and found the one binding it (offset `0x798`): `SetTile cm_s=3 cm_t=3 mask_s=6 mask_t=6` — mirror+clamp on both axes, exactly matching the texture's 64-texel period
* Confirmed the wrap boundary mattered before implementing anything: temporarily changed `psp/src/meshdraw.rs`'s `sceGuTexWrap(Repeat, Repeat)` to `Clamp, Clamp` (2-line, reversible), rebuilt, screenshotted — the canopy's repeating pattern disappeared entirely under `Clamp`, a dramatic difference proving the boundary drives real visible output; reverted immediately after
* Implemented the real fix: `crates/ssb-rom/src/texture.rs::mirror_extend` pre-bakes a mirrored copy of the decoded image per mirrored axis (4 unit tests: no-op, S-only, T-only, both-axes-all-four-quadrants); `crates/ssb-rom/src/mesh.rs` gained `TextureRef::mirror_s`/`mirror_t` gated on a nonzero mask (1 unit test reproducing Dream Land's exact `SetTile` parameters); `tools/romtool/src/main.rs`'s `convert_texture` applies it before mip generation
* Measured the real scope and cost (temporarily instrumented `romtool textures`' existing dedup loop, reverted before commit): **187 of 638 packed textures (29%)** carry `G_TX_MIRROR` on at least one axis, not just Dream Land's; packed texture VRAM rose from 763.2 KiB to **1059.0 KiB (+39%, 1.5x the ~700 KiB budget)**
* Given the scale of that tradeoff, stopped and presented the finding + four options to the user (ship fully / paletted-formats-only / document-only / revert) rather than deciding unilaterally — user chose to ship it fully
* `cargo test --workspace` — 346 passing (was 341)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `romtool pack` — 4137.6 KiB (was 3841.0 KiB), 218 of 637 textures carry mip levels
* `cargo psp --release` — builds clean
* `tools/run-ppsspp.sh --no-build --seconds 8` — Dream Land renders at 60 FPS, clean log; before/after pixel diff confirms substantial real change in the canopy region (not a no-op), though the dithered pattern still looks busy at native resolution — expected, since RE-053's separate magnification/dithering diagnosis is untouched by this fix
* Result: RE-067 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.5, `DECISIONS.md` D-003, `TODO.md` Phase C/G, `docs/rendering.md`, `docs/porting-status.md`, `docs/memory.md` updated to match. `PLAN.md` R0.5's "Dream Land canopy discrepancy resolved" item stays unchecked — one real contributing cause fixed, the magnification/dithering component is not
* Affected subsystem: `crates/ssb-rom/src/texture.rs`, `crates/ssb-rom/src/mesh.rs`, `tools/romtool/src/main.rs` — code change, plus a significant VRAM-budget documentation update
* PPSSPP: tested this pass, no crash/regression, visually different as expected
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.5: wrap/clamp verified correct, Mirror recorded as a deviation (RE-066)

* Wrote a temporary example (`crates/ssb-rom/examples/tmp_wrap_mode_scan.rs`, deleted before commit) decoding every display list archive-wide and tallying `cms`/`cmt` on tile 0 (the render tile `current_texture()` reads): 754 `G_SETTILE` commands found, 0 using plain `wrap`/`wrap`
* Cross-tabulated per-axis against that axis's own `masks`/`maskt`: every single instance where an axis requests clamp or mirror also has that axis's own mask nonzero — 0/754 counterexamples on both axes independently
* Read `refs/BattleShip/libultraship/src/fast/interpreter.cpp:3245-3251` (strips `G_TX_CLAMP` when the tile is genuinely periodic) and `:3952-3956` (forces `Clamp` for an unmasked `WRAP` tile) — confirmed real RDP tile addressing only wraps/clamps meaningfully in combination with the mask, not from the two-bit field alone; a naive `WRAP`→repeat/`CLAMP`→clamp mapping is wrong on real hardware too, not just the PSP
* Concluded `crates/ssb-rom/src/mesh.rs`'s existing mask-narrowed texture sizing (RE-044) combined with `psp/src/meshdraw.rs`'s hardcoded `sceGuTexWrap(Repeat, Repeat)` already reproduces the correct periodic addressing for every tile-0 texture in this ROM — not a bug, no code change needed
* Quantified the one real gap: `G_TX_MIRROR` on 208/754 (27.6%) tile-0 lists, which `sys::GuTexWrapMode` (`Repeat`/`Clamp` only) cannot represent at all
* Corrected `meshdraw.rs`'s comment (previously justified `Repeat` only by "UVs run outside 0..1", true but incomplete) to explain the actual mechanism and cite the measurement
* `cargo test --workspace` — 341 passing, unchanged (no functional code changed)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `cargo psp --release` — builds clean (comment-only change to `meshdraw.rs`)
* Result: RE-066 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.5 ("wrap/clamp/mirror behavior verified", "texture tile parameters verified" checked), `TODO.md` Phase C, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: `psp/src/meshdraw.rs` (comment only) — plus documentation; no functional/runtime change
* PPSSPP: not run this pass (no production behavior changed)
* Physical PSP: not tested this pass — see §8 below

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
