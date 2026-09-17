# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current pack, separate from RE-272's
now-closed Mario-face artifact. Six of nine items are fully closed, each
confirmed on both PPSSPP and physical PSP hardware: Fox, Mario/Luigi, Samus,
Link's boot, Link's shin cuff, and (this session) Yoshi.

**Yoshi's head noise: closed this session, on both PPSSPP and physical
hardware.** Node-isolated to node 3 of the runtime graph (`file 338, graph
0x33A0`) — the head node, containing every symptom the user reported
("heavy scrambled multicolour noise across the saddle/head-back region, plus
a smaller patch near the nostril"). Traced to two small CI4 (`PsmT4`)
textures bound by that node: an 8x8 decal at ROM offset `0x9518` (the
mouth-corner mark) and a 16x16 decal at offset `0x9BD0` (both eyes, shared).
Both raw ROM textures decode cleanly (a plain green/black stripe pattern and
Yoshi's eye art, respectively) — additively disabling each at pack time in
turn localized the mouth-corner mark to the first and nearly all of the
ridge/ear noise to the second, and disabling both together left the head
perfectly clean, with no residue. Both textures share the small-paletted
packed shape (8-byte-row-or-narrower `PsmT4`) already proven to trigger a
genuine PSP GE rendering defect for Link's boot and shin cuff — this time
reached through `Clamp`/`Clamp` addressing with no mirror, rather than the
boot/shin-cuff's plain `Wrap`. This is new evidence the GE quirk keys on the
small-paletted-texture shape itself, not specifically the wrap mode. Fixed
with the same paletted-transport bypass those items used
(`tools/romtool/src/main.rs`'s `convert_texture`, new `is_yoshi_head` arm,
`Psm8888` instead of `PsmT4` for these two textures).

**Verification (both platforms, same bar every other closed RE-283 item
met):** PPSSPP — node-isolated capture confirms both eyes render their
correct texture and the mouth-corner mark is a clean small decal, no noise
anywhere; full `regression_capture_yoshi` capture diffs against the prior
(RE-281) golden in a tight bbox (`(436,114)-(543,177)`, 3,920 px) covering
only the head, nothing else moved;
`tools/verify-fighter-goldens.sh` re-run for all 12 playable fighters — all
12 pass, Yoshi against its freshly refreshed golden. Physical — same unit as
every prior RE-283 hardware check (PSP Slim, firmware 6.61, ARK/Infinity,
PSPLink v3.2.1), connected with no USB-permission issue this session: native
`scrshot` of the same node-isolated primitive matches the PPSSPP capture
exactly. `cargo fmt --all --check` and `cargo test --workspace` (617 passed)
clean; plain feature-free `cargo psp --release` EBOOT hash unchanged
(`9b9bfb8f...902ca8c`), confirming the fix is entirely pack-time
(`tools/romtool`), not PSP runtime code. `tests/golden/r2-yoshi-fighter.png`
refreshed and committed. All scratch node-isolation and per-texture
disable/re-enable instrumentation reverted after use.

**A methodology note for future sessions:** partway through this
investigation, a `cargo psp` build failed on an unrelated scratch-debug
compile error; `--no-build` reuses of the stale (silently non-functional)
EBOOT that failure left behind produced misleading "no visible effect" and
even fully blank-screen results for several diagnostic pack variants,
until noticed and every prior diagnostic was redone behind a verified-fresh
full rebuild. Treat a `--no-build` capture as suspect for the rest of a
session after any build failure, and prefer a full rebuild when a result
looks surprising (unchanged, or blank).

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed — neither the Slim nor
the PSP-3000 tested so far is in that RAM class. No dedicated capture scenes
exist yet for full stage/effect coverage or runs longer than 10 minutes.
None of these block R2.2 (closed) — they gate the *physical* R2 matrix only,
and are deliberately deferred until the game structure grows beyond the
asset viewer. RE-272's face-texture bug is fully closed on both PPSSPP and
real hardware — no longer part of the deferred physical-matrix follow-up.
The USB-permission issue that blocked physical work earlier in RE-283's
session history was resolved by reconnecting the PSP (new bus address,
correct `0666` device-node permissions) — not a standing blocker, but worth
a replug first if a future session hits "Permission error while opening the
USB device" again before assuming the udev rule itself needs reinstalling.

Required next action: continue RE-283 with the remaining four untraced
fighters (Captain Falcon, Ness, Pikachu, Kirby), each checked against the
same checklist this session and every prior RE-283 session used (own
ROM-authored state vs. a real tile-addressing/packing quirk vs. genuine
porting bug; check against real N64 output before concluding either way;
node-isolate first, then classify the mechanism — including, per this
session's finding, checking whether *multiple* textures on the same node
each contribute part of one reported symptom before declaring a single
texture the whole cause — then fix or record an `ACCEPTED_DEVIATION`), each
on its own trace — do not assume one fighter's cause generalizes to another
without checking. Physically confirming the PSP-1000 class remains the sole
other *physical*-matrix blocker (unchanged — no PSP-1000 unit available in
this environment) but is secondary to RE-283 until it concludes. Do not
start R3 or combat before both RE-283 and R2's physical matrix are closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-267, RE-269, RE-270, RE-271,
RE-272, RE-273, RE-274, RE-275, RE-276, RE-277, RE-278, RE-279, RE-280,
RE-281, RE-282, RE-283 (see [docs/evidence/INDEX.md](docs/evidence/INDEX.md)
for the full R2.2/physical chain, RE-240–283). Toolchain note: the global
`cargo-psp` install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-283-sixteenth-follow-up pack/code (this session — Yoshi's
head-noise fix, confirmed on PPSSPP and physical PSP hardware).
Pack `808925688a5a40698e459c8b14a89c45e3acbed6f51c4781ba33cadb2637e91c`
(28,546.6 KiB). Plain feature-free EBOOT
`9b9bfb8f94730a47ea04e50cb2a75a8d91536bd9b13f74682256a3ebe902ca8c`
(unchanged from RE-281/the boot and shin-cuff fixes — this fix also touched
only pack-time conversion).
