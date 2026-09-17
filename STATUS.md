# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current RE-281 pack, separate from
RE-272's now-closed Mario-face artifact. Three of nine items are now fully
closed (Fox, Mario/Luigi, Samus); Link is node-isolated with its combiner
decoded but root cause still open; five (Yoshi, Captain Falcon, Ness,
Pikachu, Kirby) remain fully untraced. This blocks resuming the physical R2
matrix (which was otherwise down to only the PSP-1000-availability blocker
below) — do not treat R2's rendering gate as closed until RE-283 concludes
for the remaining fighters.

Last completed: `RE-283` (this session's ninth follow-up) — **Link's boot
defect ("buggy shoes") node-isolated to node 25/30 (file 324, graph
`0x3AE8`), identical on both feet — not closed.** Bind-pose `romtool scene
--nodes` output initially misdirected this session onto Link's arms instead
of his legs (the same bind-pose-vs-posed trap `STATUS.md` already warns
about); caught by cross-checking against the actual posed render before
trusting it. Reused this project's existing, committed single-node isolate
primitive (`psp/src/meshdraw.rs`'s `draw_object_node`/
`draw_object_posed_filtered`, built for R0.12's billboard audit) by
temporarily exposing it `pub(crate)` and adding a compile-time
(`option_env!`) local-node filter to the real fighter-view draw call;
isolating node 25 and node 30 separately via
`tools/run-ppsspp-headless.sh --feature regression_capture_link` reproduced
the identical purple/black ankle-collar patch on both feet, at the same
relative position — the "one clean, one broken" read from the full-body
crop was a camera-angle/occlusion illusion, not a real left/right asymmetry.
All isolate-tool scratch code reverted after use (`git checkout --` on
`psp/src/main.rs`/`psp/src/meshdraw.rs`); no code survives from this step.
Ruled out with direct evidence: the raw bound texture (`romtool texdump`,
offset `0xCF18`, 16x16 CI4) decodes to 11 pure brown/tan palette colours,
zero blue/purple; both boots' own `G_MW_LIGHTCOL` values are neutral grey
(`(76,76,76)`/`(255,255,255)`); both boots' raw `Vtx` records have alpha
byte `0` (lit-mode packed normals, not literal vertex colour, matching
`ftDisplayMainProcDisplay`'s unconditional fighter `G_LIGHTING`); and the
two boots' full command streams are byte-for-byte identical, ruling out an
asymmetry mechanism. Manually decoded the shared `SetCombine` word
(`hi=1211909, lo=4279759871`) into its exact RDP formula: a real two-cycle
`(TEXEL0*SHADE)*ENVIRONMENT`, distinct from Fox's/Samus's single-cycle
shapes. Walked all 32 of the graph's nodes' raw commands (zero
`Cmd::SetEnvColor` anywhere) and every node with a real, resolved static
`MObj` table in this graph (nodes 1/2/4/9/19/20, read via
`mobj::read_material` on the exact offsets `romtool mobj --file 324`
reports — all `env_color: None`). Hand-replaying `mesh.rs`'s own symbolic
combiner algebra (`cycle`/`Combined::mul`) for this exact shape with `env`
genuinely unset predicts a clean `TEXEL0*SHADE` scale of `[1.0,1.0,1.0]` —
i.e. mesh.rs's own logic, read on paper, contradicts the observed
purple/black render. Left open with two concrete next steps neither checked
this session: (1) a live per-frame material-animation script driving
`env_color` dynamically (node 19's `MObj` chain carries a `sprite`
reference, suggesting it is animated, and RE-089 already documents that
some `MObjSub` fields are only meaningful together with their driving
script) — the static snapshot this session read cannot see that; (2) a real
implementation bug in `mesh.rs`'s actual imperative `State::env_color`
mutation during its node walk, which needs direct instrumentation
(`eprintln!` inside the real conversion path) rather than by-hand algebra
replay to find. No production code or asset-pack change this session — only
`docs/evidence/re/RE-283.md`, `docs/evidence/INDEX.md`,
`plans/rendering/R2.md` and this file were updated.

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
with the remaining five fighters. For Link: instrument `mesh.rs`'s actual
`convert`/`State` node walk directly (temporary `eprintln!` of
`state.env_color` right before this primitive's material is finalized, for
file 324's graph `0x3AE8`, node 25 or 30), rather than re-deriving its
combiner algebra by hand as this session did — the by-hand replay predicts a
clean `TEXEL0*SHADE` and contradicts the observed purple/black render, so
something concrete in the live conversion state (or a material-animation
script driving `env_color` at runtime, starting from node 19's animated
`MObj` chain) differs from what the static analysis sees. Fox, Mario/Luigi,
and Samus are closed. Yoshi, Captain Falcon, Ness, Pikachu, Kirby remain
fully untraced: check each against the same "does the node carry its own
authored PRIM*SHADE colour, or a ROM-authored tile-addressing/UV quirk"
pattern first, but do not assume either conclusion generalizes — Link's
combiner turned out to be a third, two-cycle shape neither pattern
anticipated, so treat that checklist as a starting point, not exhaustive.
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
