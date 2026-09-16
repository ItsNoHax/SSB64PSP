# Matrix refresh — fighter light and Ness face (RE-264)

Part of [docs/visual-regression/README.md](../README.md).

## Fighter light and Ness face regression refresh (RE-264)

`sceGuLight` configures a PSP light but does not enable its independently
gated channel. Enabling `GU_LIGHT0` restores the directional term to every
runtime fighter-light scope. Fox's focused scene now uses Sector Z's packed
source direction, while Dream Land's Mario and Link retain their existing
source contexts. Ness separately receives the same exact-key, mild face
reconstruction correction as Kirby.

The PPSSPP-software deltas against the immediately preceding committed
goldens are:

| Scene | Differing pixels | Current SHA-256 |
|---|---:|---|
| Dream Land | 128 | `4979febc58244f9ece07fbfdbb64da4cfa4150b7f0ebea444cc16d86ac6c72e8` |
| Saffron City | 12 | `de666a5c1b3e3e23dd46a03fbf227f3a5761f3299bb8cbac19b4ff7d3de7e71b` |
| Fox | 57,220 | `a3fb34835383411ba9b0ad38ce52e008ebb42b195ed3daaa5a5f4edf11ae1a66` |
| Ness | 8,800 | `5d81b418a8fdf60baf215d2783e540fe0d579ea8b8e5ed4efaab679b03d3604b` |
| Link | 20,368 | `1d5ff77266e193e7012ec4842af4da1305281d781d87c7bf701a743b2a6f0733` |

Each accepted golden compares at zero differing pixels to its corresponding
capture. Two independently built Fox and Ness captures are byte-identical.
Visual inspection confirms that Fox's upward-facing glove/boot surfaces are white
with shaded facets intact, Ness's eyes are smooth upright ovals, and the
Dream Land/Saffron City/Link changes are confined to fighter lighting. These
are PPSSPP results; current physical-PSP confirmation remains required by R2.
