# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current RE-281 pack, separate from
RE-272's now-closed Mario-face artifact. Four of nine items are now fully
closed (Fox, Mario/Luigi, Samus, Link). Link's boot defect (nodes 25/30, both
feet identically) is fully closed this session: after the whole
`ssb-rom`/`tools/romtool` pipeline and the PSP runtime GE bind were already
proven clean by live instrumentation, and a physical-hardware capture
confirmed the purple/black patch is a genuine PSP GE rendering defect for
this texture's packed shape (not a PPSSPP-only quirk), the user directed
pursuing a fix rather than an `ACCEPTED_DEVIATION`. A stride/swizzle-floor
hypothesis was tested and rejected (widening the row to 16 bytes made the
texture swizzle but left the defect byte-identical). The fix that worked:
bypass PSP's paletted-texture transport for this one texture (`Psm8888`
instead of `PsmT4`, decoding the ROM texels through their real TLUT at pack
time) — the same class of fix `tools/romtool`'s `convert_texture` already
has for Donkey Kong's file 317 (RE-267). Confirmed removed on PPSSPP
(node-isolated and full-pose capture, diff tightly bounded to the boot
region, 21/22 other goldens byte-identical) and on physical PSP hardware
(same PSP Slim, PSPLink v3.2.1). `tests/golden/r2-link-fighter.png`
refreshed. Five (Yoshi, Captain Falcon, Ness, Pikachu, Kirby) remain fully
untraced. This blocks resuming the physical R2 matrix (which was otherwise
down to only the PSP-1000-availability blocker below) — do not treat R2's
rendering gate as closed until RE-283 concludes for the remaining fighters.
Note: the full-pose capture also surfaced a separate, unrelated dark/purple
speckle pattern on Link's shin cuff/greave, untouched by this fix and not
investigated — not conflated with the closed boot item; may need its own
trace later if it turns out not to be authored art.

Last completed: `RE-283` (this session's fourteenth follow-up) — **fixed
Link's boot defect by bypassing PSP's paletted-texture transport for that
one texture, closing the item at the user's explicit direction (fix, not
`ACCEPTED_DEVIATION`).** `crates/ssb-rom/src/psp_texture.rs`: tested and
rejected a stride/swizzle-floor hypothesis (floored `pack_indexed`'s row to
16 bytes; confirmed via a temporary debug print that this texture then packs
at `stride=32 swizzled=true`, but a rebuilt-pack PPSSPP node-isolated capture
showed the purple/black patch byte-identical — reverted the change and its
test). `tools/romtool/src/main.rs`'s `convert_texture`: added `is_link_boot`
(`src.home.id == 324 && t.data_file.is_none() && t.data_offset == 0xCF18`,
confirmed the unique match via a temporary debug print) alongside the
existing Donkey Kong (`src.home.id == 317`) case, forcing `Psm8888` instead
of `PsmT4` for this texture only. Rebuilt `assets/generated/ssb64.pak`
(+1.0 KiB). Verified with the same node-isolate scratch prior follow-ups
used (`draw_object_posed_filtered` `pub(crate)`, `RE283_ONLY_LOCAL_NODE`
compile-time filter): PPSSPP node-isolated capture shows the patch gone;
full `regression_capture_link` capture diffs only the boot region (2,836
pixels) against the prior golden, with the other 21/22 deterministic
goldens byte-identical; two captures of the fixed build are byte-identical
(deterministic). Physical PSP (Slim, firmware 6.61, ARK/Infinity, PSPLink
v3.2.1): `exlist` empty, `main_thread` alive, native `scrshot` capture shows
the same clean boot, patch gone. `cargo fmt --all --check` and `cargo test
--workspace` (617 passed) clean; plain feature-free `cargo psp --release`
EBOOT hash unchanged (fix is pack-time only, no PSP runtime code changed).
All scratch instrumentation reverted; captures not committed. See
`docs/evidence/re/RE-283.md`'s "fourteenth follow-up" section; prior
follow-ups' own findings stand unchanged.

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed — neither the Slim nor
the PSP-3000 tested so far is in that RAM class. No dedicated capture scenes
exist yet for full stage/effect coverage or runs longer than 10 minutes.
None of these block R2.2 (closed) — they gate the *physical* R2 matrix only,
and are deliberately deferred until the game structure grows beyond the
asset viewer. RE-272's face-texture bug is fully closed on both PPSSPP and
real hardware — no longer part of the deferred physical-matrix follow-up.

Current verification baseline: `cargo fmt --all --check` clean; `cargo test
--workspace` 617 passed; 21/22 deterministic goldens exact against the
rebuilt pack, `r2-link-fighter.png` refreshed (explained delta: Link's boot
texture, RE-283); effects 46/46 manager objects, 35/35 transform + 24/26
material animations, 160/160 particle scripts; 109 billboards, 0 anomalies;
11/11 framebuffer transitions; `romtool texgen --verify` pass; plain
feature-free `cargo psp --release` pass (EBOOT hash unchanged from RE-281);
strict Clippy not clean (3 pre-existing `needless_range_loop` lints,
unrelated to this work). `assets/generated/ssb64.pak` rebuilt this session
(pack hash below); `crates/ssb-rom` unchanged (its stride-floor experiment
was reverted); `tools/romtool` gained the scoped `is_link_boot` fix.

Required next action: continue RE-283 with the remaining five fighters.
Yoshi, Captain Falcon, Ness, Pikachu, Kirby remain fully untraced: check
each against the same "does the node carry its own authored PRIM*SHADE
colour, a ROM-authored tile-addressing/UV quirk, or a bug downstream in
texture packing/binding or the runtime GE bind" pattern first, but do not
assume any one conclusion generalizes — treat that checklist as a starting
point, not exhaustive. Given Link's own resolution this session, also check
early whether a given fighter's defect is the same "GE mis-renders this
exact small paletted-texture shape" class before assuming a novel cause —
but confirm with the same live-instrumentation/isolation rigor rather than
assuming it generalizes without checking. Each needs its own trace and,
where a colour/geometry source is found, potentially its own
original-hardware visibility check via the now twice-proven
`refs/ssb-decomp-re` decomp-rebuild warp technique (`n64-emulator` Skill,
`nFTKindXxx` in `scmanager.c`'s `training_man_fkind`/`training_com_fkind`
fields, same relative patch sites prior follow-ups used). When isolating a
single node for any of these, use the *posed* skeleton
(`draw_object_posed_filtered` with the real `posed[..posed_len]` array and a
node filter), not `draw_object_node`'s bind-pose-only path — confirmed
twice now (Samus's seventh follow-up, Link's trace) that trusting bind-pose
node coordinates (e.g. from `romtool scene --nodes`) to guess which limb is
which silently misleads; always cross-check against the actual posed render
first. This project's `psp/src/meshdraw.rs` already has the real isolate
primitive (`draw_object_posed_filtered`'s `only_node` parameter, built for
R0.12's billboard audit) — reuse it (temporarily `pub(crate)`, plus a
compile-time `option_env!`-read local-node filter threaded into the real
fighter-view draw call, since the PSP target has no runtime env access)
rather than building a new one; revert the exposure after use. Two
PSP-target-tracing traps hit and worth avoiding: `psp::dprintln!` writes
straight to VRAM, invisible to the GE-based headless screenshot hook
(RE-013/RE-255) — use real GE draws instead; and `Gpu::debug_text`/
`sceGuDebugPrint` corrupts rendering specifically under a
`regression_capture`-family frozen build (RE-123/RE-125) — untextured
`TRANSFORM_2D` GE quads work under that same build where debug text does
not. If a fighter's defect turns out to be the same small-paletted-texture
GE quirk Link's was, `tools/romtool/src/main.rs`'s `convert_texture` already
has the fix pattern (`is_link_boot`/`src.home.id == 317` precedent) — scope
a new match arm to that fighter's exact `data_file`/`data_offset` rather
than widening either existing condition. Physically confirming the
PSP-1000 class remains the sole *physical*-matrix blocker (unchanged — no
PSP-1000 unit available in this environment) but is secondary to RE-283
until it concludes for the remaining fighters. Do not start R3 or combat
before both RE-283 and R2's physical matrix are closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-267, RE-269, RE-270, RE-271,
RE-272, RE-273, RE-274, RE-275, RE-276, RE-277, RE-278, RE-279, RE-280,
RE-281, RE-282, RE-283 (see [docs/evidence/INDEX.md](docs/evidence/INDEX.md)
for the full R2.2/physical chain, RE-240–283). Toolchain note: the global
`cargo-psp` install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-283-fourteenth-follow-up pack/code (this session).
Pack `f477c52c096ce5c78702fdc07b3f62df7ef279e06dca6feea2d9ceae2ad32b84`
(28,532.3 KiB). Plain feature-free EBOOT
`9b9bfb8f94730a47ea04e50cb2a75a8d91536bd9b13f74682256a3ebe902ca8c`
(unchanged from RE-281 — this fix touched only pack-time conversion).
