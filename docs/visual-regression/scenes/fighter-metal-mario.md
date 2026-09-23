# Fighter capture — Metal Mario (RE-311)

Part of [docs/visual-regression/README.md](../README.md).

## Metal Mario neutral capture (RE-311)

`regression_capture_metal_mario` uses the same fighter path as the other
playable-fighter captures (see `all-fighters-neutral.md`). It selects Metal
Mario's high-detail `FTCommonPart` (fighter kind 13, file 300, graph
`0x1E08`, recovered by `ssb_rom::fighter::common_parts`). It then starts the
kind-13 `Wait` figatree, freezes at tick 240, and applies Dream Land's shared
source fighter light.

Metal Mario is the only fighter golden whose body draws ordinary
`G_TEXTURE_GEN`. No explicit camera look-at is set, so the texgen basis is
the identity look-at. This matches the fixed viewer camera, which looks down
`-Z` with `+Y` up. `DrawState::texgen_object_basis` carries that basis into
each posed node's space. The reflection therefore depends on the real `Wait`
pose. The capture pins the following:

* the GE texture-matrix texgen path on a posed, multi-node fighter;
* the shared metal reflection map (file 302 `0x30`, source texels);
* the body texture, file 300 `0x2490` (48x42 CI8). In RE-311, all 14 of its
  texgen primitives accept real-normal filter compensation variants.

Build, capture and compare:

```
tools/run-ppsspp-headless.sh --feature regression_capture_metal_mario
tools/compare-screenshot.sh tests/golden/r2-metal-mario-fighter.png ~/ppsspp-headless-test/screenshot.png
```

`tools/verify-fighter-goldens.sh` includes this scene.

Measured on 2026-09-24, with PPSSPPHeadless using the software backend:

* Two consecutive captures differ by 0 pixels.
* The same EBOOT with the RE-310 pack (full-tile texgen coverage, SHA-256
  `5179c3b3...`) differs from this golden by 52,232 pixels. The differences
  are small, spread across the body, and have no structural artifact.
* The golden was captured with pack SHA-256 `622d20e4...`.

This scene has no physical-PSP capture and no comparison against the
original N64 output.
