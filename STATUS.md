# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: **RE-286 (this session): all 41 stage entries now have a
deterministic PPSSPP software golden** (4 pre-existing + 37 added this
session — Zebes, Mushroom Kingdom, Kongo Jungle, Yoshi's Island, Hyrule
Castle, Final Destination, the tutorial map, the Metal Mario stage, Beta
Dream Land, the Test Stage, Small Yoshi's Island, Duel Zone, and all 25
bonus maps). Corrected an undercount in this file's own prior text and in
RE-285: the true remaining set was 37, not 33 — `refs/ssb-decomp-re/src/mp/
mpcollision.c`'s `dMPCollisionGroundFileInfos` table (decomp ground truth)
confirms Mushroom Kingdom (file 260) is a real selectable stage, not a test
variant. Added one parametrized Cargo feature
(`regression_capture_stage_index`, `SSB64_STAGE_INDEX` build-time env var)
rather than 37 near-identical named features — see RE-286 for the full
rationale. All 37 new goldens are deterministic (byte-identical across two
capture timings) and zero regression on every pre-existing golden (spot-
checked: default Dream Land, Peach's Castle, Saffron City gate, Kirby
fighter, all 0 differing pixels); `cargo fmt`/`cargo test --workspace`
(425 passed) clean; plain EBOOT rebuilt after (`2dbce3ee...818f47`); pack
unchanged (`256d7661...42c17d`) since no asset-pipeline code changed.
**Physical-PSP hardware confirmation for these 37 is explicitly deferred**
(user's own choice this session, given the cost of 37 sequential PSPLink
cycles) — tracked as open work below, not silently assumed passing. Stage
coverage is 41/41 on software, 4/41 hardware-confirmed (the pre-existing
Dream Land, Sector Z, Saffron City, Peach's Castle).

Prior objective: `RE-283` (the reopened fighter-texture quality gate) is
now fully traced — all nine user-reported items are accounted for. Eight
are fixed and confirmed on both PPSSPP and physical PSP hardware: Fox,
Mario/Luigi, Samus, Link's boot, Link's shin cuff, Yoshi, Captain Falcon,
and Ness. **This session traced the remaining two (Pikachu, Kirby) and
found neither reproduces the scrambled-CI4 defect class** — no code fix was
needed or applied for either.

**Pikachu:** node-isolating its default Wait-pose graph (file 341, graph
`0x2650`) found the reported "face colour mismatch" is not present in the
current build (eyes/cheeks pixel-sampled clean) — consistent with RE-281's
CI4/`PsmT4` nibble fix already having changed Pikachu's golden. The
reported "cracks" reproduced exactly: a dark streak across the collar,
traced to an 8-byte-row CI4 gradient decal (file 341 `+0x7350`, 16x8,
`mirror_s`+`clamp_t`) bound to nodes 1/2. Unlike every other RE-283 item,
applying the same `Psm8888` paletted-transport bypass produced **zero**
pixel change — a clean negative result, not an unexplained one: the raw ROM
texel data is an uncorrupted gradient, and the primitive's own UV span
(3.47 T-repeats under `Clamp`) predicts exactly this streak as ordinary
clamp-past-the-edge addressing (RE-067/RE-102's already-established
mirror+clamp conversion) holding the gradient's darkest row — not a PSP GE
rendering defect. No fix applied.

**Kirby:** node-isolating every textured/geometry node reachable from its
default Wait-pose graph (file 328, graph `0x1448`) found only one texture
in the whole model — a 32x32 CI4 face decal (`+0x1CF60`) — and its raw ROM
decode is pixel-identical to the rendered face (correct eyes/cheeks/mouth,
no mismatch). Kirby's arms and feet carry **no texture at all** (solid
Gouraud-shaded geometry), so nothing in this defect class could apply
there. The file's other, stranger textures (including one visibly borrowed
from Yoshi's own file 338 that decodes to Yoshi's red saddle shape) all
belong to Kirby's separate per-copy-ability hat graphs or its four
non-default costumes — none reachable from the tested costume-0 body, so
they cannot explain a defect in the current golden. A genuine Kirby-model
defect was not found.

**Verification (both fighters, this session):** software-only — no code
changed, so no new build/pack/hardware verification was needed or run;
`assets/generated/ssb64.pak` SHA-256 is unchanged
(`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`) after
this session's scratch instrumentation (a temporary `romtool scene
--dump-node` diagnostic and a temporary `RE283_ONLY_LOCAL_NODE`-gated node
filter in `psp/src/main.rs`, both reverted with `git checkout --`
immediately after use, matching this chain's established practice for
throwaway diagnostics).

**Follow-up in the same session: non-default costumes checked and clean.**
A temporary `RE283_COSTUME_OVERRIDE` env-gated costume index (reverted after
use) rendered all 4 of Kirby's and all 3 of Pikachu's non-default costumes.
All 7 are clean — correct per-costume recolour, no scrambling, no new
artifacts, and Pikachu's collar streak unchanged in severity across every
costume (consistent with it being ROM-authored, not a costume-specific
bug). The only remaining untested surface is Kirby's per-copy-ability hat
graphs (separate small scene graphs, not reachable from the base body) —
not currently reachable in normal play either, since combat/copy-abilities
are not implemented yet. Not a standing R2 blocker.

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed — neither the Slim nor
the PSP-3000 tested so far is in that RAM class; no PSP-1000 unit is
available in this environment, so **this item is parked, not pursued, per
explicit instruction this session.** No dedicated capture scenes exist yet
for full stage/effect coverage (41 stages, most uncovered — this is a large,
deliberately deferred effort, not a bounded next step); Kirby's
per-copy-ability hat graphs also remain untested (see the Pikachu/Kirby
note above — non-default costumes are now checked and clean). **The
"runs longer than 10 minutes" gap is now narrowed:** RE-284 (this session)
ran the plain feature-free EBOOT 30 continuous real-time minutes on the
PSP Slim — zero exceptions at six 5-minute checkpoints, `main_thread` alive
throughout, t=0/t=30 native captures differing only in a tiny
Mario-idle-pose region (43 px, no corruption/leak). Only one unit and one
duration tier tested; a second unit and longer/real-play-length runs remain
open. None of these block R2.2 (closed) — they gate physical-matrix/golden-
coverage breadth only, deliberately deferred until the game structure grows
beyond the asset viewer. The USB-permission issue that blocked physical
work earlier in RE-283's session history was resolved by reconnecting the
PSP (new bus address, correct `0666` device-node permissions) — not a
standing blocker, but worth a replug first if a future session hits
"Permission error while opening the USB device" again before assuming the
udev rule itself needs reinstalling. **New note (RE-284): if `pspsh -e ver`
returns "connection refused" despite the PSP showing `054c:01c9` on USB and
PSPLink visibly launched, start `usbhostfs_pc -v "$PWD"` first** — it
bridges the USB link `pspsh` actually connects to; this isn't spelled out
in the `psp-hardware` Skill's own connectivity-check step.

Required next action: RE-283's per-fighter chase is complete — all nine
reported items are accounted for (eight fixed and hardware-confirmed, two
traced to no reproducible defect, including their non-default costumes).
PSP-1000 confirmation is parked (no unit available, and out of scope per
explicit instruction). The 30-minute sustained-run check (RE-284) is done
and clean. **RE-286 (this session) closes the software side of stage
coverage: all 41/41 stage entries now have a deterministic PPSSPP golden.**
Remaining R2 coverage work, in rough priority order: (1) physically confirm
the 37 new stage goldens on real PSP hardware (batched PSPLink sessions,
deliberately deferred from this session per explicit user choice — do not
assume they pass on hardware just because PPSSPP is clean, per this
project's own repeated PPSSPP-vs-physical-PSP GE-quirk findings, e.g.
RE-283's Link boot/shin/Yoshi/Falcon/Ness paletted-texture defects, none of
which PPSSPP alone would have caught), (2) a second physical unit for the
long-duration check (parity with the existing 10-minute two-unit sample),
and (3) lastly (gated on copy-ability gameplay existing at all) Kirby's
per-ability hat graphs. Do not start R3 or combat before R2's physical
matrix is closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-267, RE-269, RE-270, RE-271,
RE-272, RE-273, RE-274, RE-275, RE-276, RE-277, RE-278, RE-279, RE-280,
RE-281, RE-282, RE-283, RE-284, RE-285, RE-286 (see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–286). Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-286 code (37 new stage-golden Cargo/main.rs wiring,
software-only this session) on top of RE-283's eighteenth-follow-up fixes
(Ness shoulder-strap, confirmed on PPSSPP and physical PSP hardware). Pack
unchanged this session (no asset-pipeline code touched):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`
(28,572.0 KiB). Plain feature-free EBOOT rebuilt after RE-286's source
changes (expected hash change from source-only edits, no behavior change —
confirmed by the zero-differing-pixel regression checks in RE-286):
`2dbce3ee1c718a5ebf374ef65291499d08d1429f49abc000a89dbc97b4818f47`. Not yet
staged to physical hardware this session.
