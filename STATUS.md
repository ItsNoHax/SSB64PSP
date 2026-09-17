# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current RE-281 pack, separate from
RE-272's now-closed Mario-face artifact. Two of nine items are now closed:
Fox's wrist-node defect colour is real, ROM-authored, correctly converted,
and confirmed visible on real N64 hardware (live capture) — not a porting
bug, no code change needed. Mario/Luigi's missing-button symptom is closed
on source-fidelity grounds — the button exists in the raw ROM texture, but
the ROM's own `G_SETTILESIZE` clamp window and vertex UVs clip nearly all
of it away, matching this project's already-proven archive-wide addressing
model; no live-hardware capture taken for this one (medium-high, not Fox's
high, confidence). Samus's chest/thigh speckle is narrowed to two candidate
ROM-authored CI4 textures (both decode cleanly from ROM, so not a decode
bug) but not yet pinned to the exact defect pixels — still open. Six
fighters (Samus, Link, Yoshi, Captain Falcon, Ness, Pikachu, Kirby) remain
untraced. This blocks resuming the physical R2 matrix (which was otherwise
down to only the PSP-1000-availability blocker below) — do not treat R2's
rendering gate as closed until RE-283 concludes for the remaining 6+1
(Samus partial) fighters.

Last completed: `RE-283` (this session's sixth follow-up) — **Mario/Luigi's
"missing overalls buttons" item closed; Samus's "chest/thigh speckle" item
narrowed but not concluded.** Built two small, scoped `tools/romtool`
diagnostics (`dlraw`: raw display-list decode straight from file bytes,
bypassing `convert_sequence`/segment resolution entirely; `vtxraw`: raw
`Vtx` pos/uv/rgba dump), sanity-checked `dlraw` against Fox's
already-published node 11 result before trusting it on new content, then
reverted both (`git checkout --`) after use, matching this session's own
established convention for exploratory instrumentation. Mario: found the
button texture (`file 296 offset 0x65F0`, 32x24 CI4) genuinely contains a
yellow disc, bound by the torso node's `SetTile`/`SetTileSize` (`uls:128
ult:32 lrs:380 lrt:124` → texel window S:[32,95] T:[8,31], clamp on T),
which clips away all but a ~6x2-texel sliver of the button (source rows
0-7 of 0-9 are outside the clamped/loaded window) — a raw ROM-authored
fact, not a decode bug, and consistent with the same tile-addressing model
`R2.0`/P0b-P0d already proved archive-wide-zero-divergence for this exact
shape (clamped, mirrored, non-zero-origin tile). Closed without a
live-N64 capture (medium-high confidence, not Fox's high). Samus: her
common-parts file is 320 (not her `FighterFile` model file 217, same
"common parts file ≠ model file" shape as Fox's file 313); two raw CI4
textures (`file 320` offsets `0xC9D8` — grey/red/green-checkerboard/blue,
bound by the head/neck node 9 — and `0xC408` — olive-green camo, bound by
the left-arm/cannon-chain node 12) are plausible sources for the
chest-region "speckle" and both decode cleanly straight from ROM bytes
(ruling out a corrupted-texel bug specifically), but which node's
triangles actually paint the flagged screen pixels was not proven this
session (the geometric reasoning used is circumstantial, unlike Fox's
proven per-triangle trace) — left open with a concrete next step recorded
in `docs/evidence/re/RE-283.md` (either a `mesh::walk` instrumentation pass
like Fox's, or a marker-colour palette override + re-capture). No
production code changed this session — only `docs/evidence/re/RE-283.md`,
`docs/evidence/INDEX.md`, `plans/rendering/R2.md`, and this file were
updated; `tools/romtool/src/main.rs`'s temporary `dlraw`/`vtxraw`
subcommands were reverted after use, `assets/generated/ssb64.pak` and
`crates/ssb-rom` are unchanged.

Before that, `RE-283` (fifth follow-up) — **live original-N64 capture
confirms real hardware draws Fox's brown wrist band too, closing Fox's item
in the defect catalog with no code change.** Used RE-276's own decomp-rebuild
warp technique (`refs/ssb-decomp-re`, temporary edits to `scmanager.c`/
`sc1ptrainingmode.c`, reverted after use via `git checkout --`, rebuild
reconfirmed byte-identical to `baserom.us.z64` afterward), adapted to target
Fox instead of Mario as the Training Mode Close-Up camera's subject.
`tools/run-n64-headless.sh` (offscreen, `n64-emulator` Skill) captured Fox
in his default idle pose with the wrist in frame. The captured wrist shows a
smooth brown gradient band (`(153,89,4)` down to `(64,37,2)`, sampled
directly) between the white cuff and white glove, unobscured — the same
`[168,98,4]` PRIM colour the prior session derived analytically from the PSP
pipeline, confirming this is a faithful reproduction of authored ROM
geometry/combiner state, visible on original hardware exactly as designed.
See `docs/evidence/re/RE-283.md` for images and full pixel-level detail.

Before that, `RE-283` (fourth follow-up) — traced state command-by-command
across Fox's nodes 8/10/11 and found node 11 has its own real, ROM-authored
`G_SETCOMBINE`/`G_SETPRIMCOLOR([168,98,4])`, refuting the
state-threading-bug hypothesis. See `docs/evidence/re/RE-283.md`.

Before that, `RE-282` — **physically re-confirmed RE-281's `Ci4`/`PsmT4`
nibble-order fix on real PSP hardware.** Built
`cargo psp --release --features regression_capture_mario` (PRX SHA-256
`dacdd5907d743fea5b478b73eb67018b2f3f01a4f0182d33bb257b45dc04219a`) against
the unchanged RE-281 pack
(`0065c65d92cf805b12f6157259e0e0da9a6ab0f872c01d3aae271c49e277a368`). Zero
exceptions after the 240-tick deterministic-capture freeze; native `scrshot`
capture shows Mario's cap "M" emblem rendering clean, closing RE-272's
chain fully on both PPSSPP and real hardware.

Before that, `RE-281` applied RE-280's identified `Ci4`/`PsmT4` nibble-order
fix and closed RE-272's Mario-face bug on PPSSPP software rendering. Full
detail of RE-272 through RE-280's root-causing chain: see
`docs/evidence/re/RE-280.md` and `docs/evidence/re/RE-281.md`.

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
session (RE-283's sixth follow-up) made no surviving production-code
change — `tools/romtool/src/main.rs`'s temporary `dlraw`/`vtxraw`
subcommands were added, used, and reverted (`git checkout --`); `cargo
build -p romtool` reconfirmed clean afterward. `assets/generated/ssb64.pak`
and `crates/ssb-rom`/`crates/psp` are unchanged from RE-281's rebuild.

Required next action: continue root-causing RE-283's fighter texture/
geometry defects for the remaining fighters. Fox and Mario/Luigi are now
closed. Samus is narrowed (candidate nodes/textures identified, both
confirmed not corrupted) but needs either a `mesh::walk`-style per-triangle
instrumentation pass (Fox's own precedent) or a marker-colour palette
override + `regression_capture_samus` re-capture to confirm which node
paints the flagged pixels — see `docs/evidence/re/RE-283.md`'s Samus
section for the exact next command. Link, Yoshi, Captain Falcon, Ness,
Pikachu, Kirby remain fully untraced — check each against the same "does
the node carry its own authored PRIM*SHADE colour, or a ROM-authored
tile-addressing/UV quirk" pattern first (Fox's and Mario's own mechanisms),
but do not assume either conclusion generalizes; each needs its own trace
and, where a colour/geometry source is found, potentially its own
original-hardware visibility check the way Fox got. Physically confirming
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
