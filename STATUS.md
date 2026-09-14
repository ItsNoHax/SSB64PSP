# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Primary task: complete the remaining physical-hardware matrix.
- Status: `IN_PROGRESS`
- Last complete: `RE-269 — RDP tile origin is not a source-image offset`.
  Donkey Kong's tie, hands, feet, ears, and head now use the ROM's padded CI4
  source rows without double-applying the tile origin in the PPSSPP software
  golden.
- Current build: RE-269 code with the existing v29 pack and 22 deterministic
  goldens; the formal R2 physical-hardware matrix remains primary.

## RE-264 result

Fox's materials were already correct in the ROM: white directional light over
gray ambient. The PSP configured light 0 but never enabled its independent GE
channel, so only the gray ambient term reached fighters. The runtime now pairs
`GU_LIGHT0` with the existing global lighting scope, and the Fox capture uses
Sector Z's source angle from file 262. Fox's gloves and boots render white while
retaining directional shading; Dream Land's Mario and Link lighting were
refreshed for the same state correction.

Ness's default file-335 texture at `0xB7A0` contains the same single-texel eye
steps as Kirby. It now receives the same mild, exact-file-and-offset
reconstruction correction and renders two smooth upright eyes.

- Goldens: Dream Land `4979febc5824...`, Saffron City `de666a5c1b3e...`, Fox
  `a3fb34835383...`, Ness `5d81b418a8fd...`, Link `1d5ff77266e1...`; all
  compare exactly to their accepted PPSSPP-software captures.
- Pack: 26,288,016 bytes,
  `495a4bcd52f0cfe720d5007a0e9d45b8453e2fb09cb89283993926713a7568db`
- Normal EBOOT: 5,088,016 bytes,
  `e8a058b9bb56f54f0773adefde37c37e3f60251cdf9ff2c776ad4f308a5ccf20`
- Physical PSP confirmation of this corrected light state and Ness texture is
  still required by R2.

## RE-262 result

Fox and Link did not have corrupt eye textures. Their face primitives use
negative signed S10.5 U coordinates on a clamped axis, while the PSP GE reads
`GU_TEXTURE_16BIT` as unsigned. The bit patterns therefore became large
positive coordinates and held the wrong edge texel over one eye.

- Pack v29 marks only ordinary authored-UV primitives with a negative
  coordinate on a clamped axis. Those indexed corners expand transiently to
  exact float UVs; repeat and texgen paths are unchanged.
- Fox and Link both render two eyes. Their independent duplicate captures are
  byte-identical; current hashes are `3196cc914976...` and
  `b2a6763d4670...` respectively.
- All 16 deterministic scenes pass. Eleven textured scenes changed by visually
  reviewed, source-coherent amounts; flat colour, three pure texgen controls,
  and the depth-state diagnostic remain byte-identical.
- Pack: 26,254,608 bytes,
  `119b856a7e827eaa53436295448132d09849b384eec02c4fb09e51f0e3933384`
- Normal EBOOT: 5,087,556 bytes,
  `1d9d0b3b481683b4701c80c7a397da79c94d07b36fee2a7dc61fbe1b8ae3d113`

## Integrated verification

- `cargo fmt --all --check`: pass
- `cargo test --workspace`: 616 passed
- all 22 deterministic goldens: exact after the explained RE-262–265 refreshes
- effects: 46/46 manager objects, 35/35 transform animations, 24/26 material
  animations (two documented source-unreachable rest-invisible cases), all
  160 particle scripts
- billboards: 109 inventoried, 0 structural anomalies
- framebuffer transitions: 11/11 replayed and object-bound
- `romtool texgen --verify`: pass
- plain feature-free `cargo psp --release`: pass
- strict workspace Clippy: not clean under the current toolchain because of
  three unrelated pre-existing `needless_range_loop` lints in `coord.rs`,
  `matanim.rs` and `objanim.rs`; not expanded into this fix

## Hardware state

RE-260 physically proves `MEMSIZE=1` loads the same-sized v28 full pack on PSP
Slim/6.61; v29 and its float-UV draw path have PPSSPP evidence only so far.
The reported motion stop came from the staged `regression_capture` EBOOT's
intentional tick-240 freeze; the process and renderer stayed alive, and an
independent 600-frame Mario audit replayed all 20 movement slots exactly.
A feature-free normal EBOOT is built, but sustained motion on that exact build
has not yet been reported and is not claimed.

The formal R2 matrix still needs:

- PSP-1000 coverage. Its 32 MiB RAM cannot use `MEMSIZE=1`, so current pack
  compatibility is unresolved rather than assumed.
- a long-duration feature-free physical run
- broader/exhaustive hardware coverage sufficient to close “no hardware-only
  rendering failures remain”
- physical confirmation of the synthetic depth-mask diagnostic
- physical re-capture of representative RE-262 signed-clamp scenes and RE-264
  lighting/face changes (at least Fox, Ness, Link, and one affected stage)

These physical requirements keep R3 and combat blocked. There is no remaining
R2.2 renderer-model blocker.

## Non-blocking follow-ups

- RE-263/264 resolve Kirby's and Ness's false inward eye spikes as PSP
  reconstruction artifacts, not mirror-state errors. File-and-offset-scoped
  mild filters preserve the ROM faces while restoring the original render's
  oval eyes; the Kirby and Ness captures have refreshed PPSSPP-software goldens. Physical
  PSP confirmation remains part of R2.
- T1's 164 cross-node differing-transform vertex reuses remain measured.
- N64 three-point filtering vs PSP bilinear remains an accepted fixed-function
  deviation (RE-219).
- Full runtime fighter costume selection is future work; costume zero now
  resolves palette and all five colour/light tracks.
- The installed global `cargo-psp` remains the previously documented hybrid
  build; reconciling the user's newer rust-psp branch with this repository's
  pinned nightly is separate toolchain work.

## Continuation

Read `AGENTS.md`, this file, the R2 acceptance section in `PLAN.md`, and
the affected subsystem/evidence rows. Inspect Git state. Resume the physical
R2 matrix if the required hardware is available; otherwise record the concrete
access/PSP-1000 memory blocker without starting R3 or combat.
