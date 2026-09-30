# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** lighting from `G_LIGHTING` alone with a lit scene
  seed (76,191 N64 vertex loads match; the shape guess is gone), and
  `TEX_EDGE` cutouts drop alpha < 32 and blend edges (RE-424, pack v76).
- **Next gameplay batch:** Pikachu's hat and Jigglypuff's bow
  (`accesspart`), and the undrawn entry vehicles: Fox's Arwing, Captain
  Falcon's car, the Poké Ball.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Lighting, cutout edges | 76 goldens rebaselined (cutout edges, Kirby skeleton unlit) | RE-424 |
| Stage colours | 5 goldens added, 61 rebaselined (unlit stages, fruit strips, depth) | RE-423 |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass, and `ssb-game` checks as `no_std` (RE-424).
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 136 goldens match (RE-424); captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v76: 34,589,248 bytes, SHA-256
  `750508e7fc1f769e44950366430456437e25c425b56f7a69023bed8e22d3c01a`.
- `run` is 20,480 bytes (branch range 128 KB); largest stack frame
  `enter_training`, 159,456 bytes of the 256 KB main-thread stack (RE-424).
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Pikachu's hat and Jigglypuff's bow (`accesspart`) and both selects'
  spotlight are not drawn (RE-411, TODO). With no save data, Mushroom
  Kingdom and the four unlockable fighters stay locked.
- Yoshi's roll egg and egg explosion are not drawn (RE-415, RE-416).
- Sector Z Arwing, bonus stages and stage items remain.
- Yoshi's shield-drop egg-break particles are not made (RE-418).
- Kirby's motion-script head swaps
  (`ftParamSetModelPartID`) are not ported (RE-417).
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
