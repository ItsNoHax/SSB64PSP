# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current RE-281 pack, separate from
RE-272's now-closed Mario-face artifact. Three of nine items are now fully
closed (Fox, Mario/Luigi, Samus); six (Link, Yoshi, Captain Falcon, Ness,
Pikachu, Kirby) remain untraced. This blocks resuming the physical R2 matrix
(which was otherwise down to only the PSP-1000-availability blocker below) —
do not treat R2's rendering gate as closed until RE-283 concludes for the
remaining six fighters.

Last completed: `RE-283` (this session's eighth follow-up) — **Samus's
chest "black square" closed via live original-N64 capture, at the same bar
Fox's item met.** The previous follow-up traced the defect to node 12 (the
`f320` offset `0xC408` olive-camo/joint-cap texture) rendered in Samus's
real Wait pose, but only as a structural argument (blue-channel-zero,
R/G-ratio agreement between raw texture and rendered defect) — not an
exact combiner reconstruction or a hardware check. This session closed it
with the second option RE-283 left open: a live capture. Used the same
`refs/ssb-decomp-re` decomp-rebuild warp technique Fox's fifth follow-up
used (`n64-emulator` Skill), retargeted to Samus (`training_man_fkind =
nFTKindSamus`, `training_com_fkind = nFTKindMario` in `scmanager.c`'s
`dSCManagerDefaultSceneData` reset site, plus the same Close-Up-camera call
in `sc1ptrainingmode.c`'s `sc1PTrainingModeFuncStart`, right after the
fighter-construction loop). Verified toolchain clean before patching
(`build/smashbrothers.us.z64` byte-identical to `baserom.us.z64`); the
RE-276 toolchain shim (`PATH=~/ppsspp-test/mips-bin-shim:$PATH`) needed
re-exporting this session, since it is a per-session `PATH` addition, not a
persisted shell setting. `tools/run-n64-headless.sh --frames 300
--final-screenshot` against the patched ROM landed in the real Training
Mode scene on the first attempt (HUD-confirmed: `DAMAGE 000%`/`COMBO
00`/`ENEMY STAND`/`SPEED 1/1`), showing Samus in her default idle stance,
cannon arm crossed low across the body, cannon barrel in clear view — not
hidden, not black. Pixel-sampled across the cannon barrel (266 zero-blue
samples): `R` 11-210, `G` 0-137, `B` exactly 0 throughout, `R`/`G` staying
within a few percent of each other end-to-end — the same zero-blue,
`R≈G` signature the raw texture and the golden's own darkened defect
region already showed, now confirmed on real hardware rather than inferred
from texture statistics alone. The golden's own camera/light angle happens
to catch this same real surface mostly in the near-black end of its real
shading range, producing the "black square" symptom, but it is the same
authored ROM geometry/texture/shading Fox's item's mechanism already
predicted — not a porting defect. `refs/ssb-decomp-re` reverted (`git
checkout -- src/sc/scmanager.c src/sc/sc1pmode/sc1ptrainingmode.c`) and
rebuilt clean (`build/smashbrothers.us.z64` re-confirmed byte-identical to
`baserom.us.z64` afterward). No production code changed this session —
only `docs/evidence/re/RE-283.md`, `docs/evidence/INDEX.md`,
`plans/rendering/R2.md` and this file were updated; `assets/generated/
ssb64.pak` and `crates/ssb-rom`/`crates/psp` are unchanged.

Before that, `RE-283` (seventh follow-up) — Samus's chest "black square"
positively traced to node 12 under her real posed (not bind-pose) skeleton,
matching the golden's own defect bbox almost exactly, but only at medium
confidence pending the live-hardware check the eighth follow-up above then
completed. See `docs/evidence/re/RE-283.md` for the full seventh-through-
sixth follow-up chain (Mario/Luigi's button item, the posed-vs-bind-pose
isolate-tool pitfall, and Fox's own live-N64 closure).

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

Required next action: continue root-causing RE-283's fighter texture/
geometry defects for the remaining six fighters. Fox, Mario/Luigi, and
Samus are closed. Link, Yoshi, Captain Falcon, Ness, Pikachu, Kirby remain
fully untraced: check each against the same "does the node carry its own
authored PRIM*SHADE colour, or a ROM-authored tile-addressing/UV quirk"
pattern first, but do not assume either conclusion generalizes; each needs
its own trace and, where a colour/geometry source is found, potentially its
own original-hardware visibility check via the now twice-proven
`refs/ssb-decomp-re` decomp-rebuild warp technique (`n64-emulator` Skill,
`nFTKindXxx` in `scmanager.c`'s `training_man_fkind`/`training_com_fkind`
fields, same relative patch sites both prior follow-ups used). When
isolating a single node for any of these, use the *posed* skeleton
(`draw_object_posed_filtered` with the real `posed[..posed_len]` array and
an `only_node` filter), not `draw_object_node`'s bind-pose-only path — this
session's seventh follow-up found the bind-pose shortcut silently misleads
for any fighter whose Wait pose differs substantially from its bind pose,
which cannot be assumed away per-fighter. Physically confirming the
PSP-1000 class remains the sole *physical*-matrix blocker (unchanged — no
PSP-1000 unit available in this environment) but is secondary to RE-283 now
that the renderer's own PPSSPP-software correctness is back in question for
the remaining fighters. Do not start R3 or combat before both RE-283 and
R2's physical matrix are closed.

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
