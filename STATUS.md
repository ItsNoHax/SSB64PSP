# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current pack, separate from RE-272's
now-closed Mario-face artifact. Eight of nine items are fully closed, each
confirmed on both PPSSPP and physical PSP hardware: Fox, Mario/Luigi, Samus,
Link's boot, Link's shin cuff, Yoshi, Captain Falcon, and (this session)
Ness.

**Ness's shoulder-strap noise: closed on both PPSSPP and physical
hardware.** Node-isolated to node 4 of the runtime graph (`file 335, graph
0x26B0`) — the right upper-arm/shoulder segment, containing the user-reported
"scrambled pixel noise" at the collar/sleeve junction. A raw `dlraw`-style
decode of this node's display list first gave a false lead (three clean
16x32 shirt-stripe textures at offsets `0xAD00`/`0xAE30`/`0xAB20`); bypassing
each individually and together, rebuilding the pack each time, produced zero
visible change — a repeat of the same "raw single-list decode can desync
silently" trap this chain already documented for Captain Falcon. The
authoritative `pack()`/`plan_draw_order` pipeline (a temporary
`romtool scene --dump-node` diagnostic reading the real resolved
`MeshMaterial`) recovered the actual bound texture: an 8x8 CI4 decal at ROM
offset `0xBDE8`, `Clamp`/`Clamp` addressing narrowed by a mirror to a drawn
8x16 — the same small-paletted packed shape (8-byte-row-or-narrower
`PsmT4`) as every other closed item in this chain. The raw ROM texture
decodes cleanly (a plain yellow/black stripe). Fixed with the same
paletted-transport bypass those items used (`tools/romtool/src/main.rs`'s
`convert_texture`, new `is_ness_shoulder` arm, `Psm8888` instead of `PsmT4`).

**Verification (both platforms, same bar every other closed RE-283 item
met):** PPSSPP — node-isolated capture confirms node 4 renders clean with no
scrambled patch, and the full-pose `regression_capture_ness` capture diffs
against the prior golden in a tight bbox (`(418,268)-(555,309)`, 137x41 px)
covering only the collar/shoulder region, nothing else moved;
`tools/verify-fighter-goldens.sh` re-run for all 12 playable fighters — all
12 pass, Ness against its freshly refreshed golden. Physical — same unit as
every prior RE-283 hardware check (PSP Slim, firmware 6.61, ARK/Infinity,
PSPLink v3.2.1): native `scrshot` of the full-pose build shows the identical
clean striped shoulder, no scrambled patch, no exceptions (`exlist` empty,
`main_thread` alive throughout). `cargo fmt --all --check` and
`cargo test --workspace` (618 passed) clean; plain feature-free
`cargo psp --release` EBOOT hash unchanged (`9b9bfb8f...902ca8c`), confirming
the fix is entirely pack-time (`tools/romtool`), not PSP runtime code.
`tests/golden/r2-ness-fighter.png` refreshed and committed. All scratch
node-isolation and diagnostic instrumentation reverted after use.

**A methodology note for future sessions:** partway through an earlier
RE-283 session, a `cargo psp` build failed on an unrelated scratch-debug
compile error; `--no-build` reuses of the stale (silently non-functional)
EBOOT that failure left behind produced misleading "no visible effect" and
even fully blank-screen results for several diagnostic pack variants,
until noticed and every prior diagnostic was redone behind a verified-fresh
full rebuild. Treat a `--no-build` capture as suspect for the rest of a
session after any build failure, and prefer a full rebuild when a result
looks surprising (unchanged, or blank). Separately, this session confirmed
another failure mode from the same chain: a raw single-list command dump
(`romtool dlraw`, no `convert_sequence` resolution) can desync silently on
an unrecognized opcode and produce a plausible-looking but false command
stream — always cross-check a raw decode against the authoritative
`pack()`/`plan_draw_order` pipeline before trusting an offset it reports,
not after a fix test built on it has already failed to explain the result.

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

Required next action: continue RE-283 with the remaining two untraced
fighters (Pikachu, Kirby), each checked against the same checklist every
prior RE-283 session used (own ROM-authored state vs. a real
tile-addressing/packing quirk vs. genuine porting bug; check against real
N64 output before concluding either way; node-isolate first, then classify
the mechanism — including checking whether *multiple* textures on the same
node each contribute part of one reported symptom before declaring a single
texture the whole cause, and cross-checking any raw single-list command
decode against the authoritative `pack()`/`plan_draw_order` pipeline before
trusting it, per this chain's `dlraw` desync finding (now repeated twice,
Falcon and Ness both) — then fix or record an `ACCEPTED_DEVIATION`), each on
its own trace — do not assume one fighter's cause generalizes to the next
one without checking. Physically confirming the PSP-1000 class remains the
sole other *physical*-matrix blocker (unchanged — no PSP-1000 unit available
in this environment) but is secondary to RE-283 until it concludes. Do not
start R3 or combat before both RE-283 and R2's physical matrix are closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-267, RE-269, RE-270, RE-271,
RE-272, RE-273, RE-274, RE-275, RE-276, RE-277, RE-278, RE-279, RE-280,
RE-281, RE-282, RE-283 (see [docs/evidence/INDEX.md](docs/evidence/INDEX.md)
for the full R2.2/physical chain, RE-240–283). Toolchain note: the global
`cargo-psp` install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-283-eighteenth-follow-up pack/code (this session —
Ness shoulder-strap fix, confirmed on PPSSPP and physical PSP hardware).
Pack `256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`
(28,572.0 KiB). Plain feature-free EBOOT
`9b9bfb8f94730a47ea04e50cb2a75a8d91536bd9b13f74682256a3ebe902ca8c`
(unchanged from RE-281/the boot, shin-cuff, Yoshi and Falcon fixes — this
fix also touched only pack-time conversion).
