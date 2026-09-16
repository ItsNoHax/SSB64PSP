# Scenes 3-4 — Stage Sector / Catch Swirl (RE-200)

Part of [docs/visual-regression/README.md](../README.md).

## Third and fourth deterministic test scenes (RE-200)

RE-200's graph-backed archive census found that no one scene graph covers all
four remaining matrix rows. Two exact object keys cover them with the smallest
additional suite:

* `regression_capture_scene3` selects file 109 (`StageSectorFile2`) graph
  `0x44C8`. Its converted graph contains 7 `combiner_texture_blend`
  primitives, 5 classified translucent primitives, and 8 clean clamped
  textures with neither mirror axis set.
* `regression_capture_scene4` selects file 84 (`EFCommonEffects2`) graph
  `0x2760`, the decomp-named `CatchSwirlDObjDesc`. Its four rendered quads
  use the classified flat-constant-colour path.

Both features reuse scene 2's tick-240 freeze, idle-spin freeze, object-view
boot, and HUD suppression. Selection matches both `source_file` and
`source_offset`; files 84 and 109 each contain several unrelated graphs.

Build and compare each scene with:

```
cd psp && cargo psp --release --features regression_capture_scene3
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r1-stage-sector.png ~/ppsspp-test/screenshot.png

cd psp && cargo psp --release --features regression_capture_scene4
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r1-catch-swirl-flat-color.png ~/ppsspp-test/screenshot.png
```

These images are deterministic port-regression baselines. Source-classified
primitive membership proves each path is exercised; the images do not claim
pixel equivalence to an original N64 capture.
