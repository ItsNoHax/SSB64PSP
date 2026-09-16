# Fighter capture — Kirby (RE-209)

Part of [docs/visual-regression/README.md](../README.md).

## Kirby fighter capture (RE-209)

`regression_capture_kirby` puts Kirby on screen, using the same object-viewer
pattern as the prior fighter captures: selects file 328 offset `0x1448`, Kirby's own model
graph (the lower-offset of the file's symmetric 27-node graph pair, matching
Fox and Falcon's graph-pair convention). Kirby was chosen because
RE-102 (R0.5) named him, alongside Fox and Falcon, as the third of three
fighters with a real UV-scale/clamp bug on face/torso/head textures — this
scene completes the regression lineage RE-207/RE-208 started for that bug
class. RE-263 later extended the scene to pin the PSP reconstruction
correction for Kirby's 32x32 neutral face: the expected image has two upright
oval eyes without the false inward spikes produced by raw CI4 magnification.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_kirby
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-kirby-fighter.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-209): built and `ldstart`ed under
PSPLink the same way as the other seven scenes. Zero exceptions (`exlist`
empty, `main_thread` alive in `thlist`). Native capture matches this golden
(upscaled 2x nearest-neighbour) with only the expected edge-antialiasing
band, PSPLink's status text, and PPSSPP's FPS-counter overlay differing — no
solid interior region of the model differs.
