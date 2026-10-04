# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** item colour animations, pickup arrows and authored
  item lighting (RE-441). Pack rebuilt to v89.
- **Next gameplay batch:** remaining item presentation: the Bob-omb's walk
  display lists, the Star's material animation, the item-destroy dust, the
  Bumper's lit palette and flat model, the Shells' spin and material frames
  (RE-434, RE-436, TODO.md).
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Item presentation | Bob-omb/Link Bomb/Hammer colour animations through `ENV_LERP` fog, pickup arrows, item light colours; checked against N64 scenes | RE-441 |
| Battle player interface | Tags, animated arrows, circular miniatures and Training View; source battle viewport/live FOV | RE-440 |

## Verification baseline

- All 1,748 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread).
- Clippy with warnings denied, workspace rustfmt, gameplay/ROM no_std
  and docs validation pass. Pack strict: 2,703 textures, 3,778 meshes,
  9,480 primitives; zero unresolved.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- 116 targeted game goldens match and repeat exactly; manifest 185 scenes.
  One new control and 15 rebaselines: RE-441. No full viewer matrix this
  batch; previous full baseline is RE-438.
- Production PPSSPPHeadless 20 s software smoke reaches its timeout
  without faults; production requests no screenshot. This and the
  goldens are neither physical-PSP nor N64-equivalence proof.
- Pack v89: 35,786,400 bytes, SHA-256
  `4946f9ce9554780b9c2723414f196f20cf4a3197055795eaec27996d80208416`.
- Production ELF measurements: RE-441; stack last measured RE-440.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Training menu sounds/BGM volume and magnifier sound dispatch await
  the audio backend. VS entry focus and CPU item behaviour have no N64
  runtime comparison yet (RE-437–440, TODO.md).
- Music, audio and rumble remain. Poké Ball rays/open animation, Pokémon
  materials/translucency, rock textures and FlySparks head-0 state remain
  (RE-435, RE-436, TODO.md). Lit item primitives without their own light
  colours keep the baked fallback shade (RE-441).
- The N64 throws the Poké Ball right where the port throws it left
  (RE-434). Item damage/trajectory comparisons need event alignment.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- The 1P Game, bonus stages, pipe traversal/plant notification, selects'
  spotlight and no-save-data unlocks remain in TODO.md. Stage performance
  and VRAM residency belong to `P5`.
