# Scene 2 — MVOpeningRoom (RE-199)

Part of [docs/visual-regression/README.md](../README.md).

## The second deterministic test scene (RE-199)

`regression_capture`'s Dream Land scene is `stage_view`; it never exercises
the object-view code path at all, so it cannot cover assets that only exist
outside Dream Land's own files. RE-198 tied three uncovered test-matrix rows
to concrete files (CI8 texture and untextured/vertex-coloured geometry to
file 52, clamp texture mode to file 22) but did not build anything to put
them on screen.

RE-199 adds a second Cargo feature, `regression_capture_scene2`, on the same
`ssb64-psp` crate, off by default like `regression_capture`. It:

* disables the default Dream Land `stage_view` boot (added to the same
  `cfg!(any(...))` list `animation_audit_capture`/`effect_audit_capture`/etc.
  already use to do this);
* overrides the object viewer's normal "deepest hierarchy" boot heuristic
  (`psp-asset-viewer/src/main.rs`, object_index selection) to file 52's own graph
  (`mvopeningroom.c`'s "MVCommon" scene) by matching `ObjectDesc.source_file
  == 52` — the same graph the depth/triangle heuristic already found and
  rejected for the *first* golden scene, because its 38 flat cutscene panels
  "look exactly like a rendering bug" in that context. That is precisely
  what makes it the concrete carrier of RE-198's CI8 (offset `0x2ee8`) and
  untextured/vertex-coloured (mesh index 4, primitive 0) examples;
* reuses `deterministic_capture_frozen`'s existing tick-240 freeze
  (extended to recognise this feature too) and the existing HUD-suppression
  `cfg!(any(...))` list (same extension) for a clean, exact-match frame;
* additionally freezes the object viewer's own idle model spin (`spin +=
  0.02` per frame), which `regression_capture`'s `stage_view` path never
  exercises and which is not covered by `deterministic_capture_frozen` --
  every other object-view-based audit tolerates spin drift because it only
  checks a captured frame is non-blank, not that two captures are pixel
  identical. Measured: without this, two captures of the same build 24
  real seconds apart differed by 126,693 pixels; with it, byte-identical
  (see Evidence).

Build and capture it the same way as the first scene, substituting the
feature name:

```
cd psp && cargo psp --release --features regression_capture_scene2
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r1-mvopeningroom.png ~/ppsspp-test/screenshot.png
```

Follow with a plain `cargo psp --release` before resuming normal work, for
the same reason `regression_capture` requires it.
