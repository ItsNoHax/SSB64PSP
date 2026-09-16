# Scenes 6-7 — Metal texgen and rotated (RE-214)

Part of [docs/visual-regression/README.md](../README.md).

## Sixth and seventh deterministic test scenes (RE-214)

`regression_capture_scene6` selects `StageMetalFile2`'s first graph (file
117, offset `0x1B10`, two nodes) in the same object-viewer pattern as scenes
2–4. It is the only content this port can currently show that draws
real `G_TEXTURE_GEN` material state.

`regression_capture_scene7` is the **same** graph under the **same** camera,
frozen a quarter turn further round. It exists because one frozen reflection
is not evidence: a correct coordinate generator, a stuck basis and a constant
both produce a single plausible frame. Two deterministic captures at two known
rotations distinguish them — they differ by RMSE 0.058 here — and a future
regression that silently freezes the generator fails the pair even though it
would pass either frame alone.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene6
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-metal-texgen.png ~/ppsspp-test/screenshot.png

cd psp && cargo psp --release --features regression_capture_scene7
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-metal-texgen-rotated.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-214): both scenes `ldstart`ed under
PSPLink with `exlist` empty and `main_thread` alive. Diffed 2x-upscaled
against their goldens: 25,977 and 28,866 differing pixels, of which 2,652 and
8,443 differ by more than 6%. A reflection has fine internal gradients, so its
band boundaries move under any sub-texel difference in coordinate
interpolation — the right comparison is against this harness's own baseline,
and the *non*-texgen Dream Land scene diffs at 61,362 / 11,668 by the same
measurement. These scenes agree with PPSSPP better than the long-accepted
baseline does. The two hardware captures differ from each other by 57,076
pixels, so the reflection responds to model rotation on real hardware too.

These goldens do **not** establish agreement with the original N64 output;
see RE-214 §10 for why no such comparison exists yet.

RE-235 (`PLAN.md` R2.1/T8) refreshed `r2-metal-texgen{,-rotated}.png` after
`R2.1`/T7a (RE-232) changed this exact content's addressing; the diff numbers
above are against the pre-T7a golden and are historical, not current. RE-236
re-captured both scenes on physical PSP against the refreshed goldens
(36,607 / 27,076 differing pixels 2x-upscaled — the same noise-floor order
as RE-214's own baseline) and closed `R2.1`/T8.
