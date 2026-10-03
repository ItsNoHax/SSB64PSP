# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** battle tags, off-screen arrows, circular magnifiers
  and Training View integration (RE-440). Pack rebuilt to v88.
- **Next gameplay batch:** item colour animations and pickup arrows
  (`itMainRunUpdateColAnim`, `ifCommonItemArrow*`).
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Battle player interface | Tags, animated arrows, circular miniatures and Training View; source battle viewport/live FOV | RE-440 |
| Training menu presentation | Panel, options, stats and combo counting | RE-439 |

## Verification baseline

- All 1,745 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread).
- Clippy with warnings denied, workspace rustfmt, gameplay/ROM no_std
  and docs validation pass. Pack strict: 2,702 textures, 3,778 meshes,
  9,480 primitives; zero unresolved.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- 115 targeted game goldens match and repeat exactly; manifest 184 scenes.
  Five new interface controls and 101 world rebaselines: RE-440. No full
  viewer matrix this batch; previous full baseline is RE-438.
- Production PPSSPPHeadless 20 s software smoke reaches its timeout
  without faults; production requests no screenshot. This and the
  goldens are neither physical-PSP nor N64-equivalence proof.
- Pack v88: 35,785,808 bytes, SHA-256
  `79294fc8fbbbff630df8a8779043877bd4ba9efd1bc59f7ceecf9175cc0cd9ca`.
- Production stack/ELF measurements: RE-440. Physical PSP last checked
  RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Training menu sounds/BGM volume and magnifier sound dispatch await
  the audio backend. VS entry focus and CPU item behaviour have no N64
  runtime comparison yet (RE-437–440, TODO.md).
- Item colour animations, pickup arrows, music, audio and rumble remain.
  Poké Ball rays/open animation, Pokémon materials/translucency, rock
  textures and FlySparks head-0 state remain (RE-435, RE-436, TODO.md).
- The N64 throws the Poké Ball right where the port throws it left
  (RE-434). Item damage/trajectory comparisons need event alignment.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- The 1P Game, bonus stages, pipe traversal/plant notification, selects'
  spotlight and no-save-data unlocks remain in TODO.md. Stage performance
  and VRAM residency belong to `P5`.
