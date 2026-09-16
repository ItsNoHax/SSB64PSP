# Fighter capture — Ness (RE-210)

Part of [docs/visual-regression/README.md](../README.md).

## Ness fighter capture (RE-210)

`regression_capture_ness` puts Ness on screen, using the same object-viewer
pattern as the prior fighter captures: selects file 335 offset `0x26B0`, Ness's own model
graph (the lower-offset of the file's symmetric 27-node graph pair, matching
the existing fighter graph-pair convention). Ness was chosen because RE-103 named him,
alongside Fox, Captain Falcon and Kirby, as a fighter whose surface
"melted" into rainbow noise under the old per-primitive majority-vote
lit-vs-literal heuristic — a different bug class than the prior fighters'
UV-scale/clamp fix, and the one fighter from RE-103's set still
hardware-untested. RE-264 later adds the exact `(file 335, offset 0xB7A0)`
neutral-face reconstruction correction, removing the same inward eye spikes
RE-263 isolated on Kirby without changing Ness's geometry, UVs, or palette
selection.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_ness
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-ness-fighter.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-210): built and `ldstart`ed under
PSPLink the same way as the other eight scenes. Zero exceptions (`exlist`
empty, `main_thread` alive in `thlist`). Native capture matches this golden
(upscaled 2x nearest-neighbour) with only the expected edge-antialiasing
band and PPSSPP's FPS-counter overlay differing — no solid interior region
of the model differs.
