# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current RE-281 pack, separate from
RE-272's now-closed Mario-face artifact. Three of nine items are now fully
closed (Fox, Mario/Luigi, Samus); for Link, the whole `ssb-rom`/
`tools/romtool` pipeline (combiner, env/prim colour, lighting, UV/tile
addressing, palette, texture-cache dedup, CI4 packing, mat-anim attachment)
*and* the PSP runtime GE bind itself (`psp/src/meshdraw.rs`'s
`bind_texture`) are now all confirmed clean by live instrumentation, on the
node-isolated primitive, across three sessions — root cause narrowed to a
PPSSPP/GE-specific rendering quirk, not yet confirmed against physical PSP
hardware; five (Yoshi, Captain Falcon, Ness, Pikachu, Kirby) remain fully
untraced. This blocks resuming the physical R2 matrix (which was otherwise
down to only the PSP-1000-availability blocker below) — do not treat R2's
rendering gate as closed until RE-283 concludes for the remaining fighters.

Last completed: `RE-283` (this session's twelfth follow-up) — **live
PSP-target instrumentation of `bind_texture` proves the runtime binds the
pack's own byte-identical, correct texture/CLUT for Link's boot defect,
closing the "stale or wrong bind" hypothesis; root cause narrowed to a
PPSSPP/GE rendering behaviour for this texture's specific 8-byte-row/16×16
`PsmT4` shape, not yet checked on physical hardware.** Followed the
eleventh follow-up's own named next step, with two method corrections found
along the way: (1) `psp::dprintln!` (the eleventh follow-up's own plan)
writes straight to VRAM and is invisible to the GE-based headless
screenshot hook — already established by this project's own RE-013/RE-255,
missed when that plan was written; substituted real GE quad draws instead
(not `sceGuDebugPrint`, which RE-123/RE-125 already found corrupts
rendering under a `regression_capture`-family frozen build — hit that too,
before switching to plain untextured `TRANSFORM_2D` sprites). (2)
`RE283_ONLY_LOCAL_NODE`'s node id is object-relative (matching
`romtool scene --nodes`'s own numbering), not the pack-global id
`draw_object_posed_filtered`'s `only_node` parameter expects — needed
`obj.first_node +` added at the call site to actually isolate node 30 again.
Abandoned an initial pixel-exact byte-swatch HUD decode as unreliable (this
build's screen-space sprite coordinates don't map to the captured PNG by a
simple assumed scale) in favour of a coarser, robust pass/fail design:
re-derived the pack's own known-good `TextureDesc`/CLUT for texture index
1245 host-side (`tools/romtool`, temporary subcommand, reverted after use),
cross-checked its two mauve CLUT entries against the eleventh follow-up's
independent raw-ROM decode (exact match, confirming same texture), hardcoded
them as PSP-side constants, and rendered a live match/mismatch verdict as
big green/red GE quads. Result on the node-isolated boot primitive (no other
primitive drawn that frame, so no earlier bind to have gone stale): **all
four quadrants green** — `data_offset`, `palette_offset`, dims/format/wrap/
mat-anim/palette-length, and all 16 CLUT entries are exactly the pack's own
correct baked values, live. Combined with the isolation itself and the
eleventh follow-up's neutral-light/single-cycle-combiner algebra, both
remaining "stale bind" candidates are now closed: no wrong data bound, no
earlier primitive in scope to have left a stale one. All scratch
instrumentation reverted (`git checkout --` on `psp/src/main.rs`,
`psp/src/meshdraw.rs`, `tools/romtool/src/main.rs`); `cargo fmt --all
--check` and `cargo test --workspace` reconfirmed clean after the revert.
See `docs/evidence/re/RE-283.md`'s "twelfth follow-up" section; the tenth/
eleventh follow-ups' own findings (node isolation, combiner correction,
env/prim/lighting/UV/palette/cache/packing all clean) still stand unchanged.

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

Required next action: Link's boot defect is now narrowed as far as PPSSPP-side
tracing can take it — `crates/ssb-rom`'s conversion, `tools/romtool`'s
`pack.rs`/`psp_texture.rs` packing, *and* `psp/src/meshdraw.rs`'s runtime GE
bind (`bind_texture`) are all proven clean by live instrumentation across
three sessions (tenth/eleventh/twelfth follow-ups) — do not re-check
combiner/env/light/UV/palette, texture-cache collision, CI4 packing,
`mat_anim` attachment, or the runtime-bound `TextureDesc`/CLUT bytes again;
that ground is covered on the node-isolated primitive specifically (isolate
via `obj.first_node + 30`, not raw `30` — `RE283_ONLY_LOCAL_NODE`/
`romtool scene --nodes` numbering is object-relative, `only_node` is
pack-global, see RE-283.md's twelfth follow-up for the fuller explanation).
The remaining candidate is a genuine PPSSPP/GE rendering behaviour specific
to this texture's 8-byte-row/16×16 `PsmT4` shape, not a bug in this
project's own code. Confirm or rule that out on physical PSP hardware next
(`psp-hardware` Skill, PSPLink) — build with the same node-isolate technique
(`draw_object_posed_filtered`'s `only_node`, temporarily exposed
`pub(crate)`, threaded through a compile-time `option_env!`-read filter at
the real fighter-view draw call since the PSP target has no runtime env
access; revert after use) so only the boot primitive draws, and check
whether the purple/black patch is visible on real hardware too. If it is
not, this is a PPSSPP-only emulation quirk and Link's item can close without
a code change; if it is, this needs its own hardware-specific investigation.
Two PSP-target-tracing traps hit and worth avoiding next time: `psp::dprintln!`
writes straight to VRAM, invisible to the GE-based headless screenshot hook
(RE-013/RE-255) — use real GE draws instead; and `Gpu::debug_text`/
`sceGuDebugPrint` corrupts rendering specifically under a
`regression_capture`-family frozen build (RE-123/RE-125) — untextured
`TRANSFORM_2D` GE quads work under that same build where debug text does not.
After Link, continue with the remaining five fighters. Yoshi, Captain
Falcon, Ness, Pikachu, Kirby remain fully untraced:
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
