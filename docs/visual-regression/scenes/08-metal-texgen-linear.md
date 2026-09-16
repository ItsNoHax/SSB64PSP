# Scene 8 — Metal texgen linear (RE-215)

Part of [docs/visual-regression/README.md](../README.md).

## Eighth deterministic test scene (RE-215)

`regression_capture_scene8` selects `StageMetalFile2`'s **second** graph
(file 117, offset `0x2EE0`, two nodes) in the same object-viewer pattern as
scenes 6/7. Verifying RE-215's exact `G_TEXTURE_GEN_LINEAR` fix found that
scenes 6/7's graph (`0x1B10`) carries only *ordinary* texgen primitives —
the archive's one packed linear primitive (file 117, texture 499, 12
triangles) lives in this sibling graph instead. A reachability check (a debug
colour marker in the linear-texgen branch) and a direct read of
`assets/generated/ssb64.pak` both confirmed it before this scene was added;
see RE-215 for the full account, including the correction to an earlier
RE-214 note that had claimed scene 6 itself carried linear triangles.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene8
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-metal-texgen-linear.png ~/ppsspp-test/screenshot.png
```

The fix's effect was confirmed against this exact scene before adopting the
golden: rebuilding with the pre-fix ordinary-mapping code (everything but the
scene's own wiring reverted) and comparing against the fixed build showed
10,766 differing pixels — not inert. Two captures of the fixed build are
byte-identical (deterministic). Scenes 6, 7, Dream Land, and the Fox
fighter golden (the only fighter-viewer check run here; no fighter golden
carries texgen) are all still byte-identical to their existing goldens —
this fix is isolated to the one graph that actually carries linear content.

Physical PSP hardware verification (RE-215): `ldstart`ed under PSPLink with
`exlist` empty and `main_thread` alive; native capture visually matches the
PPSSPP golden (pink/tan reflective facet, yellow flag panel, gold crystal
band). Hashes recorded in RE-215.

RE-235 (`PLAN.md` R2.1/T8) refreshed `r2-metal-texgen-linear.png` for the
same post-T7a reason as scenes 6/7 above; RE-215's physical-PSP match was
against the pre-T7a golden. RE-236 re-captured this scene on physical PSP
against the refreshed golden (30,345 differing pixels 2x-upscaled, same
noise-floor order as the other two), closing `R2.1`/T8.

RE-259 corrects those entries' visual attribution: the old smooth top bar
was **not** the linear-texgen primitive. A temporary build skipping exactly
`TEXTURE_GEN_LINEAR` removed the large pink/tan crystal cluster on the right
while leaving the top bar intact. The bar's later lattice appearance is the
expected result of RE-252/RE-253 removing a first-wins collision between
non-linear file-117 baked-texture variants. The scene still covers the one
linear primitive; its on-screen region is the crystal cluster, not the bar.
