# Fighter capture — Donkey Kong (RE-212)

Part of [docs/visual-regression/README.md](../README.md).

## Donkey Kong fighter capture (RE-212)

`regression_capture_donkey_kong` puts Donkey Kong on screen, using the same
object-viewer pattern as the prior fighter captures: selects file 317 offset `0x39A8`,
DK's own model graph (the lower-offset of the file's symmetric 26-node
graph pair, matching the existing fighter graph-pair convention). DK was chosen as the
next untested fighter in `FIGHTER_COSTUME_COUNTS`
(`tools/romtool/src/main.rs`) order once RE-102's and RE-103's own named
fighter sets were both exhausted (RE-207–210).

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_donkey_kong
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-dk-fighter.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-212): built and `ldstart`ed under
PSPLink the same way as the other nine scenes. The first two attempts
produced the built-in fallback tetrahedron instead of DK despite `exlist`
being empty and `main_thread` alive in `thlist` — a silently failed
asset-pack open, invisible to this project's usual health checks. `pspsh -e
reset` before the next `ldstart` resolved it; see RE-212 and
`docs/psplink.md` for the methodology finding. Once resolved: zero
exceptions, native capture matches this golden (upscaled 2x
nearest-neighbour) with only the expected edge-antialiasing band, PSPLink's
status text, and PPSSPP's FPS-counter overlay differing — no solid interior
region of the model differs.
That physical comparison predates RE-264's face correction; the current golden
still requires an R2 hardware re-capture.
