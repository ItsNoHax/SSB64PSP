# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current pack, separate from RE-272's
now-closed Mario-face artifact. Four of nine items are fully closed (Fox,
Mario/Luigi, Samus, Link's boot). Link's boot defect (nodes 25/30, both feet
identically) was closed last session: a genuine PSP GE rendering defect for
that texture's packed shape (confirmed on both PPSSPP and physical hardware),
fixed by bypassing PSP's paletted-texture transport for that one texture
(`Psm8888` instead of `PsmT4`), the same class of fix `tools/romtool`'s
`convert_texture` already had for Donkey Kong (RE-267). See
`docs/evidence/re/RE-283.md`'s fourteenth follow-up for the full chain.

**New handover task (user-directed, not yet started): a second, separate
dark/purple speckle pattern on Link, on his shin cuff/greave, above the now-
fixed boot.** Noticed while confirming the boot fix's blast radius (the
full-`regression_capture_link`-scene diff against the prior golden was
tightly bounded to the boot region, `y >= 414` in the 960x544 capture; the
speckle sits just above that, untouched by the boot fix, confirmed pixel-
identical before and after it). **Not yet node-isolated, not yet root-caused
— this is a fresh, unexplored item, not a continuation of the closed boot
trace.** Coordinates below are this session's own reproduction, given for a
head start; re-derive them rather than trusting them blindly, since no
capture was committed (repo policy: no captures in Git) and pixel positions
can shift if unrelated code changes.

**Reproduction.** `tools/run-ppsspp-headless.sh --feature
"regression_capture,regression_capture_link"` (file 324, graph `0x3AE8`,
Link's full posed high-detail Wait render, matches `tests/golden/
r2-link-fighter.png`). In the resulting 960x544 capture, crop roughly
`x:460-570, y:340-420` — Link's rear leg (the one nearer the shield/sword
side), just above the boot collar. The speckle is dark purple/black,
irregular, mixed with brown — visually similar in *character* (same purple
family) to the boot defect's now-fixed patch, which is worth checking first,
but do not assume it is the same texture, node, or mechanism without
verifying. **Asymmetric, unlike the boot defect:** the front leg (crop
`x:400-470, y:340-420`, facing the camera) shows a clean, unspeckled brown
greave at the same height — so this is not "both legs identically" the way
nodes 25/30 were; it is isolated to one leg/node, which narrows the search
faster than the boot trace did.

**Suggested next steps, not prescriptive:**
1. Node-isolate it. Reuse the same scratch prior RE-283 follow-ups used:
   `psp/src/meshdraw.rs`'s `draw_object_posed_filtered` made `pub(crate)`
   temporarily; `psp/src/main.rs`'s object-view fighter-draw call site
   (~line 1952, `meshdraw::draw_object_posed(...)`) switched to call
   `draw_object_posed_filtered` directly with a compile-time
   `option_env!("RE283_ONLY_LOCAL_NODE")`-read filter, `obj.first_node + n`
   (node ids from `romtool scene --nodes` are object-relative; the filter
   parameter is pack-global — this unit mismatch cost real time in the boot
   trace, see RE-283's twelfth follow-up). Sweep node ids near 25/30 first
   (spatially adjacent to the boots), but confirm against the actual *posed*
   render, not bind-pose node coordinates (`romtool scene --nodes` alone is
   known to mislead here — see RE-283's seventh/thirteenth follow-ups).
2. Once isolated, run the same checklist RE-283 already uses: does the node
   carry its own ROM-authored `PRIM*SHADE`/`SetPrimColor`, a genuine
   tile-addressing/UV quirk, or a downstream packing/bind bug? Check whether
   it is the *same* class of defect as the boot (small paletted-texture GE
   quirk — if so, `tools/romtool/src/main.rs`'s `convert_texture` already has
   the fix pattern: `is_link_boot`/`src.home.id == 317` precedent, add a new
   scoped match arm keyed to this texture's own `data_file`/`data_offset`,
   confirmed unique via a temporary debug `eprintln!` the same way).
3. **Do not assume it is a bug before checking against real N64 output.**
   Samus's olive-camo cannon (RE-283, eighth follow-up) and Fox's wrist band
   (RE-283, fifth follow-up) both turned out to be real, ROM-authored
   original-hardware appearance, not porting bugs — dark/irregular fabric or
   armor patterns are a real, recurring class of "looks like corruption but
   isn't" on this ROM. If a colour/geometry source is found and looks
   ROM-authored, confirm with a live original-N64 capture via the
   `refs/ssb-decomp-re` decomp-rebuild warp technique (`n64-emulator` Skill,
   `nFTKindXxx` in `scmanager.c`'s `training_man_fkind`/`training_com_fkind`,
   same relative patch sites RE-283's fifth/eighth follow-ups used) before
   concluding either way.
4. Known PSP-target-tracing traps, already paid for by prior follow-ups:
   `psp::dprintln!` writes straight to VRAM, invisible to the GE-based
   headless screenshot hook (RE-013/RE-255) — use real GE draws instead; and
   `Gpu::debug_text`/`sceGuDebugPrint` corrupts rendering specifically under
   a `regression_capture`-family frozen build (RE-123/RE-125) — untextured
   `TRANSFORM_2D` GE quads work under that same build where debug text does
   not.
5. Whatever the outcome, revert all scratch instrumentation (`git checkout
   --` on `psp/src/main.rs`/`psp/src/meshdraw.rs`) before finishing, same as
   every prior RE-283 follow-up; do not commit captures.

This is not yet in RE-283's checklist as a numbered item (it was found
incidentally, after Fox/Mario-Luigi/Samus/Link's boot were already
enumerated) — open a new follow-up section in `docs/evidence/re/RE-283.md`
("fifteenth follow-up" or its own record if it turns out unrelated to the
rest of RE-283) once there is a finding to write up, per this project's
"measure, then record" convention. Yoshi, Captain Falcon, Ness, Pikachu,
Kirby remain fully untraced and still block the physical R2 matrix
independently of this new item — do not treat either as higher-priority
than the other without cause; the user asked for the shin speckle next, so
start there.

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed — neither the Slim nor
the PSP-3000 tested so far is in that RAM class. No dedicated capture scenes
exist yet for full stage/effect coverage or runs longer than 10 minutes.
None of these block R2.2 (closed) — they gate the *physical* R2 matrix only,
and are deliberately deferred until the game structure grows beyond the
asset viewer. RE-272's face-texture bug is fully closed on both PPSSPP and
real hardware — no longer part of the deferred physical-matrix follow-up.

Current verification baseline (unchanged since last session, no code
touched this handover): `cargo fmt --all --check` clean; `cargo test
--workspace` 617 passed; 22/22 deterministic goldens exact against the
current pack; effects 46/46 manager objects, 35/35 transform + 24/26
material animations, 160/160 particle scripts; 109 billboards, 0 anomalies;
11/11 framebuffer transitions; `romtool texgen --verify` pass; plain
feature-free `cargo psp --release` pass; strict Clippy not clean (3
pre-existing `needless_range_loop` lints, unrelated to this work).

Required next action: node-isolate and trace Link's shin-cuff speckle (see
above) — this is the active task. After it concludes (fixed, or confirmed
ROM-authored and not a bug), continue RE-283 with the remaining five
untraced fighters (Yoshi, Captain Falcon, Ness, Pikachu, Kirby), each
checked against the same checklist, each on its own trace — do not assume
one fighter's cause generalizes to another without checking. Physically
confirming the PSP-1000 class remains the sole *physical*-matrix blocker
(unchanged — no PSP-1000 unit available in this environment) but is
secondary to RE-283 until it concludes. Do not start R3 or combat before
both RE-283 and R2's physical matrix are closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-267, RE-269, RE-270, RE-271,
RE-272, RE-273, RE-274, RE-275, RE-276, RE-277, RE-278, RE-279, RE-280,
RE-281, RE-282, RE-283 (see [docs/evidence/INDEX.md](docs/evidence/INDEX.md)
for the full R2.2/physical chain, RE-240–283). Toolchain note: the global
`cargo-psp` install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-283-fourteenth-follow-up pack/code (previous session; no
code or pack change this handover).
Pack `f477c52c096ce5c78702fdc07b3f62df7ef279e06dca6feea2d9ceae2ad32b84`
(28,532.3 KiB). Plain feature-free EBOOT
`9b9bfb8f94730a47ea04e50cb2a75a8d91536bd9b13f74682256a3ebe902ca8c`
(unchanged from RE-281 — the boot fix touched only pack-time conversion).
