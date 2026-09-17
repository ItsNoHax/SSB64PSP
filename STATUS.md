# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: `RE-283` (the reopened fighter-texture quality gate) is
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
available in this environment, so this item is parked rather than pursued.
No dedicated capture scenes exist yet for full stage/effect coverage or
runs longer than 10 minutes; Kirby's per-copy-ability hat graphs also
remain untested (see the Pikachu/Kirby note above — non-default costumes
are now checked and clean, so this is narrower than before). None of these
block R2.2 (closed) — they gate physical-matrix/golden-coverage breadth
only, deliberately deferred until the game structure grows beyond the
asset viewer. The USB-permission issue that blocked physical work earlier
in RE-283's session history was resolved by reconnecting the PSP (new bus
address, correct `0666` device-node permissions) — not a standing blocker,
but worth a replug first if a future session hits "Permission error while
opening the USB device" again before assuming the udev rule itself needs
reinstalling.

Required next action: RE-283's per-fighter chase is complete — all nine
reported items are accounted for (eight fixed and hardware-confirmed, two
traced to no reproducible defect, including their non-default costumes).
PSP-1000 confirmation is parked (no unit available). The next R2 work is
building dedicated capture scenes for the coverage this STATUS's
longstanding note calls out: full stage/effect coverage, runs longer than
10 minutes, and (lowest priority, gated on copy-ability gameplay existing
at all) Kirby's per-ability hat graphs. Do not start R3 or combat before
R2's physical matrix is closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-267, RE-269, RE-270, RE-271,
RE-272, RE-273, RE-274, RE-275, RE-276, RE-277, RE-278, RE-279, RE-280,
RE-281, RE-282, RE-283 (see [docs/evidence/INDEX.md](docs/evidence/INDEX.md)
for the full R2.2/physical chain, RE-240–283). Toolchain note: the global
`cargo-psp` install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-283-eighteenth-follow-up pack/code (Ness shoulder-strap
fix, confirmed on PPSSPP and physical PSP hardware) — unchanged this
session; Pikachu/Kirby tracing needed no code fix. Pack
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`
(28,572.0 KiB). Plain feature-free EBOOT
`9b9bfb8f94730a47ea04e50cb2a75a8d91536bd9b13f74682256a3ebe902ca8c`
(unchanged since RE-281/the boot, shin-cuff, Yoshi, Falcon and Ness fixes).
