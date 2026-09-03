# Project Status

**Last updated:** 2026-09-03

---

# 1. Execution State

## Current Milestone

`R0 — Rendering Correctness`

## Current Task

`R0.10 — Material Animation`

## Task Status

`IN_PROGRESS` on `R0.10`. RE-095 (this session) shipped step 8: the
device-side `MaterialAnimator`, the piece the whole RE-086–094 pipeline
had been building toward.

**Design mirrors `StageAnimator`, but the lifecycle differs on purpose.**
A `MatAnimDesc` entry is a property of a *texture*, not a fighter or a
stage layer, so there is no per-object "start" boundary — `start(pack)`
runs once when the pack loads, `tick(pack)` runs every frame for the
pack's whole lifetime, independent of which stage or fighter is shown
(cheap regardless: 33 real scripts, `MAX_MAT_ANIMS = 64` headroom).
Array position mirrors `TextureDesc::mat_anim`'s own index directly — no
separate lookup table. `resolved_palette` clamps into each entry's own
`palette_count` before adding `first_palette`, closing a real risk (an
out-of-range replay reading a *different* `MatAnimDesc`'s own variants in
the shared table) verified via a test proven capable of failing.

**A `no_std` build error caught a real portability gap before it
shipped.** `f32::round()` does not exist in `core` without `std`/`libm`
— `scene.rs` already avoids `libm` for `sin`/`cos` for the same reason.
Fixed with the same "add a half, truncate" trick `mesh.rs`'s own vertex
rounding already uses. This was only caught because `cargo psp --release`
was actually run before calling the work done — `cargo test`/`clippy
--workspace` alone never would have, since the crate's `std` feature is
enabled by default on the host.

**Wired into every real draw path, not just one.** `bind_texture` issues
a second `sceGuClutLoad` after the static one whenever `TextureDesc::
mat_anim` names a live, resolvable entry — riding `apply_material`'s
existing per-frame texture-cache reset for correct cadence, so a texture
with no animation, or an animator with no value yet, keeps its baked
palette rather than showing nothing. Threaded `Option<&MaterialAnimator>`
through six functions (`draw_mesh`/`draw_object`/`draw_object_posed`/
`draw_stage`/`draw_stage_animated`/`apply_material`), matching how
`pack: &Pack<'_>` is already threaded explicitly rather than stashed in
`DrawState`, and updated all four real call sites in `psp/src/main.rs`.

**Verified.** Two new tests in `skeleton.rs`: one ticks a script
reproducing RE-086/087's real archive shape (three `PaletteID` steps then
`SET_ANIM` looping forever) through a full pack round-trip and confirms
every real variant is visited and it keeps cycling rather than freezing;
one proves the neighbouring-table-read clamp is a real regression risk,
not defensive theatre. `cargo test --workspace`: 247 passing (was 245).
`cargo clippy --release` (workspace): clean. `cargo psp --release` +
`tools/run-ppsspp.sh`: builds and runs clean, no panics, Dream Land
pixel-identical at 60 FPS.

**Loaded file 105's own stage (temporary, reverted `stage_index`
override) and confirmed it runs clean on the real device profile** — 464
triangles, one layer, 60 FPS, `tex 0/648` matching the current pack. **Did
not conclusively confirm the palette visibly cycles by screenshot.** The
harness takes one screenshot per independent launch and each launch
restarts the simulation from tick 0, so two screenshots from two separate
invocations cannot isolate "more ticks, nothing else changed" — a
stage-animated floating platform's own motion (RE-050/051, unrelated)
confounded a naive crop comparison. Left honestly open, matching this
file's own already-recorded category of limitation for `R0.12`/`R0.14`'s
remaining items: the mechanism is verified by construction, watching it
happen needs video capture or interactive play, not another screenshot
diff.

Immediately before this, RE-094 (previous session) closed the lead RE-093
(previous session) left open: node 27's `texture_enabled` was `false` for
its whole span despite actively loading textures. Traced `texture_enabled`
node-by-node across the same graph (temporary instrumentation, reverted)
and found it flips to `false` exactly once, inside **node 20's own
list** — `SetCombine → Texture{on: false} → one untextured triangle`, a
self-contained decal with no `G_SETTIMG` of its own. Nothing re-enables it
from node 21 through node 27, yet four of those seven nodes each issue a
complete, independent `G_SETTIMG`/`G_SETTILE`/`G_LOADTLUT`/`G_LOADBLOCK`
chain and draw real triangles — only explicable if texturing is genuinely
active there.

Measured before fixing (temporary bypass on `current_texture()`'s
`texture_enabled` gate, reverted): ignoring the flag outright takes the
pack from 639→648 textures and 25/33→33/33 surviving `mat_anim` scripts,
confirming the exact scope — but a blanket ignore is not *correct*, since
it would also re-texture node 20's own deliberately-untextured decal
using whatever stale binding preceded it. Shipped a narrower rule
instead: `Cmd::SetTimg` now sets `texture_enabled = true` unconditionally
— a display list has no reason to reissue a whole texture chain for
geometry meant to draw untextured, so a fresh `G_SETTIMG` is as strong a
signal as an explicit `Texture{on: true}`. Re-measured with the narrower
rule: **identical result** to the blanket bypass (639→648, 25→33),
confirming it loses nothing the blanket version gained while leaving node
20 (no `SetTimg` of its own) genuinely untouched.

**Also fixed a test that was passing for the wrong reason.**
`texture_disabled_means_no_binding` omitted `Cmd::SetTile` entirely, so
its `None` result was actually caused by a missing tile format, not the
disabled flag its name claims to cover — after this fix it would have
kept passing vacuously. Rewrote it with a complete texture setup plus an
*explicit* `Texture{on: false}`. Added
`a_later_nodes_own_settimg_overrides_an_inherited_texture_off`,
reproducing nodes 20→21's exact real shape, verified capable of failing
(reverted the `SetTimg` change, confirmed the panic, restored). `cargo
test --workspace`: 245 passing (was 244). `cargo clippy --release`
(workspace): clean.

**Result, run against the real ROM: 33 of 33 known scripts now survive**
(321 palette variants, up from 297) — every script RE-089 originally
found. Texture count 639 → 648 (+9, all static/non-animated textures
recovered the same way). Meshes/triangles unchanged; `draws` rose
3447→3494 (more primitives correctly split into their own textured group
instead of merging into an untextured one). `cargo psp --release` +
`tools/run-ppsspp.sh`: builds and runs clean, no panics, Dream Land
pixel-identical at 60 FPS. **The RE-092/093 open question is now fully
closed, not partially explained** — step 7 of this file's own pipeline
list is done.

Immediately before this, RE-093 (previous session) picked up exactly the
open item the previous session's own note flagged: why did only 17 of
RE-089's 33 known `PaletteID`-cycling scripts survive RE-092's pipeline?
Treated as a real unknown, not a rounding error, per this file's own
standing instruction.

Temporary instrumentation (reverted before committing, matching
RE-079/081/089's pattern) ruled out two of the three named candidates
directly: every resolved script's chain index is genuinely called by its
node's own display list, and every node is placed. The gap was 100%
"resolved but never reached by any primitive" — not a texture-availability
miss (`no_texture_at_that_primitive` was 0 throughout).

**Read the raw ROM bytes instead of theorizing further.** Dumped file
105 node 1's actual, recursively-decoded display list and found real
stage data calls three palette-only `MObj`s back to back against *one*
already-loaded texture image — only the first and last reissue their own
`G_SETTIMG`+`G_LOADBLOCK`; the middle one's block is just
`Call → G_LOADTLUT → G_VTX → G_TRI`, deliberately reusing the image
already resident in TMEM and swapping only the palette. `mesh.rs`'s
`Cmd::LoadTlut` handler did not model this: its own comment asserted "the
real texture follows with its own SETTIMG" and nulled the image binding
outright on that assumption. This ROM data falsifies the assumption, and
when nothing reissues `G_SETTIMG`, the null had nothing to restore it —
`current_texture()` returned `None` for the rest of that group's
geometry, which lost not just its `mat_anim` tag but its *texture
entirely* (those triangles packed as flat-shaded, untextured
primitives). This is a real rendering-correctness bug, broader than
material animation: any static, non-animated multi-palette-sharing-one-
image primitive anywhere in the archive was exposed to the same loss.

**Fixed by making `State` remember the real binding, not just the
transient one.** New field `real_timg: Option<(u32, Option<u16>)>` is
updated only by an actual `Cmd::SetTimg` or an `MObj`'s own `sprite`
field — never by a palette-only `MObj`'s injected address — and
`Cmd::LoadTlut` now restores from it instead of clearing. This is more
faithful to real hardware, not a special case: the RDP's texture-image
register has no "unset" state, so restoring the last real value is
correct whether or not a fresh `SETTIMG` follows — when one does (the
ordinary case), it overwrites the restore immediately, so nothing about
the already-working paths changes. Verified the fix can fail: new test
`a_palette_only_mobj_keeps_the_image_a_prior_settimg_bound` reproduces
file 105 node 1's exact shape and fails without the fix (two expected
primitives collapse into one, the second `MObj`'s geometry silently
merging into an untextured group), confirmed, then restored. `cargo test
--workspace`: 244 passing (was 243).

**Result, run against the real ROM: 17 → 25 of 33 known scripts now
survive** (181 → 297 palette variants). Every other pack figure —
meshes, triangles, draws, objects, node placement, and **texture count
(639, unchanged)** — is identical to the pre-fix pack, the expected
signature of a correlation fix (existing textures gaining a correct
palette/`mat_anim` attribution) rather than a new-texture side effect.
`cargo clippy --release` (workspace): clean. `cargo psp --release` +
`tools/run-ppsspp.sh`: builds and runs clean, no panics, Dream Land
pixel-identical at 60 FPS — notable because, unlike every RE-089–092
session, this fix is **not** animation-scoped: it corrects
`Cmd::LoadTlut` archive-wide, so Dream Land was a genuine (not
guaranteed) candidate to change, and didn't.

**Still open: 8 of 33 remain missing**, with a concrete, different,
unchecked lead this session's own diagnostic surfaced: file 105 node
27's `texture_enabled` was `false` for its *entire* span despite the
node actively loading TLUTs and, for 3 of its 7 entries, issuing real
`G_SETTIMG`/`G_LOADBLOCK` pairs — behavior that only makes sense with
texturing genuinely on. That points at `mesh.rs`'s cross-node
`texture_enabled` inheritance (RE-064) itself, a different mechanism
from this session's `Cmd::LoadTlut` fix, and was not investigated
further.

Immediately before this, RE-092 (previous session) solved RE-091's
blocker and wired `romtool`'s real build loop, closing step 6 for real
archive data.

**The fix was in `mesh.rs` all along — no new bookkeeping needed to find
the texture, only to remember the script.** Re-read `State::apply_mobj`
before designing anything new: for a palette-only `MObj` (`sprite: None`,
RE-091's own finding, true for all 33 real cases), `apply_mobj` sets
`timg_addr` to the *palette's* address first — and the display list's own
subsequent `G_LOADTLUT`+`G_SETTIMG` (ordinary commands `mesh.rs` already
walks, nothing `MObj`-specific about them) load the TLUT and then
overwrite `timg_addr` with the real texture image address. By the time a
primitive is emitted, `State::current_texture()` already resolves the
correct texture through this existing mechanism. The only real gap was
remembering *that a script drove this palette* across those same
intervening commands — a bookkeeping problem, not a texture-identification
one.

Added `MeshMaterial::mat_anim: Option<MatAnimRef>` (`{ source_file,
script }`, identity only — the same "where, not the decoded bytes"
division `TextureRef` already draws) and a parallel `mat_anims` slice on
`SequenceItem`/`State`, indexed by the same segment-`0x0E` heap index
`mobjs` already is. Set inside the *same* `if let Some(palette) = m.palette`
branch that sets `timg_addr` in `apply_mobj` — not merely "when a script
is present" — so a *later*, unanimated palette-bearing `MObj` correctly
clears a stale marker instead of leaking it onto whatever texture ends up
bound next. `forget_texture` clears it too, for the same reason it
already clears `palette_offset`.

**Verified the clearing rule can fail, not just that it compiles.** A new
test builds two `MObj` calls in one node — first animated, second not,
each binding a different texture — and asserts the second primitive's
`mat_anim` is `None`. Confirmed this test can actually fail: reverting
`apply_mobj` to the naive `if mat_anim.is_some() { ... }` form (RE-091's
own original sketch) made it fail with the stale reference still
attached, before reverting back to the fix. A second test confirms the
positive case. `cargo test --workspace`: 243 passing (was 241), all 48
pre-existing `mesh` tests and all 35 pre-existing `pack` tests unaffected.

**Wired `romtool`'s real `pack` build loop, not just `mesh.rs`.** Added
`resolve_layer_mat_anims` (per stage-layer graph, same-file only per
RE-089's scope: ticks `MaterialJoint` and calls `mobj::read_palettes`
exactly as RE-089/090 already do) and `convert_mat_anim_palette` (the
same RGBA5551→ABGR8888 conversion `convert_texture` already applies to
the static palette, just per resolved variant). `pack_mesh` now checks
every primitive's `mat_anim` after resolving its texture, dedupes by
script the same way textures dedupe by texel address, and calls
`add_mat_anim`/`set_texture_mat_anim` for real.

**Result, run against the real ROM: 17 of RE-089's 33 known scripts
survived the whole pipeline — 181 palette variants, 23 textures
animated.** Not a guess: every surviving case's entry count agrees
*exactly* with RE-089's own independently-recorded numbers — file 117
contributed both of its scripts, still 16 entries each (the
decomp-matching case); file 114 contributed 6 of 13, still 18 entries
each; file 105 contributed 8 of 18, entry counts (2, 3, 4) still inside
RE-089's own recorded range. That agreement is strong evidence the
pipeline carries data through correctly end to end, not merely that it
runs without erroring. Pack size grew 4311.0 → 4470.3 KiB (+159.3 KiB of
palette blobs). `cargo run --release -p romtool -- pack`: "verified loads
back cleanly". `cargo psp --release` + `tools/run-ppsspp.sh`: builds and
runs clean, Dream Land pixel-identical at 60 FPS — expected, since Dream
Land uses none of files 105/114/117 and nothing on the device side reads
`mat_anim` yet.

`PLAN.md` R0.10's "material state updated correctly" acceptance item is
now checked, with real verified pack data behind it. **Not investigated
this session**: why 16 of the 33 known scripts did not survive
(candidates: unplaced nodes, a display list this project's own discovery
pass never authoritatively reaches, or something else — genuinely
unknown, not swept under the 17/33 success).

Immediately before this, RE-091 (previous session) shipped step 5's pack
format — `MatAnimDesc`/`MatAnimPalette` (a new table pair mirroring
`AnimDesc`/`AnimJoint`) plus `TextureDesc::mat_anim` (filling 4 bytes of
existing padding, no size change), `pack::VERSION` 11 → 12,
`PackWriter::add_mat_anim` deduplicating each driving script's whole
source file the same way `add_anim` already does for joint animation.
Round-trip verified with 3 new unit tests (238 → 241 passing) plus all 35
pre-existing `pack` tests unaffected. `cargo run --release -p romtool --
pack` against the real ROM builds cleanly at the new version and reports
"verified loads back cleanly" — same 639 textures/41 stages/other counts
as before, since nothing populates the new tables yet (a schema-only
change so far). `cargo psp --release` + `tools/run-ppsspp.sh`: builds and
runs clean, Dream Land pixel-identical at 60 FPS.

**Wiring `romtool`'s real build loop to populate the new tables found a
genuine blocker, checked before writing around it.** The natural design
keys a texture's animated palette table by `(data_file, data_offset)` —
the same dedup key `pack_mesh`'s existing texture cache already uses —
via each animated `MObjSub`'s own `sprite` field, the exact same
`MObjMaterial` RE-090 already reads `palettes[]` from. Checked this
against the real ROM before committing to it (temporary instrumentation
on `romtool stages`'s existing replay loop, reverted): **all 33 real
`PaletteID`-cycling `MObjSub`s have `sprite: None`.** A palette-cycling
material never names its own texture image — the texture it applies to
is whichever CI4/CI8 image is already bound at that point in the node's
draw sequence, tracked correctly today only by `mesh.rs`'s own cross-node
material-state threading (RE-064), not by anything recoverable from the
`MObjSub` alone. Populating `TextureDesc::mat_anim` for real needs that
threading extended with an animation marker (architecturally similar to
how RE-073/074 added and threaded `combiner_texture_blend` through
`MeshMaterial`) — a `mesh.rs`-level change, not more `romtool`/`pack.rs`
plumbing. `TextureDesc::mat_anim` deliberately stays `NO_ANIM` for every
real texture rather than guess at a correlation; `git diff --stat` on
`tools/romtool/src/main.rs` is empty — the instrumentation used to check
this was fully reverted, matching RE-079/081/089's established pattern.

Immediately before this, RE-090 (previous session) shipped the read
RE-088 retracted, now that RE-089 (session before that) supplied a sound
bound for it: `mobj::read_palettes(file, sub_at, count)` reads exactly
`count` consecutive `palettes[]` entries, where `count` comes from outside the
struct (RE-089's script-computed bound) rather than being discovered from
local bytes the way RE-088's over-reading walk tried and failed to do.
Each of the `count` entries is validated by the same relocation-backed
check `read_material`'s existing entry-0 logic already uses; any failure
within that count returns `None` for the whole read rather than a
silently-shorter list, since a mismatch between the script's bound and
the array's real extent is a real problem worth surfacing.

Wired into the same `romtool stages` replay RE-089 built, immediately
after computing each `PaletteID` script's bound: reads that script's
actual `MObjSub.palettes[]` using the bound it just computed (now
tracking the node/chain-position indices through `resolve_scripts`'
table so the right `MObjSub` offset is available), and checks the result
is not just present but non-degenerate (every entry pairwise distinct —
a repeated palette across a cycling table's own indices would suggest
the bound or the array is wrong even if the read technically succeeds).

**Result, run against the real ROM: 33/33 succeeded, 0 failures, 0
arrays with a duplicate entry.** Every `PaletteID` script RE-089 found —
file 105's 18 (2–4 entries each), file 114's 13 (18 entries each), file
117's 2 (16 entries each, independently matching the decomp's own
declared array size) — now reads back a fully valid, all-distinct
palette-pointer array using exactly its own script's computed bound.
This is genuine end-to-end validation of the whole chain RE-088 broke
and RE-089/RE-090 rebuilt: decode script → compute bound → read exactly
that many real pointers → confirm they resolve and are not degenerate —
not a plausibility check on one hand-picked example, but 33 independent
cases across three files and three different entry counts.

`cargo test --workspace`: 238 passing (was 234; 4 new `mobj` unit tests
covering exact-count reads, not-reading-past-count, honest failure on an
overshooting count, and a cross-file entry at a non-zero index). `cargo
clippy --release` (workspace): clean. `cargo psp --release` +
`tools/run-ppsspp.sh`: builds and runs clean, Dream Land pixel-identical
at 60 FPS (still nothing wired to rendering or the pack format — this
session is read-path verification only, matching RE-089's own shape).

`PLAN.md` R0.10's step 2 (reading `palettes[1..]`) is now done at the
`ssb-rom`/`romtool` level. What remains is packing this into the runtime
format (step 4: a new table pair, a `pack::VERSION` bump) and the
device-side `MaterialAnimator`/`sceGuClutLoad` wiring (steps 5–6) — both
genuinely larger units of work than this session's, which was entirely
about proving the read path is correct before building anything on top
of it.

Immediately before this, RE-089 (previous session) did what RE-088's own
retraction said was the actual next step: resolve `p_matanim_joints` into
per-(node, `MObj`-chain-position) script addresses *first*, since the
`palettes[]` bound RE-088 needed can only come from there.

Generalised rather than reinvented: `matanim.rs`'s existing
`costume_colors` (fighter costume selection, RE-040) already walks this
exact shape — outer array parallel to `DObjDesc`, each entry a per-chain
script list — it just also evaluates each script's colour tracks in the
same pass. Factored the walk itself into a new public
`resolve_scripts(file, table, nodes, chain_len) -> Vec<Vec<Option<u32>>>`
and rebuilt `costume_colors` on top of it, behaviour-preserving (two new
unit tests, plus the crate's full existing suite unaffected). One
function now serves both known instances of this shape — a fighter's
`p_costume_matanim_joints` and a stage layer's `p_matanim_joints` — since
RE-086 already established they are the same struct laid out in
different files.

Wired it permanently into `romtool stages` (not a temporary reverted
census): resolves each stage layer's `p_matanim_joints` against its
same-file `mobjsub_table` chain lengths, then replays every resolved
script to completion with the already-shipped `MaterialJoint` engine —
the first archive-wide exercise of that engine beyond RE-087's one
hand-picked example. Run against the real ROM: **61 scripts resolved, 0
failures.** The category breakdown on this same-file subset (`PaletteID`
54%, the rest split across `TraU`/`TraV`/`TextureIDCurrent`) is smaller
than but consistent with RE-086's full archive-wide number (172 scripts,
`PaletteID` 71%) — cross-file `p_matanim_joints` tables are still not
attempted, so this is a subset, not a disagreement.

**This closes the loop RE-088 opened.** Ticking each resolved `PaletteID`
script to completion and taking its largest value gives exactly the
`palettes[]` entry count that script will ever need. Two concrete,
cross-checked results: file 117 (`StageMetalFile2`) — the very file
RE-088 cited from the decompilation as its largest known example — 
independently resolves to **16 entries** from the *script's own runtime
values*, matching the decomp's declared `..._palettes[16]` array exactly,
via a completely different method than reading the C source. File 105
(`StageZebesFile2`, 18 scripts needing 2–4 entries) and file 114
(`StageLastFile2`, 13 scripts needing exactly 18 entries) are concrete,
non-Dream-Land stages for the "representative palette-cycling stage"
step this task's own prior note flagged as still needed.

`cargo test --workspace`: 234 passing (was 232). `cargo clippy --release`
(workspace): clean. `cargo psp --release` + `tools/run-ppsspp.sh`: builds
and runs clean, Dream Land pixel-identical at 60 FPS (nothing wired to
rendering or the pack format yet — this session is resolution and replay
only, not consumption).

Immediately before this, RE-088 (previous session) attempted the
concrete next step the session before that had pointed at — extending
`mobj.rs` to read `MObjSub.palettes[1..]`, not just `[0]` — and retracted
it after archive-wide measurement showed it does not work.

Implemented a walk from `palettes[0]`'s array base, accepting each
further entry only if it passed the same relocation-backed validity
check `indirect`'s already-shipped entry-0 logic uses (real intern
pointer, or a zero word with an extern relocation behind it), stopping
at the first slot that failed, capped at 32 entries as a sanity bound.
New unit tests against synthetic fixtures passed cleanly and made the
approach look sound — the fixtures are sparse, single-purpose byte
arrays with nothing else nearby that could accidentally look like a
pointer, which is exactly the condition that does not hold in a real
ROM file.

Before trusting it, measured archive-wide via a temporary `romtool`
census (reverted, not committed, matching RE-079/081's pattern): of 243
palette-carrying materials, **110 (45%) hit the artificial 32-entry cap
outright**. Traced one concretely (file 75 `MVOpeningRunCrash`, `MObjSub`
at `0x2C60`) by dumping every word the walk read: the "entries" were a
perfect arithmetic sequence (`1280, 1240, 1200, ... 280`, stride `-40`,
26 steps) that wraps and repeats identically at index 26 — not a real
palette-pointer table, but the walk wandering into unrelated, densely
pointer-laden neighbouring file data (Vtx arrays, DL fragments,
sub-object pointers) once it ran past the table's true end. Root cause:
`is_ptr` proves a slot was *some* relocated pointer in the original
compiled file, not that it belongs to *this* array — and real files are
dense enough with pointers that a fixed stride past a table's end
reliably keeps finding something that validates.

Reverted the change completely (`git checkout` on `crates/ssb-rom/src/
mobj.rs` and `tools/romtool/src/main.rs`) rather than ship a
measurably-wrong heuristic. Checked whether any purely-local alternative
exists: the decomp's own two real `palettes[]` examples already disagree
on shape (`328_KirbyModel.c`'s is NULL-terminated at index 5;
`117_StageMetalFile2.c`'s 16-entry table has no terminator and runs
straight into the next struct), and a NULL mid-table is not reliably a
sentinel either — some real tables legitimately have a "no palette here"
entry that is not the end. There is no length recoverable from this
array's own bytes. Recorded as RE-088 in `docs/reverse-engineering.md`,
with `PLAN.md` R0.10 updated to reflect the correction — `mobj.rs` itself
is byte-for-byte unchanged from the previous commit.

**This changes the shape of the remaining pipeline.** `STATUS.md`'s own
prior note framed "extend `mobj.rs`" and "resolve `p_matanim_joints`" as
sequential steps 2 and 3. They are not separable that way: the palette
table's true length is only knowable once the driving material animation
script is decoded and its max `PaletteID` payload is known, so reading
`palettes[]` has to happen *at* the point a node's script reference is
resolved, using the script's own bound, not beforehand in isolation.

Immediately before this, RE-087 (previous session) picked up exactly
where RE-086 left off: found which stage to target (a `PaletteID`-cycling
script, via a temporary `romtool` subcommand, reverted) and decoded it
byte-for-byte instead of guessing at the runtime engine's shape from the
track-category census alone.

The real script is a genuine, continuous loop: `SET_VAL_AFTER_BLOCK`
steps `PaletteID` through `0,1,2,3,2,1,0,...`, then `SET_ANIM` jumps
back to the script's own start — not a one-shot key list the way
`matanim.rs`'s existing `colors_at` (built for fighter costume
selection) assumes. `colors_at` declines `JUMP` outright ("a costume
list has no reason to jump"), which was correct for its own use case
and is now confirmed *wrong* for the general one — a real script
depends on that jump to loop forever.

Added `matanim::MaterialJoint`: a persistent, tick-based player reusing
`crate::figatree::Aobj`/`Kind` — the exact interpolation state
`crate::objanim::StageJoint` already plays joint tracks with, including
already-correct `JUMP`/`SET_ANIM` handling this project didn't need to
reinvent — over a unified 15-track window (ten material tracks, then
five colour tracks), so the same engine can play `PrimColor`/`EnvColor`/
`BlendColor` later without a third parallel implementation.
`colors_at`/`Colors`/`costume_colors` (fighter costume selection,
`R0.11`'s own mechanism) are completely untouched — this is a new,
adjacent engine for the general case, not a rewrite of the existing one.

One real subtlety, found and handled rather than assumed away: a
material track's raw word is a genuine `f32` (`PaletteID`'s `0x3F800000`
really is `1.0`), but a colour track's raw word is RGBA bytes
reinterpreted, never arithmetic. Storing both in the same `f32`-typed
slots is only safe because a colour track this project's data actually
uses is always `Kind::Step` (matching `colors_at`'s own pre-existing,
accepted limitation) — `MaterialJoint::track_is_stepped` makes that
condition explicit and checkable, verified by a unit test that a
ramped (non-step) colour track is correctly flagged as untrustworthy
rather than silently trusted.

Verified with 7 new unit tests reproducing the exact real shape:
immediate (`payload=0`) and delayed (`payload=3`) steps, raw
`0x3F800000`-style float words (not integer reinterpretation), and a
`SET_ANIM`-terminated loop ticked twelve times past the script's own
length to confirm it keeps cycling rather than erroring, freezing, or
reading garbage. `cargo test --workspace`: 375 passing (was 368, no
regressions). `cargo clippy --release` (workspace): clean. `cargo psp
--release` + `tools/run-ppsspp.sh`: builds and runs clean under the
real `no_std` PSP target too (new code has to compile there even before
anything calls it), no panics, Dream Land unchanged (nothing wired to
rendering yet).

`PLAN.md` R0.10 gains two newly-checked acceptance items: "animation
data decoded" and "runtime clock implemented". What remains — "material
state updated correctly" and both "verified" items — is real,
substantial work: `mobj.rs` still reads `MObjSub.palettes[0]` only;
nothing resolves `p_matanim_joints` into per-(node, `MObj`) script
references at pack time; no pack table carries any of this; nothing on
the device side ticks a `MaterialJoint` or reloads a CLUT. This session
shipped the *engine*, tested in isolation — the *pipeline* around it
(pack format, `MaterialAnimator` lifecycle, `sceGuClutLoad` wiring) is
next, and is a genuinely different, larger piece of work than decoding
was.

Immediately before this, with every code-reading/lookup item on
`R0.12`/`R0.14` exhausted, this picked up the
first genuinely eligible `TODO` task in `PLAN.md`'s own dependency order
(`R0.11` depends on `R0.10`; `R0.10` itself depends on `R0.6`, adequately
progressed, and `R0.9`, `COMPLETE`) — per `PLAN.md` §13's autonomous rule:
resume an `IN_PROGRESS` task if one is actionable, else select the first
eligible `TODO`.

Started designing the runtime engine assuming R0.10's own framing was
right — that the interesting case is `PRIM`/`ENV`/`BLEND` **colour**
animation, since that is what `matanim.rs`'s existing decoder (built for
fighter costume selection) already reads. Before committing to that
design, dumped one concrete script (Dream Land's own material-animated
layer, file 104) and found it does not touch colour at all — it drives
`nGCAnimTrackTraU` (texture U-translate) and `nGCAnimTrackSetLFrac`
(mipmap LOD blend), a texture-sway effect. That result alone was enough
to stop and measure archive-wide before designing anything further.

RE-086 (this session) walked all 12 stage layers' `p_matanim_joints`
tables down to 172 individual scripts (temporary `romtool` subcommand,
reverted, matching RE-079/RE-081's pattern) and classified each by which
track category it ever sets. A first pass produced impossible numbers
(a track "set" more times than there were scripts) — a script that loops
back via `JUMP` re-executes the same instruction every pass, and the
walker needed to detect a revisited program counter and stop rather than
keep counting until an arbitrary cap. Fixed, the real archive-wide
picture is: **`PaletteID` cycling is 71% (122/172) of stage material
animation**, `TextureIDCurrent` (frame swapping) is 22%, UV translate/
scale/scroll is a meaningful minority, and **colour is under 2% (3/172)**
— the opposite of this task's own original framing. `mobj.rs` already
reads `MObjSub.palettes[0]` only; `palettes[1..]`, needed for the
dominant case, are never read at all currently, confirmed as a real gap.

This matters beyond trivia: implementing R0.10 as originally framed would
have correctly handled 2% of the real need and left the dominant case
(and the cheapest one to implement — the PSP GE's native indexed-texture
format already separates image from CLUT via `sceGuClutLoad`, needing no
new combiner or vertex-recolouring machinery) completely unaddressed,
with nothing that would have caught the gap short of someone noticing a
palette-cycling stage looked static. `PLAN.md` R0.10 moved `TODO` →
`IN_PROGRESS` (real, substantive investigation happened and every
acceptance item's scoping changed, even though no implementation code
exists yet — the same shape `R0.5`/`R0.7` already use for
investigation-heavy progress). No code changed this session on `R0.10` —
the census subcommand was reverted before committing; `git diff --stat`
on `tools/romtool/src/main.rs` is empty.

Immediately before this, on `R0.14`, RE-085 closed the last
actionable-right-now item this file's own previous note named:
"depth mapping verified", the one `R0.14` acceptance item with no
decomp-side constant to look up (the N64's Z-buffer is inherent RDP
hardware behavior, not a game-configurable value like the FOV RE-084
just fixed).

Confirmed `psp/src/gu.rs`'s `sceGuDepthRange(65535, 0)` +
`DepthFunc::GreaterOrEqual` matches the `psp` crate's *own documented*
`sceGuDepthRange` contract exactly: the SDK binding's own doc comment
(`sys/gu.rs:1162` in `psp` 0.3.13) states "the depth buffer is inversed,
and takes values from 65535 to 0" — this project's near=65535/far=0
call is the documented convention, not a workaround invented for a bug.
`DepthFunc::GreaterOrEqual` correctly complements it (larger buffer
value = nearer in this convention, so keeping the greater value keeps
the nearer fragment, same semantic a standard depth test achieves with
`LessOrEqual` under the opposite sign). Read the binding's own
`sceGuDepthRange` implementation to confirm the arguments are consumed
as a plain range remap, not special-cased in a way that could hide a
mismatch. Corroborated on-device: screenshotted Dream Land's stage view
(already-built, already-committed binary — no source change needed for
this check) and inspected every overlapping-geometry case visible (tree
trunk vs. canopy, decorative sprites, platform edges, the fighter
marker) for z-fighting or inside-out rendering — found none.

`PLAN.md` R0.14's "depth mapping verified" item is checked, and
`DECISIONS.md` D-007's previously uncited "Verified working" now points
at RE-085. No code changed — a documentation/audit pass confirming an
already-shipped implementation matches its own SDK's stated contract,
the same shape as RE-072/RE-082/RE-084's non-fix findings. `R0.14` is
now 5 of 7 items checked; the remaining two ("camera transforms
verified", "representative scenes compared") are structurally blocked
on a real game camera system that does not exist yet, not on further
investigation of this kind.

Immediately before this, RE-084 (this session) did the "actionable-right-
now" `R0.14` FOV lookup this file's own previous "Next Eligible Task" note
named directly: read the decompilation's real camera setup instead of
continuing to carry an unsourced constant.

`psp/src/main.rs` called `sceGumPerspective(60.0, ...)` every frame with
no comment, citation, or decision record explaining where `60.0` came
from — checked, and it does not trace to anything. `refs/ssb-decomp-re/
src/gm/gmcamera.c:1191` sets the real default battle camera's
`gGMCameraStruct.fovy = 38.0F`, and `gmCameraAdjustFOV(38.0F)` (the
function that smoothly re-targets the live FOV) is called with exactly
that value from four separate camera-behavior functions in the same
file — only two other call sites (`...PlayerZoom`/`...PlayerFollow`, a
KO/photo-finish zoom and a follow-camera mode) take a different,
caller-supplied situational value. Four-to-two in one file is strong,
non-exhaustive evidence that `38.0` degrees is the real default, and
`60.0` was this project's own unsourced guess, 58% wider than the
original.

Fixed the constant, and — because this interacts with framing, not just
a number in isolation — recomputed the two `FIT` constants
(`psp/src/main.rs`'s stage-view and object-view debug-camera distance
calculations) that were themselves derived from the *old* 60-degree FOV
(`1/tan(30°) ≈ 1.733`, by their own existing comments) to their
38-degree equivalent (`1/tan(19°) ≈ 2.904`, `≈1.677×` larger) — without
this, every stage and object would visibly zoom in and crop at the
viewer's default zoom purely as a side effect of correcting the FOV,
an unrelated regression this fix should not introduce. Verified by
screenshot, not just arithmetic: Dream Land's stage view before
(60°/`FIT=1.733`) and after (38°/`FIT=2.904`) frames the whole stage the
same way at the default zoom. `cargo psp --release` clean, fresh
`tools/run-ppsspp.sh` run clean (no panics), `cargo test --workspace`
unaffected (`psp/` has no host test suite, excluded from the workspace
per `Cargo.toml`), `cargo clippy --release` (workspace) clean.

`PLAN.md` R0.14's "projection matrix verified" item is now checked — the
FOV term is sourced from the decompilation, joining RE-082's already-
verified aspect/viewport terms. Left open: "depth mapping verified"
(D-007's evidence is thinner than the others and wasn't re-audited),
"camera transforms verified" and "representative scenes compared" (both
need a real game camera system, which doesn't exist yet — sourcing the
FOV *value* is not the same as reproducing the camera's actual
positioning/movement behavior).

Immediately before this, RE-083 (this session) picked up the concrete,
ready candidate this file's own previous "Next Eligible Task" note
pointed at once `R0.14`'s aspect-ratio work (RE-082, below) closed out
what it could reach without a real game camera.

Checked `R0.12`'s two specific open worries directly. First, "the
decomp's `rot_mode` choice between matrix kinds 45/46" turned out to be
a non-issue rather than an unmodelled gap: `gcDecideDObj3TransformsKind`
(the function with the actual `rot_mode` branch) is only ever called
from `gcSetupCustomDObjs`, the runtime/dynamic transform path RE-063
already ruled out of this project's scope. Read `gcSetupCommonDObjs`
directly (the *only* ROM-driven path, per RE-063) and confirmed it maps
`0x4000`→`Kind46`/`0x2000`→`Kind48` unconditionally, with no `rot_mode`
branch in it at all — kind 45 (and 47, 49) are structurally unreachable
from any `DObjDesc` array, so there was nothing left to model.

Second, ran an archive-wide census (temporary `romtool` subcommand,
reverted, matching RE-079/RE-081's pattern) of every billboard-flagged
node's own primitives: 109 nodes (not the `81` `PLAN.md` had stated —
stale since RE-062 added `RecalcRotRpyRSca` billboards; `STATUS.md`'s
own history already had 109, just never propagated to `PLAN.md`, now
fixed), 118 primitives. `z_buffer` is on for **100%** of them — depth
behavior for billboards is unambiguous and matches RE-068's RDP-reset
default with zero exceptions. `alpha_test` (28.8%) is the same
already-shipped RE-069 mechanism, nothing further needed. `translucent`
(29.7%) is the interesting number: it is **not** a new problem, it is
RE-069/RE-071's already-known, still-unresolved gap (deferred after
producing a checkerboard on Dream Land's own canopy-highlight surface)
— but billboards hit it at roughly **double** the archive-wide rate
(14.4%), meaning that gap's priority should be weighted by billboards
specifically, not treated as a uniform archive-wide long tail.

`PLAN.md` R0.12 gains four newly-checked acceptance items: "billboard
types enumerated" (RE-063), "camera-facing transforms verified"
(RE-049, already existed, just never checked off), "depth behavior
verified" (this session), and stays `VERIFYING` overall since "scale
verified", "orientation verified", "texture orientation verified" and
"all flagged billboard nodes verified" remain open — this census
measured render *state* distribution, not per-node visual correctness
beyond RE-049's own Dream Land spot check. No code changed; the
temporary subcommand was reverted before committing — `git diff --stat`
on `tools/romtool/src/main.rs` is empty.

Immediately before this, RE-082 (this session) picked up the fresh,
unblocked candidate this file's own previous "Next Eligible Task" note
pointed at, since both `R0.6` (blocked on `R0.7`) and `R0.5` (RE-081,
just below, now looking like it needs real hardware) ran into real
blockers this session.

Re-audited RE-034's own long-standing loose end: after fixing the real
viewport/projection mismatch it found, RE-034 reported a residual
6.6% width/height error on the fighter's collision-diamond marker
(`1.000` measured against `0.938` expected) and never explained it.
Three independent re-measurement attempts this session — the same
default-zoom screenshot with threshold-sensitivity analysis, and a
temporary (reverted) `cam_distance` override in `psp/src/main.rs` for a
bigger on-screen marker at two different zoom levels — produced three
different ratios (`0.82`, `0.90`–`0.95`, `1.14`–`1.16`), straddling both
`1.0` and the expected `0.9375` depending on method. This showed the
marker (20–80 pixels depending on zoom) is too small for the
single-digit-percent precision RE-034's own number implied, before
concluding anything about a bug either way.

Resolved it by reading the code instead of continuing to fight the
screenshot: `psp/src/gu.rs`'s `Gpu::init` (which sets `sceGuViewport`/
`sceGuScissor`) and `psp/src/main.rs` (which sets the projection's
`aspect` parameter) both call the *same* `coord::pillarboxed_viewport()`
and use its output directly — the exact two values RE-034's original bug
had disagreeing cannot diverge again by construction. Checked the `psp`
crate's `sceGumPerspective` binding itself (`sys/gum.rs`, VFPU assembly):
it computes `m.x.x = cot(fovy/2)/aspect`, `m.y.y = cot(fovy/2)` — the
standard textbook symmetric-frustum formula, no quirk. Combined with
`coord.rs`'s own existing passing unit test pinning `pillarboxed_viewport`'s
arithmetic, there is no remaining code path that could produce a real
aspect-ratio defect. Concluded RE-034's residual was pixel-counting noise
on a too-small shape, not a surviving bug — a correction to that entry's
*confidence*, not to its *fix* (which stands, unchanged).

`PLAN.md` R0.14 gains three newly-checked acceptance items: "viewport
verified", "aspect ratio verified", and "N64/PSP resolution differences
explicitly handled". Left deliberately open: "projection matrix
verified" (this audited the aspect term specifically, not the `60.0`
degree FOV constant's own provenance, which is the debug viewer's own
arbitrary choice), "camera transforms verified" (no real game camera
system exists yet to check against the original's), "depth mapping
verified" (D-007's evidence is thinner than RE-034/RE-082's and was not
re-audited this session), and "representative scenes compared" (no
side-by-side N64-vs-PSP reference imagery exists). No code changed — the
temporary `cam_distance` edit and all screenshots were reverted/discarded
before finishing; `git diff` after this session's `R0.14` work is
documentation-only, the same shape as RE-072/RE-076/RE-081.

Immediately before this, RE-081 (this session) was a brief,
self-contained detour into `R0.5` (matching RE-070's own precedent),
picked from this
file's own "push further on the dither" option once R0.6's combiner-shape
work (RE-079/RE-080, below) ran into `R0.7`'s blocker. Resolved RE-053's
long-standing apparent self-contradiction (its UV-span math said the
canopy was "minified", its visual symptom said "magnified") by measuring
Dream Land's two canopy textures *separately* instead of as one case:
`romtool textures --file 104` shows the "gradient" texture genuinely
minified (`3.70×1.36` repeats, exactly RE-053's own number) and the
"highlight" texture magnified on its V axis (`1.56×0.88`, below 1.0) —
RE-053 was right on both counts, just about two different textures the
fix was applied to as if they were one.

Also tested this file's own previously-untried "larger blur radius or
multiple passes" idea directly: a temporary (reverted, not committed)
`romtool` subcommand measured mean adjacent-pixel channel difference on
both canopy textures at 0/1/2/3 `box_blur_wrapped` passes — a second
pass reduces noise a further ~35–40% beyond the already-shipped single
pass (gradient 5.64→3.69, highlight 6.14→3.73). Tested it as a real,
reversible on-device change before trusting the number: rebuilt the pack
with a temporary double-blur edit (no `psp/` source changes needed, only
data differs), took before/after screenshots of the same cropped canopy
region, and found `magick compare` reports a real but small difference
(`MAE` 0.26%, `RMSE` 1.5%) that is not visually distinguishable at the
tested camera distance — the dither pattern looks the same in both
crops. This is the same outcome RE-075 already found for a different
change to these same textures. Per RE-071's standing rule (a measured
improvement alone is not sufficient — the image has to actually look
better), **not shipped**. Reverted the experimental subcommand and the
double-blur edit completely; `git status`/`git diff` confirmed the tree
matched `HEAD` before this investigation, and a fresh repack from the
reverted state matched the previously committed pack.

`PLAN.md` R0.5 gained two newly-checked acceptance items
("magnification behavior identified", "minification behavior
identified") from the disambiguation above, but "Dream Land canopy
discrepancy resolved" stays open — if anything, more clearly blocked on
real hardware than before, since a substantially larger blur change
than RE-070's shipped one still did not surface on screen under PPSSPP.
`git diff --stat` after this session's work is empty except for
documentation (`PLAN.md`, `STATUS.md`, `docs/reverse-engineering.md`) —
this was a measurement pass, not an implementation one, the same shape
as RE-072/RE-076.

Immediately before this detour, RE-080 (this session) fixed the one real
gap RE-079 (below) identified but left open: `(ZERO-ZERO)*ZERO+PRIM`
(1,589 primitives archive-wide), a flat constant colour with no shade or
texel dependence, which neither `combiner_shade_scale` nor
`combiner_texture_blend` can express. Added `combiner_flat_color`,
structurally disjoint from the other two (each requires a different,
mutually exclusive combination of RE-079's presence flags), gated the
same way RE-079 fixed `combiner_texture_blend` — only requires whichever
of `PRIM`/`ENV` the shape actually reads, so a bare `ONE` (28
occurrences) needs neither. Also covers `(ZERO-ZERO)*PRIM+ENV` (9
occurrences, `ENV` alone via a different slot arrangement) for free,
since it is the same underlying arithmetic case, not a new shape needing
new logic.

Wired further than `texture_blend` was left at its own equivalent stage
(RE-073 detected and packed it, RE-074 wired consumption later): since
`TEXEL` provably never enters this shape's formula, `material_now`
immediately forces the primitive untextured (rather than let the GE's
default `Modulate` silently sample and multiply in whatever texture
happened to be bound) and `push_vertex` bakes the resolved colour into
affected vertices, the same content-keyed-dedup-safe mechanism
`prim_color`'s scale and `texture_blend`'s base colour already use.
Packed as `pack.rs`'s `flags::FLAT_COLOR` and `PrimDesc::flat_color`
(`PrimDesc::SIZE` 32→36 bytes, `VERSION` 10→11) — no `psp/` changes
needed, since an untextured primitive with a baked vertex colour already
renders correctly through the existing path.

Repacking the whole archive measured a real, cross-checked side effect:
bound textures **644 → 639**, mip-carrying textures **223 → 221**. Five
textures were referenced only by primitives whose combiner never
actually reads them — previously packed and uploaded for nothing, now
correctly dropped. `cargo test --workspace`: 368 passing (was 364; four
new tests: three unit tests for `combiner_flat_color`, one integration
test through `convert` confirming a textured display list's flat-colour
primitive comes out untextured with its vertex baked). `cargo clippy
--release` (workspace): clean. `cargo psp --release` +
`tools/run-ppsspp.sh`: Dream Land renders at 60 FPS, debug overlay's
texture counter reads `0/639` matching the repack, scene visually
unchanged (Dream Land's own geometry doesn't use this shape). Both
`PLAN.md` R0.6 "primitive color"/"environment color verified" items
still stay open — not because of an uncaught shape any more, but because
`(PRIM-ENV)*TEXEL0+ENV`'s remaining misses are a genuine absence of
`prim_color`/`env_color` on this converter's own state (likely `R0.7`'s
territory), which no further combiner-classification work can reach.

RE-079 (also this session) did the
"systematic accounting of every distinct `SetCombine` shape" RE-073 left
as the open reason "primitive color verified"/"environment color
verified" were unchecked. Temporarily instrumented `mesh.rs`'s
`material_now` (reverted before committing) to log every
combiner-bearing primitive's raw combiner words and whether
`combiner_shade_scale`/`combiner_texture_blend` already recognised them,
then ran it through the real `romtool pack` archive walk: 262,778
combiner-bearing primitives, 97.0% already recognised.

Found and fixed two real bugs in the two misses that mattered, not just
measured them. First: `combiner_shade_scale` evaluated a combiner into a
`k`/`s`/`t`/`st` decomposition and inferred *which* term was present by
checking which was numerically nonzero — indistinguishable from "this
term is present with value exactly black". `(PRIM-ZERO)*SHADE+ZERO`
(RE-039's own "prim_color" mechanism) declined for 1,118 primitives
whose `PRIM` was set to exactly `[0,0,0,255]`, silently rendering
unmodified (non-black) vertex shade instead of the solid black real
hardware always produces. Second: `combiner_texture_blend` required
*both* `PRIMITIVE` and `ENVIRONMENT` to be set even for shapes that only
read one of them — `(ONE-ENV)*TEXEL0+ENV` (125 occurrences) never reads
`PRIMITIVE` at all, but declined whenever `prim_color` merely hadn't
been set elsewhere.

Fixed by giving `Combined` a `_used` presence flag per term, independent
of its numeric value, threaded through `zip`/`sub`/`add`/`mul`; and a
new `combiner_reads(hi, lo, two_cycle, code)` helper so
`combiner_texture_blend` only requires a colour the shape actually
reads. The first version of the presence-flag fix regressed a separate
27-primitive shape (`(ONE-ZERO)*ZERO+SHADE`) by conflating "multiplied
by a real constant that happens to be black" with "multiplied by a
literal, structurally-absent hardware zero" — caught by the archive-wide
before/after census (hit count dropped from 27 to 0), not by a unit
test, and fixed by having `mul` collapse the whole product away only
when the constant side's own value is itself unsourced. Verified with
three new unit tests, `cargo test --workspace` (364 passing, up from
361, no regressions), `cargo clippy --release -p romtool -p ssb-rom`
clean, and a fresh `cargo psp --release` + `tools/run-ppsspp.sh`: Dream
Land renders at 60 FPS, pixel-identical (expected — its own primitives
don't use either fixed shape). Archive-wide effect: 97.0% → 97.5%
recognised. Temporary instrumentation (the `mesh::census` module and a
`romtool combine-census` subcommand) was fully reverted before
committing, matching `AGENTS.md` §17's "do not commit temporary
debugging artifacts" — `git diff --stat` on `tools/romtool/src/main.rs`
is empty; only `mesh.rs`'s real fix and tests remain.

Both `PLAN.md` R0.6 acceptance items stay unchecked: a third real,
uncaught shape (`(ZERO-ZERO)*ZERO+PRIM`, 1,589 primitives, a flat
constant colour with no shade/texture dependence) needs a new
`MeshMaterial` field rather than a classification fix, and
`(PRIM-ENV)*TEXEL0+ENV`'s remaining 3,085/4,580 misses are a genuine
absence of `prim_color`/`env_color` on this converter's own state
(likely `R0.7`'s material-table pairing gaps), not something this pass's
fix could reach.

## Last Completed Task

`R0.7`-adjacent (RE-078, previous session) extended RE-077's method
archive-wide instead of stopping after Kirby's one fix. Ran
`ssb_rom::mobj::search_tables` over all 63 graphs R0.7 still had no
table for; 13 came back with exactly one demand-matching candidate
(the other 50 stayed ambiguous). Checked each of the 13 against its own
file's decompilation with an address-anchored `@ 0x<offset>` match, not
a substring search — that distinction caught a real mistake before it
shipped: a looser first pass "confirmed" two of file 85's candidates by
matching a sub-offset baked into an unrelated symbol's name
(`..._sub_0x108`), not the actual byte address 0x108, and both were
dropped once re-checked properly.

Six survived the stricter check, each confirmed by *both* address and
entry count matching a real, named, typed decompiled symbol: files 22
(`MNPlayersSpotlight`), 69 (`MVOpeningStandoff`), 75
(`MVOpeningRunCrash`), 83/84 (`EFCommonEffects1`/`2`), and 167
(`MNTitle`). File 84's took extra care — the search's candidate address
sat 8 bytes before the decompiled symbol's own start, which turned out
to be `PAD(8)` exactly covering that graph's two genuinely zero-demand
leading nodes, not a mismatch. Fixed via `PartTables::insert()`
(matching RE-059/060/077's established pattern); verified archive-wide:
`romtool mobj` paired 64→70, mismatches held at 0 across 383 nodes (up
from 364); `romtool textures` packed 638→646, failures held at 27 (same
known classes, none new). `cargo test --workspace` (218 passing,
unaffected) and `cargo psp --release` both clean; Dream Land's stage
view re-screenshotted and pixel-identical. The other 7 unique hits
(file 85's two false positives, plus one each in a stage file and two
special-move files landing on still-untyped bytes) were checked and
correctly left unfixed.

`R0.7` — RE-077 (earlier this session) followed up directly on RE-076's
own hedge instead of leaving it
unchecked. RE-076 guessed that several fighters' low measured texture
counts were "almost certainly" undercounts from R0.7's 64 unpaired
`MObj` graphs. Checked it file by file: `romtool mobj --file <id>` for
each of the 11 remaining real playable fighters found **nine with zero
unpaired graphs** (Mario, Fox, Donkey Kong, Samus, Luigi, Jigglypuff,
Captain Falcon, Yoshi, Pikachu) — their low counts are a real low-poly
N64 model, not a gap. Only Kirby (5 unpaired) and Ness (1) had real
gaps. Got the full, untruncated list of all 64 unpaired graphs (not the
CLI's default 12-line summary) and found most are menu/character-select
emblem models, stage files and fighters' special-move files, not core
fighter bodies.

Kirby's largest gap (`JointTree_0x19F08`, 22 nodes) had exactly one
demand-length-matching `--search` candidate (0x18D60) — anchored to real
relocation pointer slots, across a non-uniform demand sequence, not
RE-061's failure mode of one repeated value. Cross-checked against the
decompilation before trusting it: `328_KirbyModel.c:7254` types exactly
that region as a real, fully-typed 24-slot `MObjSub **` array, and
0x18D60 lands precisely on slot 2 with slots `[2..24)` matching the
graph's 22 nodes exactly — not a coincidence. Fixed via
`PartTables::insert()`; verified `romtool mobj --file 328` (paired 2→3,
unnamed 5→4, 0 mismatches across 21 nodes) and a clean `cargo psp
--release` + pixel-identical Dream Land regression screenshot. Re-
measured Kirby's aggregate texture VRAM afterward: unchanged (the
newly-resolved materials reuse textures his other objects already
reference) — this is a rendering-correctness fix, not a VRAM one, so
RE-076's larger point (archive-wide vs. per-scene VRAM shouldn't be
compared directly) still stands, but its "several fighters are
undercounted" hedge was wrong and is now corrected in
`docs/memory.md`/`PLAN.md`/`TODO.md`. Ness's one candidate (5-way
ambiguous, one candidate sitting in a decomp region flagged as
previously mis-typed) and Kirby's other four (2-node, 10-way ambiguous,
possibly further sub-ranges of the same confirmed array) were checked
and left unfixed — suggestive, not conclusive.

`TODO.md` Phase G — RE-076 (earlier this session) measured whether
"texture streaming... required" was actually supported by the number it
was being justified with. `docs/memory.md` compared the packed set's
**1170.9 KiB archive-wide total** directly against the **700 KiB
per-scene budget** — but those are different things, since no single
scene needs every stage, fighter, menu and effect loaded at once.
Walked the actual pack (not the ROM) and measured what one real scene
needs: the largest stage (Dream Land, 137.0 KiB) plus the four largest
of the 12 real playable fighters, texture indices deduped rather than
summed blind, comes to **217.1 KiB** — well under half the budget.
Flagged, at the time, that this was likely an undercount from R0.7's
unpaired `MObj` graphs — RE-077 (above) checked that specific claim and
mostly refuted it. Updated `docs/memory.md`/`TODO.md` to stop treating
"streaming is required" as settled and instead point at what's actually
known: the per-scene need is probably much smaller than the archive-wide
total, and `docs/memory.md`'s already-planned per-scene `AssetArena`
(one load per scene transition, mirroring the original's own loading
pattern) may already be enough without a separate runtime residency
system. No code changed in this pass — it was a measurement correcting
a comparison, not an implementation.

`R0.5 — Texture Sampling Correctness` — RE-075 (earlier this session)
was a brief, self-contained detour picked from the previous pass's own
"push further on the dither" option. Found and fixed a real but small
ordering bug in the canopy dither blur (`tools/romtool/src/main.rs`'s
`convert_texture`): it blurred *after* mirroring, so
`box_blur_wrapped`'s toroidal wraparound sampled the mirrored copy at the
seam instead of the texture's own true periodic neighbour. Reordered to
blur first, mirror second — same cost, more correct. Before trusting it
changed anything, built two packs (one per order, isolated via `git
stash`) and byte-diffed them directly: **6724 bytes differ** between the
two canopy textures' packed data, confirming the change is real, not a
no-op. Then screenshotted before and after anyway, because a byte diff
is not a visual confirmation: the canopy crop was **pixel-identical** at
the debug viewer's default camera distance — the difference is real but
too small (seam-adjacent texels only) and too minified at this distance
to be currently visible. Shipped as a correctness cleanup that costs
nothing, explicitly **not** claimed as progress on RE-053's still-open
dithering discrepancy.

R0.6 — Material System Correctness — RE-074 (earlier this session)
closed the loop RE-073 left open. RE-073's low-confidence deferral was
whether baking a `TEXTURE_BLEND` primitive's flat base colour into its
vertices could corrupt a vertex shared with a normally-shaded primitive
loading the same RSP vertex-cache slot. Reading `crates/ssb-rom/src/
mesh.rs`'s existing `Builder::push_vertex` answered it without new
experimentation: it already folds `prim_color`'s scale into a vertex's
`rgba` *before* deduplicating (`Builder::seen: BTreeMap<MeshVertex,
u16>`, keyed on the already-coloured vertex), and its own doc comment
says the dedup turns a vertex shared by two differently-coloured
primitives into two entries by itself. `texture_blend` baking a flat base
colour the same way inherits that guarantee for free — the "risk" was a
real gap in what had been *checked*, not a real gap in the architecture.

Shipped the bake (mirroring `prim_color`'s branch) and wired
`psp/src/meshdraw.rs`'s `apply_material` to
`sceGuTexFunc(TextureEffect::Blend, ...)`/`sceGuTexEnvColor`. Doing the
wiring correctly surfaced a real, previously-latent bug: `bind_texture`
unconditionally called `sceGuTexFunc(Modulate, ...)` on every texture
change, which would have silently clobbered a `Blend` state set moments
earlier whenever a `TEXTURE_BLEND` primitive's texture changed but its
coarse `flags` word didn't (two primitives can share identical flags with
different target colours). Fixed by removing that hardcoded call and
tracking the blend state in its own `DrawState::last_texture_blend`
field, independent of the existing `last_flags`/`last_texture` fields —
the same reasoning that gives each of those its own field already.

Verified visually, not just by compiling: no existing debug-viewer
control reaches a specific fighter's specific object headlessly, so a
temporary, fully reverted patch to `psp/src/main.rs` forced the viewer
onto object 306 (file 324 LinkModel's own `TEXTURE_BLEND` piece, found by
scanning the pack for the flag). Screenshotted before (via `git stash` of
the wiring changes, rebuilding from a deleted `EBOOT.PBP` each time per
RE-070's lesson) and after: before, a flat monochrome grey shape (the raw
packed-normal byte, no combiner colour); after, the correct grey-to-orange
gradient. Also re-screenshotted Dream Land's stage view (unaffected by
this shape) before and after — pixel-identical, no regression. `PLAN.md`
R0.6's "combiner behavior verified" item is now checked.

R0.6 — Material System Correctness — RE-073 (earlier this session)
measured what `combiner_shade_scale` actually declines. A reloc-anchored
scan (not `Exhaustive`, per RE-072's lesson) over every `G_SETCOMBINE`
found 79 of 1360 (5.8%) reading `ENVIRONMENT`, 72 of those (91%) matching
one shape in some cycle: `(PRIM-ENV)*TEXEL+ENV`, a texture-driven blend
from `ENV` to `PRIM` with no shade term at all — across 28 files,
including three fighters' own base models (Link, Ness, Pikachu). Verified
one occurrence directly against ROM bytes (Link's own model, offset
`0x11670`, `hi=0x00309661 lo=0x552EFF7F`), not just the scan output.
Added `combiner_texture_blend` (`crates/ssb-rom/src/mesh.rs`) to detect
it, gated on a real texture being bound (same reasoning as RE-069's
`alpha_test`/`translucent` gate), and factored the shared two-cycle
evaluation logic out into `evaluate_combiner` (behaviour-preserving).
Shipped detection into the pack format (`pack.rs`'s `flags::TEXTURE_BLEND`
plus two new `PrimDesc` fields, `PrimDesc::SIZE` 24→32, `VERSION` 9→10) —
device-side consumption followed in RE-074, above.

R0.6 — Material System Correctness — RE-072 (earlier this session)
closed the "fog verified" item, and along the way caught its own
near-miss: an archive-wide re-scan (using `scan::Candidates::Exhaustive`)
initially found `G_SETFOGCOLOR` 7 times and `G_FOG` geometry-mode sets 4
times, seemingly contradicting `DECISIONS.md` D-025's existing "twice"
figure. Before treating that as a correction, cross-checked against
reliable, reloc-anchored discovery (`find_root_display_lists`, which only
follows real pointers) — under that, only 2 `SetFogColor` occurrences
survive and 0 `G_FOG` hits. The `Exhaustive`-mode extras were false
positives; D-025's original number was right all along, and the
near-correction would have been the actual mistake. Went further than an
occurrence count to confirm those two are functionally inert, not just
rare: grepped the entire decompilation for `gSPFogPosition` (the call
that gives the RSP a fog range to compute against) — zero results,
anywhere, in any file. Read the one real stage that sets a fog colour
(file 118, `118_StageYosterSmallFile2`, confirmed via `romtool stages` to
be one of the 41 currently-loaded stages) and checked whether its own
`G_SETRENDERMODE` calls ever reference `G_BL_CLR_FOG` — both use
`G_BL_CLR_MEM` instead. `DECISIONS.md` D-025 stands, now backed by
checking that the surrounding machinery doesn't exist either, not just
that the command is rare. `PLAN.md` R0.6's "fog verified" item is
checked. No code changed in that pass — it concluded "correctly not
implemented," not "needs implementing."

R0.6 — Material System Correctness — RE-071 (earlier this session)
followed up on a natural question RE-070 raised: now that Dream Land's
canopy-highlight texture is pre-blurred (RE-070), is RE-069's deferred
`translucent` blend (deferred because it produced a checkerboard on that
same texture) safe to enable? Re-tested directly, as a reversible
experiment (re-enabled `GuState::Blend`, rebuilt clean from a deleted
`EBOOT.PBP` per RE-070's own lesson about stale binaries): **no** — the
result is different and *worse*, blown-out oversaturated highlights
erasing the flowers and most other detail, not the earlier checkerboard.
Objective pixel statistics alone said "~4% less noise," which would have
been a misleading green-light if trusted without also looking at the
actual image — a second methodology lesson layered on RE-070's first one.
Also tested and ruled out unpremultiplied-alpha blurring as the cause (a
premultiplied variant gave an identical result). Both experiments were
reverted before commit; only the negative-result record remains, in
`docs/reverse-engineering.md` and a permanent, updated comment in
`psp/src/meshdraw.rs` so a future session doesn't re-run either
already-eliminated experiment. `PLAN.md` R0.6's "blending verified" item
stays open, narrowed but not closed.

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

**`R0.10` — the engine exists and is tested; the pipeline around it does
not.** RE-086 scoped it, RE-087 shipped and verified `matanim::MaterialJoint`
(step 1 below, done). The concrete next step is the pipeline that feeds
it real data and consumes its output — sized like a small feature, not
a lookup:

1. ~~A tick-based decoder for the material/colour track windows~~ — done
   (RE-087, `matanim::MaterialJoint`, 7 unit tests, verified against the
   real `PaletteID`-cycling shape including its `SET_ANIM` loop).
2. ~~Extend `mobj.rs` to read `MObjSub.palettes[1..]` on its own~~ —
   **tried and retracted (RE-088)**. The struct has no length field, and
   an `is_ptr`-validated walk from `palettes[0]` looked sound in unit
   fixtures but, measured archive-wide against the real ROM, produced
   nonsense for 45% of cases (a repeating arithmetic sequence from
   unrelated neighbouring file data, not a palette table, traced
   concretely in file 75). There is no way to bound this array's real
   length from its own bytes. Do not re-attempt this exact approach
   without a new source of length information — that source is step 3,
   which this step now has to happen *inside*, not before.
3. ~~Resolve `p_matanim_joints` into per-(node, MObj-chain-position)
   script references~~ — done (RE-089, `matanim::resolve_scripts`,
   generalised from the fighter-costume walk `costume_colors` already
   did; wired permanently into `romtool stages`, 61 scripts resolved
   archive-wide same-file, 0 failures).
4. ~~Extend `mobj.rs` to read `MObjSub.palettes[1..]`, bounded by the
   resolved script's own maximum `PaletteID` payload~~ — done (RE-090):
   `mobj::read_palettes(file, sub_at, count)` reads exactly `count`
   entries, `count` supplied externally (RE-089's bound) rather than
   discovered. Verified archive-wide against the real ROM, not just unit
   fixtures: 33/33 `PaletteID` scripts' computed bounds produced a fully
   valid, all-distinct `palettes[]` read, 0 failures.
5. ~~Pack every palette a texture's animation cycles through, plus the
   resolved script reference, into the runtime format~~ — **the format
   half is done (RE-091)**: `MatAnimDesc`/`MatAnimPalette` (mirroring
   `AnimDesc`/`AnimJoint`'s shape, per the prior note's own prediction)
   plus `TextureDesc::mat_anim`, `pack::VERSION` 11→12, round-trip
   verified. **The population half is blocked, on a real measured gap,
   not a design choice left open:**
6. ~~Correlate an animated `MObjSub` to its texture via `mesh.rs`'s own
   state, and finish wiring `romtool`'s build loop~~ — done (RE-092). The
   texture was already resolvable through `mesh.rs`'s existing
   `State::apply_mobj`/`current_texture` (a palette-only `MObj` sets
   `timg_addr` to the palette's own address, and the display list's own
   subsequent `G_LOADTLUT`+`G_SETTIMG` overwrite it with the real texture
   image — no new texture-identification logic needed). Added
   `MeshMaterial::mat_anim` threaded the same way `timg_addr`/`palette_offset`
   already are, verified the clearing rule (a later unanimated palette
   must not leak a stale marker) can actually fail before trusting it, and
   wired `romtool`'s real `pack` build loop (`resolve_layer_mat_anims`,
   `convert_mat_anim_palette`, `pack_mesh` deduplicating by script).
   Archive-wide: **17 of RE-089's 33 known scripts survived the whole
   pipeline** (181 palette variants, 23 textures), every surviving case's
   entry count matching RE-089's own numbers exactly (file 117: both
   scripts, 16 entries each; file 114: 6/13, 18 each; file 105: 8/18,
   2–4 each). Pack "loads back cleanly", 4311.0 → 4470.3 KiB, Dream Land
   pixel-identical (nothing consumes `mat_anim` on-device yet).
7. ~~Investigate why only 17 of 33 known scripts survived~~ — done
   (RE-093): a real bug, not an acceptable gap. Every resolved script's
   chain index is genuinely called on a placed node (the first two
   candidates from this note were ruled out directly); a raw ROM
   display-list dump showed real stage data legitimately calls several
   palette-only `MObj`s against *one* already-loaded texture image,
   reissuing `G_LOADTLUT` per palette with no `G_SETTIMG` after the
   first — and `mesh.rs`'s `Cmd::LoadTlut` handler nulled the image
   binding on an unverified "the real texture follows with its own
   SETTIMG" assumption this ROM data falsifies, dropping the texture
   entirely (not just `mat_anim`) for every entry after the first in a
   shared-image group. Fixed by having `State` remember the last real
   `G_SETTIMG`/`sprite` binding and restoring it after `G_LOADTLUT`
   instead of clearing — verified capable of failing first. Archive-wide:
   **17 → 25 of 33 survive** (297 palette variants), every other pack
   figure including texture count (639) unchanged. This was a genuine
   rendering-correctness bug broader than material animation — any static
   multi-palette-sharing-one-image primitive anywhere in the archive was
   exposed — yet Dream Land still renders pixel-identical (it has none of
   this shape, or none visible at the tested distance).
7b. ~~Investigate why 8 of 33 known scripts still don't survive~~ — done
   (RE-094). Traced `texture_enabled` node-by-node and found it flips
   `false` exactly once, inside node 20's own self-contained untextured
   decal (no `G_SETTIMG` of its own), and nothing re-enables it through
   nodes 21-27 despite four of those seven issuing a complete, independent
   texture chain and drawing real triangles. Measured a blanket bypass
   first (reverted): fixes it (639→648 textures, 25→33 scripts) but isn't
   *correct* (would also re-texture node 20's decal). Shipped the narrower
   rule instead — `Cmd::SetTimg` now sets `texture_enabled = true`
   unconditionally, since a display list has no reason to reissue a whole
   texture chain for geometry meant to stay untextured — re-measured to
   the identical result while leaving node 20 untouched. Also caught and
   fixed an existing test that was passing for the wrong reason (missing
   tile format, not the disabled flag its name claimed). New test
   reproduces nodes 20→21's exact shape, verified capable of failing.
   **Archive-wide: all 33 of RE-089's known scripts now survive** (321
   palette variants), +9 static textures recovered, meshes/triangles
   unchanged. `cargo test --workspace`: 245 passing. Dream Land
   pixel-identical. This closes the RE-092/093 open question completely.
8. ~~A `MaterialAnimator` (mirroring `StageAnimator`'s lifecycle) that
   wraps `MaterialJoint::tick`, resolves the live `PaletteID` value, and
   issues `sceGuClutLoad`~~ — done (RE-095). Starts once at pack load
   (not per-object — a `MatAnimDesc` entry is a texture's property, no
   per-object boundary to restart on), ticks every frame, wired into
   `bind_texture` via a second `sceGuClutLoad` after the static one.
   Threaded `Option<&MaterialAnimator>` through six draw functions and
   all four real call sites in `main.rs`. Caught a real `no_std`
   portability bug (`f32::round()` needs `std`/`libm`) by actually
   running `cargo psp --release`, not just host tests. Two new tests
   (a real-shaped script cycling correctly; the neighbouring-table clamp
   proven to matter). `cargo test --workspace`: 247 passing. `cargo psp
   --release` + `tools/run-ppsspp.sh`: clean, Dream Land pixel-identical.
9. Verify on a stage that actually needs it — **not Dream Land**, whose
   own material-animated layer is a `TraU`/`SetLFrac` texture-sway case,
   not `PaletteID` cycling (RE-086). ~~Finding which of the other 40
   stages has a representative palette-cycling layer~~ — done (RE-089):
   file 105 (`StageZebesFile2`, 18 scripts needing 2–4 palette entries
   each) and file 114 (`StageLastFile2`, 13 scripts needing exactly 18
   entries each) are both concrete, non-Dream-Land candidates, one small
   and one large. **Partially done (RE-095):** loaded file 105's stage
   (temporary, reverted `stage_index` override) and confirmed it runs
   clean on the real device profile, no panics, correct texture count.
   **Not done:** confirming by eye that the palette actually cycles on
   screen — the screenshot harness takes one shot per independent launch
   (each restarting from tick 0), so two separate invocations cannot
   isolate "more ticks, nothing else changed," and a stage-animated
   platform's own motion confounded a naive crop comparison. Needs video
   capture or interactive input, not another screenshot diff — the same
   category of limitation this file already records for `R0.12`/`R0.14`.

`TraU`/`TraV`/`ScrU`/`ScrV` (UV translate/scale/scroll, the next-largest
category after `PaletteID`) and `TextureIDCurrent` (frame swapping, 22%)
are real, measured, but smaller follow-on work — a texture matrix update
per frame for the former, a bound-texture swap for the latter — neither
blocks starting with palette cycling, and neither should be designed
around alongside it in the same pass; `PaletteID` alone is 71% of the
real need.

**Other, now-secondary threads, each already investigated as far as this
session's methods reach:**

1. **The dither/coverage problem (`R0.5`/`R0.6`'s `translucent`) is
   fully tried out for now.** RE-081 tested the last untried idea on
   this file's list ("a larger blur radius or multiple passes") and
   found a real, substantial texture-level improvement that still does
   not surface on screen at the tested camera distance — the same
   outcome RE-075 already found for a different change to these
   textures. RE-053's own remaining suggestion — deciding this on real
   hardware, or rendering the surface in isolation at a controlled scale
   — is now the most credible next step, and physical PSP access is
   `R2`'s territory, blocked behind `R1`. `translucent` itself remains
   the separate closed line RE-071 already established (do not re-run
   either of its eliminated experiments without new evidence).
2. **`R0.6`'s remaining items are blocked on `R0.7`, not on more
   combiner-shape work.** RE-079/RE-080 (this session) classified every
   combiner shape this model can resolve at all into three structurally
   disjoint cases (shade-scale, texture-blend, flat-colour) and fixed
   every misclassification found. What remains — `(PRIM-ENV)*TEXEL0+ENV`
   missing `prim_color`/`env_color` for 3,085/4,580 primitives — is a
   genuine absence on this converter's own per-graph state, most likely
   the same `R0.7` unpaired-`MObj`-graph gap already tracked there.
   `R0.7` itself was already characterized as a long tail after RE-078:
   further progress needs upstream decomp typing or a `--search` result
   narrowing to exactly one candidate, not open-ended investigation.
   Re-running `romtool mobj --search` archive-wide without new
   decompilation coverage would just re-find the same ambiguous/untyped
   set RE-078 already found and correctly left alone.

3. **`R0.14` has no more code-reading/lookup items left.** RE-082
   closed "viewport verified", "aspect ratio verified" and "N64/PSP
   resolution differences explicitly handled"; RE-084 closed "projection
   matrix verified" (the FOV); RE-085 (this session) closed "depth
   mapping verified" (matches the `psp` crate's own documented
   `sceGuDepthRange` convention exactly). 5 of 7 items are now checked.
   What is left — "camera transforms verified", "representative scenes
   compared" — cannot be verified at all until a real game camera
   exists; right now only the debug viewer's free-roaming inspection
   camera does, and per `PLAN.md` §5's gate, gameplay/camera systems are
   downstream of rendering correctness, not the other way round. This
   task is now genuinely blocked on that, not merely under-investigated.
4. **`R0.12`'s remaining items need either a decomp-derived expected
   value or a real camera to test against — the same shape as `R0.14`'s
   remainder.** RE-083 closed "billboard types enumerated", "camera-
   facing transforms verified" and "depth behavior verified", and
   narrowed "alpha behavior verified" to a single, already-tracked
   blocker (RE-069/RE-071's `translucent` checkerboard, now known to hit
   billboards at ~2x the archive-wide rate). What remains — "scale
   verified", "orientation verified", "texture orientation verified",
   "all flagged billboard nodes verified" — all need either a
   decomp-derived expected value per billboard type or a rotated/moved
   camera to test against beyond RE-049's own single Dream Land spot
   check, which the debug viewer's zoom/orbit controls can already do —
   this is device-interactive verification work, not a code-reading or
   archive-census task the way the last several sessions' progress was.

Every code-reading/archive-census-shaped lookup this file has been able
to name is now done. What is left on `R0.12`/`R0.14` needs either
device-interactive verification (moving the debug camera by hand, not
scripting a fixed screenshot) or a real game camera system that does not
exist yet — genuinely different *kinds* of work than the last several
sessions' pattern, not just a next item in the same list.
`RE-069`/`RE-071`'s `translucent` checkerboard (still unsolved, confirmed
by RE-083 to matter more for billboards specifically than previously
weighted) remains the one standing lead of the previous kind, unchanged
from before — worth a fresh pair of eyes on the alpha-channel-provenance
idea `TODO.md` Phase D already suggests, if a future session wants one
more attempt before treating it as needing real hardware too.

**Before trusting any on-device comparison**, delete
`psp/target/mipsel-sony-psp/release/EBOOT.PBP` (or otherwise confirm a
real rebuild happened) — RE-070 was fooled once by `--no-build` reusing a
stale binary from an earlier diagnostic. And don't stop at pixel
statistics either: RE-071 found a case where "less measured noise" was
paired with a visibly worse result (blown-out highlights) — look at the
actual image *and* the numbers, neither alone is sufficient.

`TODO.md` Phase G's "texture streaming" item is **no longer a clear-cut
"required" candidate** — RE-076 found that the 1170.9 KiB figure it was
justified by is an archive-wide total being compared against a per-scene
budget, and a direct measurement of one real scene's actual need
(largest stage + four largest fighters, deduped) came to 217.1 KiB, well
under budget. RE-077 then checked, rather than left standing, RE-076's
own worry that this was a large undercount: mostly it wasn't — 9 of the
11 fighters measured have zero unpaired `MObj` graphs, so their low
texture counts are real, not hidden. One real gap (Kirby's) was found
and fixed, without changing the VRAM total. The 217.1 KiB estimate
should now be trusted more, not less, than when RE-076 first produced
it. `R0.4`'s own remaining item ("all missing palette cases resolved")
is already fully attributed to `R0.7`'s file-86 long tail, so R0.4 has
no further independently-actionable work. `R0.7` remains technically
`IN_PROGRESS`, but the "worth a `--search` pass each" suggestion from
this note's own prior draft is now done: RE-078 ran `--search` over
every one of the (then) 63 remaining unpaired graphs archive-wide in one
pass, not file by file, found 13 with exactly one candidate, and fixed
the 6 that cross-checked cleanly against a typed decompiled symbol
(Kirby's, RE-077, was the first of what turned out to be 7 findable this
way; 57 remain, most of them landing on still-untyped bytes or genuinely
ambiguous candidates with no further discriminating evidence available
right now). Further progress on the remainder needs either upstream
decomp typing (file 86, file 353's Spin Attack, and most of the other
57) or a materially different lead — re-running `--search` again without
new decompilation coverage would just re-find the same ambiguous/
untyped set. `R0.8 — Transform Correctness` is `COMPLETE`.

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

## 2026-09-03 — R0.6: fog verified as correctly unimplemented (RE-072)

* Wrote a temporary probe (`crates/ssb-rom/examples/tmp_fog_scan.rs`, deleted before commit) using `scan::Candidates::Exhaustive`: found `G_SETFOGCOLOR` 7 times and `G_FOG` geometry-mode sets 4 times archive-wide, apparently contradicting `DECISIONS.md` D-025's existing "twice" figure
* Cross-checked with reliable, reloc-anchored discovery (`find_root_display_lists`) before treating that as a correction: only 2 `SetFogColor` occurrences survive, 0 `G_FOG` hits — the `Exhaustive`-mode extras were false positives, a known risk of that scan mode; D-025's original number was right all along
* Grepped the entire decompilation for `gSPFogPosition` (the call that gives the RSP a fog range to compute against) — zero results anywhere, confirming no game code ever configures fog range/scale
* Identified the two surviving occurrences: file 63 (`63_MVOpeningRoomTransition`, the opening movie) and file 118 (`118_StageYosterSmallFile2`); confirmed via `romtool stages` that file 118 is one of the 41 currently-loaded, real stages, not a discarded variant
* Checked file 118's own list (offset `0x3310`) for whether its two `G_SETRENDERMODE` calls ever reference `G_BL_CLR_FOG` — both use `G_BL_CLR_MEM` instead; the fog colour is set and never read by anything in the same list
* Result: `DECISIONS.md` D-025 stands, now verified against the supporting machinery (fog range, blend-equation reference) rather than just an occurrence count; RE-072 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.6's "fog verified" item checked; `docs/rendering.md` cross-referenced
* Affected subsystem: documentation/investigation only, no code changed — the correct conclusion was "already correctly unimplemented," not "needs implementing"
* `cargo test --workspace` — 354 passing, unchanged
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* PPSSPP: not run this pass (no production code changed)
* Physical PSP: not tested this pass — see §8 below

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
