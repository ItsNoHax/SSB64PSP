# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current RE-281 pack, separate from
RE-272's now-closed Mario-face artifact. Fox's wrist-node defect colour is
now fully explained mathematically (see "Last completed" below), but
whether that explanation matches real N64 hardware is not yet settled, and
the other eight fighters' symptoms remain untraced. This blocks resuming
the physical R2 matrix (which was otherwise down to only the
PSP-1000-availability blocker below) — do not treat R2's rendering gate as
closed until RE-283 concludes.

Last completed: `RE-283` (this session's third follow-up) — **ruled out
the `lit`/unlit-misclassification hypothesis and found the exact
arithmetic source of Fox's wrist-node `#321d01` defect colour**, using a
temporary `romtool vtxdump` diagnostic (added and reverted this session)
with two modes: a raw static `G_VTX` byte dump, and a real-pipeline trace
running `plan_draw_order`/`convert_sequence` (the same code `pack()`
uses). Node 11's (wrist) five raw vertex normals are legitimate unit
vectors (magnitude ~127), not miscoded colour bytes — ruling out
byte-level corruption outright. The real pipeline resolves node 11's
triangle material to `prim_color = [168, 98, 4]` (`#A86204`, Fox's "arm
fur" colour, identical to the value node 5 — the mirrored right-arm
equivalent — sets explicitly), via the same `PRIM * SHADE` "arms" combiner
shape this codebase's own Mario test fixture already names. Both scene
lights are achromatic (`#FFFFFF` directional, `#4C4C4C` ambient), so
`76/255 × [168, 98, 4]` truncates to exactly `[50, 29, 1] = #321d01` —
an exact, not approximate, reconstruction of the golden's defect colour.
This is real, correctly-combinered math, not a decode bug — but *why*
that specific `PRIM` value is still active by the time node 11's own
triangles draw is not fully traced: `plan_draw_order`'s `Gfx *dls[2]`
pre/post-pair mechanism (RE-225/T1) is ruled out (every node in this
graph resolves as a single `Direct` entry), and a plain
"last-state-of-the-previous-node's-list" model already fails to predict
node 10's own first primitive's `prim_color`, so the real mechanism runs
through per-triangle-command state and cross-node vertex-cache reuse this
session did not fully unravel. Two possibilities remain: (a) a real
state-threading defect in `plan_draw_order`/`convert_sequence`, or (b) the
ROM's own DL and vertex-cache reuse produce this same result on real
hardware too, for a sliver the original keeps hidden behind overlapping
cuff/glove geometry that this port's per-node transform exposes. See
`docs/evidence/re/RE-283.md`.

Before that, `RE-282` — **physically re-confirmed RE-281's `Ci4`/`PsmT4`
nibble-order fix on real PSP hardware.** Built
`cargo psp --release --features regression_capture_mario` (PRX SHA-256
`dacdd5907d743fea5b478b73eb67018b2f3f01a4f0182d33bb257b45dc04219a`) against
the unchanged RE-281 pack
(`0065c65d92cf805b12f6157259e0e0da9a6ab0f872c01d3aae271c49e277a368`). Loaded
on a PSP Slim (firmware 6.61, ARK/Infinity, PSPLink v3.2.1) via `host0:`
(fixed a stale USB-permission issue first: the `50-psplink.rules` udev rule
was present but hadn't applied to the already-enumerated device node; a
cable replug re-triggered udev and fixed it, no rule change needed). `reset`
issued before load per this project's own PSPLink convention. Zero
exceptions (`exlist` empty, `main_thread` alive in `thlist`) after the
240-tick deterministic-capture freeze. Native `scrshot` capture
(SHA-256 `5e72668e314e3a3f25ec8711731e0a8c58312b7d3b9ea575b1216413fcd417e0`,
not committed) shows Mario's cap "M" emblem rendering clean and
correctly-rounded, with no repeating adjacent-texel-pair-swap artifact —
RE-272's original named complaint. Quantitative diff against
`tests/golden/r2-mario-fighter.png` (box-downsampled to the hardware
capture's resolution, PSPLink overlay region excluded): 349/130,560 pixels
(0.27%) exceed a 30-level threshold, consistent with this project's existing
hardware-vs-PPSSPP antialiasing noise floor (RE-203/RE-207), not a new
defect. This closes the one remaining item RE-281's own Confidence note
named; the RE-272→RE-281 investigation chain is now fully closed on both
PPSSPP and real hardware. Shut down cleanly (`kill`, `reset`,
`usbhostfs_pc` stopped). No production code changed this session; only
`docs/evidence/re/RE-282.md` added, `docs/evidence/INDEX.md` regenerated,
and `plans/rendering/R2.md`/this file updated.

Before that, `RE-281` — applied RE-280's identified `Ci4`/`PsmT4`
nibble-order fix and closed RE-272's Mario-face bug on PPSSPP software
rendering. Flipped all three coordinated sites in
`crates/ssb-rom/src/psp_texture.rs` together (`encode_level`'s `PsmT4`
branch, `pack_indexed`'s straight ROM-copy path — now swaps nibbles instead
of copying unchanged — and `pad_edge_repeat_nibbles`'s `get`/`set`
convention), and updated `ci4_packing_round_trips_through_swizzle_and_clut`'s
`unpack_ci4` test helper plus four literal-byte-value test assertions to the
corrected convention. `cargo test -p ssb-rom psp_texture`: 44/44 pass;
`cargo test --workspace`: 617/617 pass; `cargo fmt --all --check`: clean.
Rebuilt `assets/generated/ssb64.pak` from the verified local ROM (same
28,531.3 KiB size, new hash `0065c65d...e277a368`, was `a79b0aa9...5935f`).
Re-ran the full 22-scene deterministic regression matrix against the rebuilt
pack: 18 goldens changed, 4 did not (`r1-catch-swirl-flat-color.png`,
`r2-depth-mask-diagnostic.png`, `r2-dk-fighter.png`, `r2-kirby-fighter.png`
— consistent with RE-280's own prediction that the bug is least visible on
smoothly-shaded content or bytes whose two nibbles happen to be equal).
Refreshed all 18 changed goldens; every changed pixel traces to the same
single cause (corrected `Ci4` texel decode), not a rendering regression.

Before that, `RE-280` found RE-272/274's Mario-face bug's actual root
cause: `ssb_rom::psp_texture`'s `Ci4` (`PsmT4`) packing packed two 4-bit
texels per byte high-nibble-first, "matching the N64 order" — but PPSSPP's
`PsmT4` reader (the deterministic golden source this project already trusts
for GE addressing) wants the opposite order, low nibble first. Before that,
`RE-279` confirmed RE-274's real primitive is correctly routed through
`meshdraw`'s signed-UV float path. Before that, `RE-278` built a permanent
measurement rig ruling out "the GE mis-samples a known fixed coordinate near
[the clamp] boundary" as RE-272's cause. Before that, `RE-277` ran a dense
per-texel sweep ruling out the addressing *formula* itself. Before that,
`RE-276` — a live N64 reference render of Mario's face renders clean, no
repeating pattern — evidence for "real PSP-side addressing/scale bug" over
"faithfully-reproduced ROM quirk". Before that, `RE-273` physically
confirmed the renderer on a second hardware unit (PSP-3000): scene1,
depth-mask diagnostic (RE-271's fix holds), Mario fighter capture,
10-minute sustained run, zero exceptions throughout — and that Mario
capture surfaced `RE-272`, now closed as described above.

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed — neither the Slim nor
the PSP-3000 tested so far is in that RAM class. No dedicated capture scenes
exist yet for full stage/effect coverage or runs longer than 10 minutes.
None of these block R2.2 (closed) — they gate the *physical* R2 matrix only,
and are deliberately deferred until the game structure grows beyond the
asset viewer. RE-272's face-texture bug (root-caused by RE-280, fixed by
RE-281, physically re-confirmed by RE-282) is now fully closed on both
PPSSPP and real hardware — no longer part of the deferred physical-matrix
follow-up.

Current verification baseline: `cargo fmt --all --check` clean; `cargo test
--workspace` 617 passed; all 22 deterministic goldens exact against the
rebuilt pack (18 refreshed by RE-281, each with its semantic delta explained
— see RE-281 and `docs/visual-regression/README.md`); effects 46/46 manager
objects, 35/35 transform + 24/26 material animations, 160/160 particle
scripts; 109 billboards, 0 anomalies; 11/11 framebuffer transitions; `romtool
texgen --verify` pass; plain feature-free `cargo psp --release` pass; strict
Clippy not clean (3 pre-existing `needless_range_loop` lints in
`coord.rs`/`matanim.rs`/`objanim.rs`, unrelated to this work). This session
(RE-282) made no production-code change; it built
`--features regression_capture_mario` and `--release` PRXes for hardware
loading only. `assets/generated/ssb64.pak` is unchanged from RE-281's
rebuild. RE-283 also lands with no surviving production-code change: its one
tested hypothesis (widen the CI4 swizzle-eligibility floor) was patched,
measured (0-pixel-diff PPSSPPHeadless capture against the existing golden),
rejected, and reverted — `crates/ssb-rom/src/psp_texture.rs` and
`assets/generated/ssb64.pak` are both back to their RE-281 state. This
session's own follow-up (the `vtxdump` diagnostic and node 11 material
trace) also made no surviving production-code change: `tools/romtool/src/
main.rs`'s temporary subcommand was reverted (`git checkout --`) after use;
only `docs/evidence/re/RE-283.md` and this file were updated.

Required next action: continue root-causing RE-283's fighter texture/
geometry defects. Fox's wrist node (local node 11 of graph `0x2938`, `dl
0x22B8`) — the `#321d01` colour itself is now fully explained (exact
`PRIM * SHADE` arithmetic, `PRIM = [168,98,4]`, real "arm fur" colour) but
*why* that `PRIM` value is still active at node 11's own triangle draw is
not traced. Next step, in order of cost: (1) extend a diagnostic to trace
state command-by-command (not list-by-list) across nodes 8/10/11 —
specifically which `G_TRI` commands in node 10's two primitives (12
triangles each) consume which vertex-cache slot, loaded under which
`SetCombine`/`SetPrimColor`, to determine whether `plan_draw_order`/
`convert_sequence`'s state model is even internally consistent with the
raw command stream; if it's provably wrong there, that is a real,
fixable bug independent of any ROM capture. (2) If that traces clean, get
a live original-N64 reference of Fox's wrist via the `n64-emulator` Skill
(RE-276's own method) to check whether real hardware draws this geometry
visibly at all — this is the only way to confirm or rule out hypothesis
(b) (an authored, normally-hidden shadow/fur sliver). Once Fox's specific
defect is resolved, the other eight fighters' symptoms (Mario/Luigi
missing buttons, Samus/Link/Yoshi/Falcon/Ness speckle, Pikachu/Kirby
face-colour mismatch) remain open and untraced — check each against this
same `PRIM`-carry mechanism first, since it may be a unifying cause.
Physically confirming the PSP-1000 class remains the sole *physical*-matrix
blocker (unchanged — no PSP-1000 unit available in this environment) but
is secondary to RE-283 now that the renderer's own PPSSPP-software
correctness is back in question. Do not start R3 or combat before both
RE-283 and R2's physical matrix are closed.

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
