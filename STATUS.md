# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current RE-281 pack, separate from
RE-272's now-closed Mario-face artifact. Three of nine items are now fully
closed (Fox, Mario/Luigi, Samus); Link is node-isolated and its whole
`ssb-rom` conversion path (combiner, env/prim colour, lighting, UV/tile
addressing, palette) is now confirmed clean by live instrumentation, so root
cause is narrowed to pack-time texture serialization or the PSP texture bind
path — still open; five (Yoshi, Captain Falcon, Ness, Pikachu, Kirby) remain
fully untraced. This blocks resuming the physical R2 matrix (which was
otherwise down to only the PSP-1000-availability blocker below) — do not
treat R2's rendering gate as closed until RE-283 concludes for the remaining
fighters.

Last completed: `RE-283` (this session's tenth follow-up) — **live
instrumentation of the real `mesh.rs`/`romtool pack` code path closes off
every ROM-data/conversion-logic theory for Link's boot defect; root cause
narrowed to downstream of `ssb-rom` entirely — still not closed.** Followed
the ninth follow-up's own required next step instead of re-deriving algebra
by hand: added temporary `eprintln!` tracing (gated behind
`std::env::var_os("RE283_TRACE_ENV")`) inside `mesh.rs`'s real
`Cmd::SetEnvColor` handler, `State::apply_mobj`, `State::material_now`, and
the `Vtx`-cache push site, plus one node-id-correlating print in
`tools/romtool/src/main.rs`'s pack loop, then ran the actual production
build path (`romtool pack rom.z64 --file 324`) and read the live trace for
nodes 25/30. Findings, all live not hand-derived: (1) the shared
`SetCombine` word's `two_cycle` flag is actually `false` throughout this
whole node range — the ninth follow-up's "two-cycle
`(TEXEL0*SHADE)*ENVIRONMENT`" read was wrong; it's plain single-cycle
`TEXEL0*SHADE`, like most of the archive; (2) `env_color`/`prim_color` are
genuinely `None` everywhere in this graph, confirmed live, not just in the
static table (also corrected a methodology gap: the ninth follow-up's
node-1/2/4/9/19/20 check never confirmed those offsets belonged to graph
`0x3AE8` specifically — the live trace does confirm it, so the conclusion
still holds, just wasn't actually verified before); (3) `state.material.lit`
is confirmed `true` live; (4) `light1_color`/`light2_color` at both boots
are the neutral `(255,255,255)`/`(76,76,76)` pair, and a raw `MoveWord` dump
shows every node from 22 through 30 explicitly re-authors these same
literal words via its own `G_MW_LIGHTCOL` — deliberate, not a leak; (5) boot
UVs span exactly `[0,16]` texels against the tile's own declared 16×16 size,
no wraparound; (6) the actual 16-entry RGBA16 palette `LoadTlut` names
(`0xB4B8`) was decoded directly and is all brown/tan/muted-mauve, no
purple/black. Every input `mesh.rs` touches for this primitive is clean —
both of the ninth follow-up's open candidates (a live material-animation
script, an imperative `State` bug) are now ruled out. All scratch
instrumentation reverted (`git checkout --` on `crates/ssb-rom/src/mesh.rs`,
`crates/ssb-rom/src/mobj.rs`, `tools/romtool/src/main.rs`); no code survives
from this step. See `docs/evidence/re/RE-283.md`'s "tenth follow-up" section
for the ninth-follow-up writeup this superseded (node isolation, ruling out
texture/light/vertex-colour/asymmetry — those findings still stand).

Before that, `RE-283` (eighth follow-up) — **Samus's chest "black square"
closed via live original-N64 capture, at the same bar Fox's item met.** The
previous follow-up traced the defect to node 12 (the `f320` offset `0xC408`
olive-camo/joint-cap texture) rendered in Samus's real Wait pose, but only
as a structural argument (blue-channel-zero, R/G-ratio agreement between raw
texture and rendered defect) — not an exact combiner reconstruction or a
hardware check. That follow-up closed it with a live capture: the same
`refs/ssb-decomp-re` decomp-rebuild warp technique Fox's fifth follow-up
used (`n64-emulator` Skill), retargeted to Samus (`training_man_fkind =
nFTKindSamus`, `training_com_fkind = nFTKindMario` in `scmanager.c`'s
`dSCManagerDefaultSceneData` reset site, plus the same Close-Up-camera call
in `sc1ptrainingmode.c`'s `sc1PTrainingModeFuncStart`, right after the
fighter-construction loop), landing in the real Training Mode scene on the
first attempt (HUD-confirmed) and sampling the cannon barrel's zero-blue,
`R≈G` gradient directly on real hardware — the same signature the raw
texture and golden already showed. `refs/ssb-decomp-re` reverted and
rebuilt clean afterward. See `docs/evidence/re/RE-283.md` for the full
seventh-through-sixth follow-up chain (Mario/Luigi's button item, the
posed-vs-bind-pose isolate-tool pitfall, and Fox's own live-N64 closure).

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed — neither the Slim nor
the PSP-3000 tested so far is in that RAM class. No dedicated capture scenes
exist yet for full stage/effect coverage or runs longer than 10 minutes.
None of these block R2.2 (closed) — they gate the *physical* R2 matrix only,
and are deliberately deferred until the game structure grows beyond the
asset viewer. RE-272's face-texture bug is fully closed on both PPSSPP and
real hardware — no longer part of the deferred physical-matrix follow-up.

Current verification baseline: `cargo fmt --all --check` clean; `cargo test
--workspace` 617 passed; all 22 deterministic goldens exact against the
rebuilt pack; effects 46/46 manager objects, 35/35 transform + 24/26
material animations, 160/160 particle scripts; 109 billboards, 0 anomalies;
11/11 framebuffer transitions; `romtool texgen --verify` pass; plain
feature-free `cargo psp --release` pass; strict Clippy not clean (3
pre-existing `needless_range_loop` lints, unrelated to this work). This
session (RE-283's eighth follow-up) made no production-code or asset-pack
change — only `refs/ssb-decomp-re` (an external reference checkout) was
temporarily patched and rebuilt to take a live-N64 capture, then reverted
and reconfirmed byte-identical to `baserom.us.z64`. `assets/generated/
ssb64.pak` and `crates/ssb-rom`/`crates/psp` are unchanged from RE-281's
rebuild.

Required next action: finish root-causing Link's boot defect, then continue
with the remaining five fighters. For Link: `mesh.rs`'s own conversion is
now proven clean by live instrumentation (this session's tenth follow-up) —
do not re-check combiner/env/light/UV/palette again, that ground is
covered. Move downstream: trace this exact primitive (file 324, texture
`0xCF18`/16x16 CI4, palette `0xB4B8`) through `pack.rs`'s `pack_mesh`/
texture interning (does dedup key on `(file, address)` rather than content,
and could this tiny texture collide with unrelated content elsewhere in the
shared pak?) and through `crates/ssb-rom/src/psp_texture.rs` and
`psp/src/meshdraw.rs`'s runtime texture upload/bind (a swizzle/stride bug
specific to a texture this small, or this DL's two-tile CI4 loading idiom —
`SetTile(tile=7, ...)` for `LoadBlock` then `SetTile(tile=0, ...)` for the
actual draw — not replicated correctly by the packer). The already-tested-
and-rejected CI4 swizzle-threshold hypothesis (this file's own title, see
RE-283.md's "Hypothesis 1") was for a different symptom archive-wide and its
fix is already live in the current pack — do not re-test that specific
hypothesis, but the general area (texture packing/binding) is now the right
place to look, not RDP state modelling. Fox, Mario/Luigi, and Samus are
closed. Yoshi, Captain Falcon, Ness, Pikachu, Kirby remain fully untraced:
check each against the same "does the node carry its own authored
PRIM*SHADE colour, a ROM-authored tile-addressing/UV quirk, or (per this
session's Link finding) a bug downstream in texture packing/binding" pattern
first, but do not assume any one conclusion generalizes — treat that
checklist as a starting point, not exhaustive. Each needs its own trace
and, where a colour/geometry source is found,
potentially its own original-hardware visibility check via the now
twice-proven `refs/ssb-decomp-re` decomp-rebuild warp technique
(`n64-emulator` Skill, `nFTKindXxx` in `scmanager.c`'s
`training_man_fkind`/`training_com_fkind` fields, same relative patch sites
prior follow-ups used). When isolating a single node for any of these, use
the *posed* skeleton (`draw_object_posed_filtered` with the real
`posed[..posed_len]` array and a node filter), not `draw_object_node`'s
bind-pose-only path — confirmed twice now (Samus's seventh follow-up, this
session's Link trace) that trusting bind-pose node coordinates (e.g. from
`romtool scene --nodes`) to guess which limb is which silently misleads;
always cross-check against the actual posed render first. This project's
`psp/src/meshdraw.rs` already has the real isolate primitive
(`draw_object_posed_filtered`'s `only_node` parameter, built for R0.12's
billboard audit) — reuse it (temporarily `pub(crate)`, plus a compile-time
`option_env!`-read local-node filter threaded into the real fighter-view
draw call, since the PSP target has no runtime env access) rather than
building a new one; revert the exposure after use. Physically confirming
the PSP-1000 class remains the sole *physical*-matrix blocker (unchanged —
no PSP-1000 unit available in this environment) but is secondary to RE-283
now that the renderer's own PPSSPP-software correctness is back in question
for the remaining fighters. Do not start R3 or combat before both RE-283
and R2's physical matrix are closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-269, RE-270, RE-271, RE-272,
RE-273, RE-274, RE-275, RE-276, RE-277, RE-278, RE-279, RE-280, RE-281,
RE-282, RE-283 (see [docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the
full R2.2/physical chain, RE-240–283). Toolchain note: the global `cargo-psp`
install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-281 pack/code (unchanged this session).
Pack `0065c65d92cf805b12f6157259e0e0da9a6ab0f872c01d3aae271c49e277a368`
(28,531.3 KiB). Plain feature-free EBOOT
`9b9bfb8f94730a47ea04e50cb2a75a8d91536bd9b13f74682256a3ebe902ca8c`.
