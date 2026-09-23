# All-playable-fighter neutral regressions (RE-265)

Part of [docs/visual-regression/README.md](../README.md).

## All-playable-fighter neutral regressions (RE-265)

The original game's fighter display path does more than submit a model graph:
it installs the high-detail `FTCommonPart`, applies the looping `Wait`
figatree pose, rebuilds the active stage's directional light, and then draws.
The old object-view goldens covered only six isolated fighters, and four of
those omitted the runtime light scope. A raw graph also leaves Link in his
bind-pose T-pose, which is not an in-game presentation.

Every playable character now has an independently buildable capture feature
and committed `r2-*-fighter.png` golden. Each scene selects the decomp/ROM
high-detail model graph, starts that fighter's `Wait` animation from frame
zero, freezes it at deterministic tick 240, and frames the posed hierarchy
with the existing fixed 38-degree fitted camera. Dream Land supplies the
shared source stage light; Fox retains Sector Z because that is the supplied
original-game reference context. The PSP `GU_LIGHT0` scope and each model's
authored material light colours are therefore exercised for all twelve.

The fighter features are named for their subject (for example,
`regression_capture_fox` and `regression_capture_link`). The remaining
numbered non-fighter scenes are contiguous: `regression_capture` is scene 1,
followed by `regression_capture_scene2` through `_scene9`.

Run the exact-pixel gate with:

```
tools/verify-fighter-goldens.sh
```

It rebuilds, captures, and compares Mario, Fox, Donkey Kong, Samus, Luigi,
Link, Yoshi, Captain Falcon, Kirby, Pikachu, Jigglypuff, and Ness. The twelve
2026-09-14 PPSSPP-software captures compare at zero differing pixels; their
individual SHA-256 values are the committed image hashes. This is an expanded
software regression suite, not a claim of new physical-PSP coverage.

RE-311 adds Metal Mario (`regression_capture_metal_mario`,
`r2-metal-mario-fighter.png`). It is the first fighter golden that draws
`G_TEXTURE_GEN`. See [fighter-metal-mario.md](fighter-metal-mario.md).

2026-09-24 rebaseline (after RE-309, RE-310 and RE-311): 11 goldens changed.
Differing pixels against the RE-307 goldens: Mario 1,344, Fox 644, Samus
2,476, Luigi 1,552, Link and Link costume 1 12 each, Captain Falcon 6,156,
Kirby 324, Pikachu 340, Jigglypuff 1,532 and Ness 8,012. Donkey Kong and
Yoshi did not change. For Mario and Ness, the RE-310 and RE-311 packs give
identical captures. Captain Falcon's old golden had red and beige glove-cuff
stripes that are not in the source texture `332:0xBFF8`. The new capture
shows only the source brown, olive and yellow. Ness changes only in sub-texel
filtering on the shirt. The new captures were taken with pack SHA-256
`622d20e4...`.
