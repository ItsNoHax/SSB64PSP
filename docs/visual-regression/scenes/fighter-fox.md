# Fighter capture — Fox (RE-207)

Part of [docs/visual-regression/README.md](../README.md).

## Fox fighter capture (RE-207)

Every prior fighter-bearing scene used Mario. `regression_capture_fox`
puts Fox on screen instead, reusing scenes 2–4's object-viewer pattern:
selects file 313 offset `0x2938`, Fox's own model graph, and the same tick-240
freeze, idle-spin freeze, `stage_view` disable and HUD suppression. The graph
was chosen deliberately, not arbitrarily: it is the exact one RE-152 found
and fixed a real bug on (a clamp-window coordinate bug that painted Fox's
lower face solid black), so this scene doubles as a regression check for
that fix rather than an untested pick. RE-264 additionally wraps this scene in
Sector Z's source light (selected by `StageDesc.source_file == 262`), matching
the supplied original-game setting and pinning the white directional material
on Fox's gloves and boots plus the required PSP `GU_LIGHT0` channel enable.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_fox
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-fox-fighter.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-207): built and `ldstart`ed under
PSPLink the same way as the other five scenes. Zero exceptions (`exlist`
empty, `main_thread` alive in `thlist`). Native capture matches this golden
(upscaled 2x nearest-neighbour) with only the expected edge-antialiasing
band and PPSSPP's own FPS-counter overlay differing — no solid interior
region of the model differs, confirming RE-152's fix holds on real hardware.
That physical comparison predates RE-264's corrected directional-light state;
the current golden still requires an R2 hardware re-capture.
