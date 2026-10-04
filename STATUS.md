# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** live item process passes, promoted descriptor-1
  roots, Link Bomb body/fuse transforms and held bloat, and throw-turn
  joint yaw (RE-444). Pack remains v91.
- **Next gameplay batch:** thrown-fighter attacks and ownership:
  `SetDamageThrown`, the throw pointer and `ftCommonThrownProcStatus`
  (TODO.md).
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Item lifecycle and root ownership | Same-pass spawns, promoted roots, Bomb fuse/bloat and throw-turn joint transforms | RE-444 |
| Poké Ball and Pokémon presentation | Materials, rays, link 18, callback modes, three rock textures; Scale-X state corrected | RE-443 |

## Verification baseline

- All 1,764 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread); final gameplay crate rerun: 972 pass. Clippy with warnings
  denied, workspace rustfmt, portable no_std and docs validation pass.
- Pack v91: 35,818,544 bytes, SHA-256
  `731f7593aec0e4cb8596f6f6756f3793efded012cdd517782ae5337ab0132e53`.
  No asset-pipeline change this batch.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- 33 targeted game goldens match and repeat exactly; three new Bomb
  controls and one 72-pixel fuse rebaseline (RE-444). Manifest: 195 scenes.
  Larger game baseline: RE-443; last full viewer matrix: RE-438.
- Production PPSSPPHeadless 20 s software smoke exits 0 at its expected
  timeout without reported faults. Production has no screenshot hook.
  This and the goldens are not physical-PSP or N64-equivalence proof.
- Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Training menu sounds/BGM volume and magnifier sound dispatch await
  the audio backend. Music, audio and rumble remain.
- VS entry focus, CPU item behaviour and item damage/trajectory checks
  need event-aligned original-game evidence (RE-434, RE-437–440).
  The N64 throws the Poké Ball right where the port throws it left.
- Lit primitives without authored light colours retain baked fallback
  shade; Bumper's lit palette, Star flicker and the Bombs' critical
  flashes lack N64 observations.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- Thrown-fighter hits, 1P Game, bonus stages, pipe traversal/plant
  notification, selects' spotlight and no-save-data unlocks remain in
  TODO.md. Stage performance and VRAM residency belong to `P5`.
