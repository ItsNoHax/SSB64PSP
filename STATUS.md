# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** accessories (`accesspart`), motion-script
  model-part swaps and the entry vehicles (Arwing, Blue Falcon, Poké
  Ball), matched to N64 captures (RE-425, pack v77).
- **Next gameplay batch:** texture-part swaps (`ftParamSetTexturePartID`:
  eyes, mouths), the selects' and results' demo scripts, and low detail
  in three- and four-fighter VS.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Accessories, parts, vehicles | 7 goldens added, 8 rebaselined (part swaps, Samus's unmade parts) | RE-425 |
| Lighting, cutout edges | 76 goldens rebaselined (cutout edges, Kirby skeleton unlit) | RE-424 |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass, and `ssb-game` checks as `no_std` (RE-425).
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 143 goldens match (RE-425); captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v77: 34,880,272 bytes, SHA-256
  `b6d6b19d183d875579821b61aa34f59231c0096c2afd1316577d1ec10e4020ef`.
- `run` is 21,764 bytes (branch range 128 KB); largest stack frame
  `enter_training`, 159,552 bytes of the 256 KB main-thread stack (RE-425).
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Both selects' spotlight is not drawn (RE-411, TODO). With no save data, Mushroom
  Kingdom and the four unlockable fighters stay locked.
- Yoshi's roll egg and egg explosion are not drawn (RE-415, RE-416).
- Sector Z Arwing, bonus stages and stage items remain.
- Yoshi's shield-drop egg-break particles are not made (RE-418).
- Kirby's motion-script head swaps
  (`ftParamSetModelPartID`) are not ported (RE-417).
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
