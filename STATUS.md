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
`ssb-rom`/`tools/romtool` pipeline (combiner, env/prim colour, lighting,
UV/tile addressing, palette, texture-cache dedup, CI4 packing, mat-anim
attachment) is now confirmed clean by live instrumentation across two
sessions, so root cause is narrowed specifically to the PSP runtime
texture/CLUT bind path (`psp/src/meshdraw.rs`) — still open; five (Yoshi,
Captain Falcon, Ness, Pikachu, Kirby) remain fully untraced. This blocks
resuming the physical R2 matrix (which was otherwise down to only the
PSP-1000-availability blocker below) — do not treat R2's rendering gate as
closed until RE-283 concludes for the remaining fighters.

Last completed: `RE-283` (this session's eleventh follow-up) — **live
instrumentation of the real `tools/romtool` pack path clears `pack.rs`'s
texture interning and `psp_texture.rs`'s CI4 packing for Link's boot defect;
combined with the tenth follow-up's neutral-light/single-cycle-combiner
findings, root cause is now narrowed to the PSP runtime texture/CLUT bind
path alone — still not closed.** Followed the tenth follow-up's own named
next step: added temporary `eprintln!` tracing (gated behind
`RE283_TRACE_ENV`, same convention) in `pack_mesh`'s texture-cache lookup and
`set_texture_mat_anim` call site, then ran the real production
`romtool pack rom.z64` path (whole-ROM) and read stderr for every reference
to file 324 offset `0xCF18` (the boot texture). Findings, all live: (1) the
same 17-field `TexKey` recurred for four display-list offsets in file 324
(`0x36B8`/`0x39E0` — nodes 25/30's own boots — plus `0x70F0`/`0x73D0`, the
same boots under another costume/graph) with zero collision — `convert_texture`
ran exactly once, producing texture index 1245, every other reference a
genuine cache hit; (2) the packed bytes are byte-exact to the raw ROM: hand-
decoded (Python, independent of the project's own decoder) the printed
16-entry palette and 128 bytes of texel data — 11 unique texel indices,
matching the tenth follow-up's `texdump` count exactly, and neither of the
palette's two genuinely mauve entries (indices 8/15) is ever referenced; (3)
texture index 1245 never receives a `set_texture_mat_anim` call anywhere in
the ROM, so `bind_texture`'s dynamic-palette branch is dead for this
primitive, not just unlikely. Given the already-live-confirmed neutral light
pair and single-cycle `TEXEL0*SHADE` combiner, a neutral-coloured light can
only scale brightness, never shift hue — so this texture's own correct data,
correctly bound, cannot render a purple/black pixel at any lighting angle;
the defect must be either a stale CLUT left bound from a different
primitive, or a genuine `sceGuTexImage`/`sceGuClutLoad` behavioural quirk for
this texture's specific 8-byte-row/16×16 shape, both runtime-only and
outside what host-side tracing can reach. All scratch instrumentation
reverted (`git checkout --` on `tools/romtool/src/main.rs`); no code
survives from this step; `cargo test --workspace` reconfirmed clean after
the revert. See `docs/evidence/re/RE-283.md`'s "eleventh follow-up" section;
the tenth follow-up's own findings (node isolation, combiner correction,
env/prim/lighting/UV/palette all clean) still stand unchanged.

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
with the remaining five fighters. For Link: `crates/ssb-rom`'s conversion
*and* `tools/romtool`'s `pack.rs`/`psp_texture.rs` packing are now both
proven clean by live instrumentation across two sessions (tenth/eleventh
follow-ups) — do not re-check combiner/env/light/UV/palette, texture-cache
collision, CI4 packing, or `mat_anim` attachment again, that ground is
covered and the algebra (neutral light × single-cycle combiner cannot shift
hue) rules out a lighting-side explanation too. Move to the one remaining
surface: `psp/src/meshdraw.rs`'s runtime texture/CLUT bind
(`bind_texture`, `DrawState::last_texture`/`forget_texture`). Port the same
live-instrumentation method to PSP-target code — `psp::dprintln!` gated by a
compile-time `option_env!` flag (the PSP target has no runtime env access),
printing the actually-bound `TextureDesc` fields and/or CLUT bytes
immediately before drawing texture index 1245's primitives (re-derive that
index with the same `RE283_TRACE_ENV` `eprintln!` method against the current
ROM if the pack changes) — and run it through
`tools/run-ppsspp-headless.sh`, whose existing stdout/stderr redirect to
`ppsspp-headless.log` already captures `psp::dprintln!` output. Use the
ninth follow-up's `RE283_ONLY_LOCAL_NODE` node-isolate method to reproduce
the defect in isolation first, so the trace has only one primitive's bind
calls to read. Check specifically whether a stale CLUT from an
earlier-drawn, unrelated primitive survives into this draw (Fox's own still-
open hypothesis (b): a stray/stale texture or CLUT binding between
sequential small-texture draws) before assuming a hardware/PPSSPP quirk.
The already-tested-and-rejected CI4 swizzle-threshold hypothesis (this
file's own title, see RE-283.md's "Hypothesis 1") was for a different
symptom archive-wide and its fix is already live in the current pack — do
not re-test that specific hypothesis. Fox, Mario/Luigi, and Samus are
closed. Yoshi, Captain Falcon, Ness, Pikachu, Kirby remain fully untraced:
check each against the same "does the node carry its own authored
PRIM*SHADE colour, a ROM-authored tile-addressing/UV quirk, or (per this
session's Link finding) a bug downstream in texture packing/binding or the
runtime GE bind" pattern first, but do not assume any one conclusion
generalizes — treat that checklist as a starting point, not exhaustive.
Each needs its own trace and, where a colour/geometry source is found,
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
