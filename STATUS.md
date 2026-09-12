# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Primary task: complete the remaining physical-hardware matrix.
- Status: `IN_PROGRESS`
- Last complete: `R2.2 — Second Renderer Corrective Gate (C1–C7)`
  (RE-240–261). C1–C5 corrected primitive-colour ownership, load-time
  lighting provenance, independent depth state, submission order, texture
  cache identity and PSP GE cache isolation. C6/C7 now pass and are reconciled.
- Current build: RE-261's focused costume-light/gate-closure commit is at
  `HEAD`, directly above the user's scene-15 commit `d524114`.

## RE-261 result

Link's white/blue tunic was not an animation fault. File 324's costume scripts
drive `LIGHT1COLOR` and `LIGHT2COLOR`; the decoder read those tracks but its
public result discarded them, leaving the baked last-costume blue. Both tracks
now reach the packed material. Scene 15 uses Dream Land's source-derived
fighter light and renders the authored costume-zero green.

- New golden: `tests/golden/r2-link-fighter.png`
  (`37f14f3cb2d6ba2cf2fca2ff2fd16926aa8483b62a052208971e689ec795e5ed`)
- Determinism: two Link captures, 0 differing pixels
- Semantic Link correction: 7,492 pixels
- Existing matrix: 14/15 exact; Dream Land's 24 stable costume-light pixels
  were source-explained and refreshed
- Pack: 26,254,608 bytes,
  `d992dbe734f63b7cb97e36d68b56ea703d6beab25ff68b53bec556f03b3474c6`
- Normal EBOOT: 5,078,344 bytes,
  `57d27419c57610dc82b491b0780e36a7bf0c4acd73b410a3e547ba85894fab5c`

## Integrated verification

- `cargo fmt --all --check`: pass
- `cargo test --workspace`: 613 passed
- all 16 deterministic goldens: pass after the explained refresh
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

RE-260 physically proves `MEMSIZE=1` loads the full pack on PSP Slim/6.61.
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

These physical requirements keep R3 and combat blocked. There is no remaining
R2.2 renderer-model blocker.

## Non-blocking follow-ups

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
