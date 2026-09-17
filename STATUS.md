# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` reopened the fighter-texture quality gate —
user-reported visual defects (missing Mario/Luigi overalls buttons, texture
speckle on Fox/Samus/Link/Yoshi/Falcon/Ness, Pikachu/Kirby face-colour
mismatch) are confirmed real on the current pack, separate from RE-272's
now-closed Mario-face artifact. Five of nine items are fully closed, each
confirmed on both PPSSPP and physical PSP hardware: Fox, Mario/Luigi, Samus,
Link's boot, and (this session) Link's shin cuff.

**Link's shin-cuff speckle: closed this session, on both PPSSPP and
physical hardware.** Node-isolated to node 23 of the runtime graph (`file
324, graph 0x3AE8`) — one specific leg's shin/greave mesh; its mirrored
counterpart (node 28) was already clean. Both nodes bind the identical
texture (`data_offset 0xB4E0`, 16x32 `Ci/Bits4`, same 8-byte-row `PsmT4`
packed shape as the now-fixed boot texture); the only difference between
the two nodes' otherwise-byte-identical display lists is `G_SETTILE`'s
S-axis wrap mode — `Wrap` (plain repeat) on the speckled node, `Mirror` on
the clean one. Root cause: the same PSP GE rendering quirk the boot defect
already proved real (a genuine hardware behavior for this small-paletted-
texture packed shape, not a stale bind or an addressing-formula bug —
`ssb_rom::n64_addressing`'s own reference model agrees with plain
`Wrap`/`Repeat` exactly), triggered specifically under `Wrap` addressing of
this texture shape and not under `Mirror`. Fixed with the same
paletted-transport bypass the boot used (`tools/romtool/src/main.rs`'s
`convert_texture`, new `is_link_shin` arm, `Psm8888` instead of `PsmT4` for
this one texture). Full detail, including the raw display-list bytes
decoded by hand, the exact bbox/pixel-count blast-radius check, and the
physical-hardware capture, in `docs/evidence/re/RE-283.md`'s fifteenth
follow-up.

**Verification (both platforms, same bar every other closed RE-283 item
met):** PPSSPP — node-isolated capture confirms the patch is gone; full
`regression_capture_link` capture diffs against the prior (boot-fixed)
golden in a tight bbox (`(492,350)-(533,421)`, 2,260 px) directly above the
boot's own bbox, nothing else moved; `tools/verify-fighter-goldens.sh`
re-run for all 12 playable fighters — 11 byte-identical to their existing
goldens, Link matches its freshly refreshed one. Physical — same unit as
every prior RE-283 hardware check (PSP Slim, firmware 6.61, ARK/Infinity,
PSPLink v3.2.1): a first `usbhostfs_pc` attempt failed on a USB-permission
error, the user physically reconnected the device, and the replug (new bus
address, correct device-node permissions) resolved it — native `scrshot` of
the same node-isolated primitive shows a clean brown greave, no
purple/black patch. `cargo fmt --all --check` and `cargo test --workspace`
(617 passed) clean; plain feature-free `cargo psp --release` EBOOT hash
unchanged (`9b9bfb8f...902ca8c`), confirming the fix is entirely pack-time
(`tools/romtool`), not PSP runtime code. `tests/golden/r2-link-fighter.png`
refreshed and committed. All scratch node-isolation instrumentation
reverted both times it was used this session.

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed — neither the Slim nor
the PSP-3000 tested so far is in that RAM class. No dedicated capture scenes
exist yet for full stage/effect coverage or runs longer than 10 minutes.
None of these block R2.2 (closed) — they gate the *physical* R2 matrix only,
and are deliberately deferred until the game structure grows beyond the
asset viewer. RE-272's face-texture bug is fully closed on both PPSSPP and
real hardware — no longer part of the deferred physical-matrix follow-up.
The USB-permission issue that blocked physical work earlier this session was
resolved by reconnecting the PSP (new bus address, correct `0666` device-node
permissions) — not a standing blocker, but worth a replug first if a future
session hits "Permission error while opening the USB device" again before
assuming the udev rule itself needs reinstalling.

Required next action: continue RE-283 with the remaining five untraced
fighters (Yoshi, Captain Falcon, Ness, Pikachu, Kirby), each checked against
the same checklist this session and the boot session both used (own
ROM-authored state vs. a real tile-addressing/packing quirk vs. genuine
porting bug; check against real N64 output before concluding either way;
node-isolate first, then classify the mechanism, then fix or record an
`ACCEPTED_DEVIATION`), each on its own trace — do not assume one fighter's
cause generalizes to another without checking. Physically confirming the
PSP-1000 class remains the sole other *physical*-matrix blocker (unchanged —
no PSP-1000 unit available in this environment) but is secondary to RE-283
until it concludes. Do not start R3 or combat before both RE-283 and R2's
physical matrix are closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-267, RE-269, RE-270, RE-271,
RE-272, RE-273, RE-274, RE-275, RE-276, RE-277, RE-278, RE-279, RE-280,
RE-281, RE-282, RE-283 (see [docs/evidence/INDEX.md](docs/evidence/INDEX.md)
for the full R2.2/physical chain, RE-240–283). Toolchain note: the global
`cargo-psp` install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-283-fifteenth-follow-up pack/code (this session — Link's
shin-cuff fix, confirmed on PPSSPP and physical PSP hardware).
Pack `d49eb5ee56d35c6b705b226fd8f22556cdb7ee0a3624244e4272af5b44660a3d`
(28,539.3 KiB). Plain feature-free EBOOT
`9b9bfb8f94730a47ea04e50cb2a75a8d91536bd9b13f74682256a3ebe902ca8c`
(unchanged from RE-281/the boot fix — this fix also touched only pack-time
conversion).
