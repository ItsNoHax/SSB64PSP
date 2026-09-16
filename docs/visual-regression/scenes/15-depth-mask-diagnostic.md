# Scene 15 — Synthetic depth-mask diagnostic (RE-251)

Part of [docs/visual-regression/README.md](../README.md).

## Fifteenth scene: synthetic depth-mask diagnostic (RE-251)

No ROM-content golden's frozen camera window happens to put a depth-test-
without-write surface in front of something drawn after it at an overlapping
screen position — not even stage 7 (Sector Z), whose render-layer 1 has more
`PlannedList` list-1 (translucent, depth-test-without-write) content than any
other stage's layer 1 except one: a whole-stage capture of it is
byte-identical before and after RE-251's `apply_material` wiring change. A
regression relying only on existing golden scenes would not catch a broken
`sceGuDepthMask` wire, so `depth_mask_diagnostic` (`psp/src/depth_diag.rs`)
adds a synthetic, non-ROM scene, the same shape `normal_diag.rs`'s
`texgen_normal_diagnostic_*` rig uses for a different GE-contract question
this crate has no other way to exercise.

Three overlapping quads, all drawn under whatever camera the default stage
view already established that frame:

1. Opaque red, farthest (`z = -10`). Depth test on, write on.
2. Translucent green (alpha 0.5), nearest (`z = -6`), blended. Depth test on,
   write **off**.
3. Opaque blue, *between* the two in depth (`z = -8`), drawn last. Depth
   test on, write on again — the ON→OFF→ON switch.

Blue is farther than green, so if green had wrongly written depth, blue's
test against green's near value would fail and it would stay hidden. If
`sceGuDepthMask` correctly kept green from writing, the buffer still holds
red's far value when blue tests, blue passes, and its opaque colour shows
through *in front of* green on screen despite lying behind it in depth and
being submitted after it — real-time translucency compositing, the exact
property RE-244's `ZMODE_XLU` exists for. Correct output is a solid blue
disc nested inside a blended olive (red+green) square inside a red square;
a broken write wire collapses the blue disc away entirely.

Build and compare:

```
cd psp && cargo psp --release --features depth_mask_diagnostic
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-depth-mask-diagnostic.png ~/ppsspp-test/screenshot.png
```

**Test-the-test.** Temporarily forcing the green quad's `sceGuDepthMask` call
to `0` (simulating a broken wire) made the blue disc vanish entirely in a
PPSSPP headless capture, confirmed, then reverted — this regression actually
discriminates a working wire from a broken one, not just "something drew."
Two captures of the fixed build are byte-identical (0 differing pixels). New
golden: `tests/golden/r2-depth-mask-diagnostic.png`, SHA-256
`47d0cb28107e790b9a3e524f8352e59752e2d277fb63403d019890b3f9745890`.

Physical-PSP hardware verification: not yet executed (the PSP was
enumerated over USB but not in an active PSPLink session this session could
establish — launching PSPLink from the device's own XMB needs physical
interaction). `PLAN.md` C3's acceptance line accepts PPSSPP *or* physical-PSP
evidence; this scene's PPSSPP evidence, including the bug-injection
self-check, closes C3 on its own. A future session's hardware pass remains
open work, the same shape several earlier scenes' hardware rows started in.

**Existing-golden impact.** Wiring `apply_material` to the independent
`depth_test`/`depth_write` state (superseding the `z_buffer` proxy) changed 4
of the 14 prior golden scenes, measured against a same-environment pre/post
rebuild rather than the possibly-stale committed PNGs directly (see RE-251's
own environment-drift note): `r0-dream-land-default.png` (1120 px, canopy
translucent-highlight and hull-texture occlusion), `r2-metal-texgen.png`
(220 px), `r2-metal-texgen-rotated.png` (13,008 px, one crystal facet's
occlusion flips), and `r2-metal-texgen-camera-rotated.png` (1704 px). The
other 9 (2–5, 7–10, 13) are byte-identical — confirmed for Fox's own graph
that all 30 of its packed primitives already read `z_buffer == depth_test ==
depth_write == true`, so the new wiring changes nothing for them. All four
changed goldens were refreshed after visual inspection found each diff
localized and explainable, not corrupted (see RE-251).
