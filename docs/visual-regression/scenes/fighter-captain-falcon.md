# Fighter capture — Captain Falcon (RE-208)

Part of [docs/visual-regression/README.md](../README.md).

## Captain Falcon fighter capture (RE-208)

`regression_capture_captain_falcon` puts Captain Falcon on screen, using the
same object-viewer pattern: selects file 332 offset `0x3BE0`, Falcon's
own model graph (the lower-offset of the file's two symmetric 26-node
graphs, matching Fox's graph-pair convention). Falcon was chosen
over the other untested fighters because RE-102 (R0.5) already named him,
alongside Fox and Kirby specifically, as one of three fighters with a real
UV-scale/clamp bug on face/torso/head textures — this scene extends the
same regression lineage RE-207 started for Fox to a second fighter.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_captain_falcon
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-falcon-fighter.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-208): built and `ldstart`ed under
PSPLink the same way as the other six scenes. Zero exceptions (`exlist`
empty, `main_thread` alive in `thlist`). Native capture matches this golden
(upscaled 2x nearest-neighbour) with only the expected edge-antialiasing
band, PSPLink's status text, and PPSSPP's FPS-counter overlay differing — no
solid interior region of the model differs.
