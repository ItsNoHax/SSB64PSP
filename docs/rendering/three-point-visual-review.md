# N64 3-point filtering: visual review of section 11 candidates

Decisions for the rows that
[three-point-residuals.md](three-point-residuals.md) section 11 triaged as
`MANUAL_FIX_RECOMMENDED` or `VISUAL_REVIEW_REQUIRED` (pack v31, report at
pack SHA-256 `97b3f4d8…5707f216`). Evidence: RE-312.

## Method

- Each candidate is the fix class's own cross-phase fit
  (`residuals::cross_phase_fit`): fitted on the site's phase-uniform set U1,
  GE-exact integer refinement, shipped as an immutable RGBA8888 variant at
  the exact use sites only (`residual_fixes::Kind::Direct`). Review
  candidates live in `residual_fixes::REVIEW_CASES` and are packed only with
  `romtool pack --residual-candidates v<N>`.
- A = shipped pack. B = shipped pack plus that one candidate, nothing else.
  Every B pack differs from A in exactly the candidate's texture (checked
  per primitive).
- Captures: PPSSPPHeadless software GE, frozen deterministic tick, native
  480×272. Scenes: the candidate's whole stage
  (`regression_capture_stage_index`) and its source graph
  (`regression_capture_object`, `SSB64_CAPTURE_OBJECT=<file>:<graph>`).
- Metrics: `tools/residual-ab-metrics.py` over the full frame. Silhouette
  changes (cutout sites) use a third pack whose candidate texels have
  alpha 0 (`--residual-hide-probe`): a pixel belongs to the site when it
  differs from that probe frame.
- Reproduce: `tools/residual-ab-capture.sh`. Images (A, B, ×8 amplified
  difference, nearest-neighbour enlarged crop) go to
  `assets/generated/three-point-visual-review/` (ROM-derived, not committed).
- No N64 reference frame exists for these scenes: the asset viewer's camera
  and scene state are not the original game's, and no host path produces a
  3-point frame. The reference distance is therefore the cross-phase U2
  result only. No candidate can show a frame-level move toward the N64
  result, which the `DEPLOY` bar requires.

## Results

Frame metrics are for the scene with the most changed pixels. Pixel
thresholds are on the maximum channel difference.

| Variant | Use | Proposed fix | Cross-phase gain | PSP-frame result | Decision | Memory delta (level 0 / pack) |
|---|---|---|---:|---|---|---:|
| v766 `120:0x418` | Board the Platforms (Mario) `137:0x28C8#1`, `0x28D0#1` | `DIRECT_RGBA_OVERRIDE` (silhouette fit) | 47.4% | 75–77 px, max 8, 0 px ≥ 16; 0 silhouette px (site 108–114 px); 0 px outside the site | **DEPLOYED** (recommended row) | +1,728 / +1,728 B |
| v397 `107:0x50` | Mushroom Kingdom `107:0x6A30#2`, `#4` | `USE_SITE_TEXTURE_VARIANT` | 34.6% | 8,211 px, max 34, mean 1.6; 7 px ≥ 16, 1 ≥ 32; dither-level speckle | ACCEPT_CURRENT | +982,976 / +1,309,920 B |
| v1004 `157:0x6C0` | Zebes map object `157:0x9D8#0` | `DIRECT_RGBA_OVERRIDE` | 34.8% | 0 px in stage and graph views; the site draws no pixel in either | ACCEPT_CURRENT | +917,440 / +1,223,360 B |
| v404 `108:0x6D40` | Kongo Jungle `108:0x7E90#7` | `DIRECT_RGBA_OVERRIDE` | 26.8% | 293 px, max 12; 0 px ≥ 16 | ACCEPT_CURRENT | +57,280 / +76,096 B |
| v252 `86:0x4C18` | Starman item `86:0x5450#0`, `0x5458#0` | `DIRECT_RGBA_OVERRIDE` (silhouette fit) | 39.6% | 13,238 px, max 223; 214 silhouette px (212 gained, 2 lost); contour grows by one pixel and gains an isolated notch | ACCEPT_CURRENT | +11,264 / +15,360 B |
| v837 `121:0x30` | Board the Platforms (Samus) `140:0x10F0#3` | `DIRECT_RGBA_OVERRIDE` | 43.7% | 9,910 px, max 53; 275 px ≥ 16, 16 ≥ 32; texel-scale speckle across the wall | ACCEPT_CURRENT | +917,440 / +1,223,232 B |
| v799 `121:0x30` | Board the Platforms (Fox) `138:0x1DB8#9` | `DIRECT_RGBA_OVERRIDE` | 42.2% | 3,803 px, max 174; 1,009 px ≥ 32: two other primitives turn magenta | ACCEPT_CURRENT (new artifact) | +917,440 / +1,223,104 B |
| v800 `121:0x30` | Board the Platforms (Fox) `138:0x1DB8#10` | `DIRECT_RGBA_OVERRIDE` | 37.0% | 3,064 px, max 187; 901 px ≥ 32: the same magenta primitives | ACCEPT_CURRENT (new artifact) | +917,440 / +1,223,360 B |
| v902 `121:0x30` | Board the Platforms (Falcon) `144:0x34D8#33`, `0x34E0#33` | `DIRECT_RGBA_OVERRIDE` | 27.1% | 183 px, max 91; 23 px ≥ 32 on another primitive, identical to v901's | ACCEPT_CURRENT | +1,834,944 / +2,446,528 B |
| v901 `121:0x30` | Board the Platforms (Falcon) `144:0x34D8#32`, `0x34E0#32` | `DIRECT_RGBA_OVERRIDE` | 27.2% | 1,067 px, max 91; the same 23 px ≥ 32 as v902 | ACCEPT_CURRENT | +229,312 / +305,472 B |
| v609 `117:0x7A8` | Meta Crystal `117:0x1708#1` | `DIRECT_RGBA_OVERRIDE` | 27.6% | 871 px, max 13; 0 px ≥ 16 | ACCEPT_CURRENT | +458,688 / +611,520 B |
| v599 `116:0x21C8` | Battlefield `116:0x3AB0#6` | `DIRECT_RGBA_OVERRIDE` | 26.5% | stage 0 px; graph 155 px, max 7 | ACCEPT_CURRENT | +917,440 / +1,223,360 B |
| v76 `52:0x24228` | Opening movie `52:0x24660#0` | `DIRECT_RGBA_OVERRIDE` (silhouette fit) | 36.4% | not capturable: no pack object graph draws mesh `52:0x24660` | ACCEPT_CURRENT | +2,048 / +3,072 B |

Totals: 1 deployed, 12 `ACCEPT_CURRENT`, 0 `NEEDS_PHYSICAL_PSP_CHECK`.
Shipped memory delta: +1,728 B level 0, +1,728 B pack.

## Findings

- **Texture-space gain does not reach the frame.** Every stage and graph
  view shows these textures minified or at about one texel per pixel. At
  that scale the GE's bilinear and the RDP's 3-point filter nearly agree, so
  a 27–47% U2 gain becomes a few hundred mostly sub-8/255 pixels.
- **RGBA8888 in these meshes leaks into other primitives.** Binding either
  v799 or v800 turns the same two thin primitives of mesh `138:0x1DB8`
  magenta (494 shared pixels ≥ 32). Binding either v901 or v902 changes the
  same 23 pixels of another primitive in stage 35. An RGBA8888 texture with
  the source texels, unfitted, reproduces the v799 magenta, so the fit is
  not the cause. v766's changed pixels all fall inside its own site. The
  cause is not yet known; see `TODO.md`.
- **48 packed textures exceed the GE's 512-texel limit** after power-of-two
  padding, including `121:0x30`'s 576-texel variants (v902's texture is
  304×576). This predates RE-312; see `TODO.md`.
