# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** item material scripts, the Bob-omb's and Shells'
  root lists and spin, the Bumper's lit and attached lists and the
  item-destroy dust (RE-442). Pack rebuilt to v90.
- **Next gameplay batch:** the Poké Ball's rays and open animation and the
  Pokémon status materials/translucency, rock textures and FlySparks head-0
  state (RE-434, RE-435, TODO.md).
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Item scripts and root lists | Star/Ray Gun tables, Bob-omb walk list, Shell spin, Bumper lit/attached lists, destroy dust; N64-checked | RE-442 |
| Item presentation | Colour animations through `ENV_LERP` fog, pickup arrows, item light colours | RE-441 |

## Verification baseline

- All 1,755 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread).
- Clippy with warnings denied, workspace rustfmt, gameplay/ROM no_std
  and docs validation pass. Pack strict: 2,719 textures, 3,782 meshes,
  9,484 primitives; zero unresolved.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- 119 targeted game goldens match and repeat exactly; manifest 188 scenes.
  Three new controls and two rebaselines: RE-442. No full viewer matrix
  this batch; previous full baseline is RE-438.
- Production PPSSPPHeadless 20 s software smoke reaches its timeout
  without faults. This and the goldens are neither physical-PSP nor
  N64-equivalence proof.
- Pack v90: 35,800,000 bytes, SHA-256
  `09e43a12ab1a1a1d8d43ceb5b9247d958f86d5b066dac010526ce29595164598`.
- Production ELF measurements: RE-442; stack last measured RE-440.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Training menu sounds/BGM volume and magnifier sound dispatch await
  the audio backend. VS entry focus and CPU item behaviour have no N64
  runtime comparison yet (RE-437–440, TODO.md).
- Music, audio and rumble remain. Poké Ball rays/open animation, Pokémon
  materials/translucency, rock textures and FlySparks head-0 state remain
  (RE-435, RE-436, TODO.md). Lit item primitives without their own light
  colours keep the baked fallback shade (RE-441). The Bumper's lit palette
  and the Star's flicker are not yet observed on the N64 (RE-442).
- The N64 throws the Poké Ball right where the port throws it left
  (RE-434). Item damage/trajectory comparisons need event alignment.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- The 1P Game, bonus stages, pipe traversal/plant notification, selects'
  spotlight and no-save-data unlocks remain in TODO.md. Stage performance
  and VRAM residency belong to `P5`.
