# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Poké Ball throw/open materials and rays, Pokémon
  status materials and link-18 placement, conditional translucent lists,
  Onix rock textures and corrected Scale-X head-1 CLD state (RE-443).
  Pack rebuilt to v91.
- **Next gameplay batch:** item object lifecycle and root ownership:
  same-pass spawned-item processes, remaining non-container item roots
  and light throw-turn joint yaw (TODO.md, RE-435–436).
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Poké Ball and Pokémon presentation | Materials, rays, link 18, callback modes, three rock textures; Scale-X state corrected from decomp | RE-443 |
| Item scripts and root lists | Star/Ray Gun tables, Bob-omb walk list, Shell spin, Bumper lit/attached lists, destroy dust | RE-442 |

## Verification baseline

- All 1,758 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread). Clippy with warnings denied, workspace rustfmt, portable
  no_std checks and docs validation pass.
- Pack strict: 2,740 textures, 3,788 meshes, 9,490 primitives;
  zero unresolved. Pack v91: 35,818,544 bytes, SHA-256
  `731f7593aec0e4cb8596f6f6756f3793efded012cdd517782ae5337ab0132e53`.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- 123 game goldens match and repeat exactly (122 together, Fall alone);
  manifest 192 scenes. Four new controls and six rebaselines: RE-443.
  No full viewer matrix this batch; previous full baseline is RE-438.
- Production PPSSPPHeadless 20 s software smoke exits 0 at its expected
  timeout without reported faults. Production has no screenshot hook.
  This and the goldens are not physical-PSP or N64-equivalence proof.
- Production ELF measurements: RE-443; stack last measured RE-440.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Training menu sounds/BGM volume and magnifier sound dispatch await
  the audio backend. Music, audio and rumble remain.
- VS entry focus, CPU item behaviour and item damage/trajectory checks
  need event-aligned original-game evidence (RE-434, RE-437–440).
  The N64 throws the Poké Ball right where the port throws it left.
- Item-root ownership, same-pass processes and throw-turn yaw remain.
  Lit primitives without authored light colours retain baked fallback
  shade; the Bumper lit palette and Star flicker lack N64 observations.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- 1P Game, bonus stages, pipe traversal/plant notification, selects'
  spotlight and no-save-data unlocks remain in TODO.md. Stage performance
  and VRAM residency belong to `P5`.
