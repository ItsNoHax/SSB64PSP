# Scene 16 — Link costume-light regression (RE-258/RE-261)

Part of [docs/visual-regression/README.md](../README.md).

## Sixteenth scene: Link costume-light regression (RE-258/RE-261)

`regression_capture_link` selects Link's file 324 graph `0x3AE8` and uses
the established object-view freeze/HUD suppression. Unlike the generic fighter
viewer scenes, it also configures Dream Land's source-derived fighter light:
Link's grayscale tunic textures are intentionally tinted by material
`LIGHT1COLOR`/`LIGHT2COLOR`, so a light-free viewer cannot validate his
costume. RE-261 fixed the converter dropping those two already-decoded costume
tracks; the exact decompilation values are pinned by a unit test.

Capture and compare with:

```
tools/run-ppsspp-headless.sh --feature regression_capture_link
tools/compare-screenshot.sh tests/golden/r2-link-fighter.png ~/ppsspp-headless-test/screenshot.png
```

Two post-fix captures differ by 0 pixels. The incorrect-blue to canonical-
green correction changes 7,492 pixels. The accepted golden is
`tests/golden/r2-link-fighter.png`, SHA-256
`b2a6763d4670475df115b396773fe3c2a9a7858ee904be9ed223636445124b54`
after RE-262's later signed-clamp correction restored the second eye. RE-264
supersedes it with hash `1d5ff77266e193e7012ec4842af4da1305281d781d87c7bf701a743b2a6f0733`
after enabling the directional light channel that this scene was already
configured to use.
