# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the results' 3D emblem and confetti, the KO halo's
  rays and quake, and the far plane 39,936 (RE-420, pack v72), each
  compared with N64 captures. The near plane stays 1 (deviation, RE-420).
- **Next gameplay batch:** find how the N64 draws the shield bubble over
  its fighter (RDRAM joint depths), so the near plane can be 256.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Results emblem, KO pieces | 2 goldens added, 13 rebaselined (swirl texture, rays, quake, emblem, far plane) | RE-420 |
| Training character select | 2 goldens added, none rebaselined | commit |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass, and `ssb-game` checks as `no_std` (RE-420).
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 127 goldens match (RE-420); captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v72: 34,455,072 bytes, SHA-256
  `527e07c595bc22503683d60997f1342f482ee51a63b3ffddbf59a442aafd3d6b`.
- `run` is 20,476 bytes (branch range 128 KB); largest stack frame
  `enter_training`, 159,456 bytes of the 256 KB main-thread stack (RE-420).
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Pikachu's hat and Jigglypuff's bow (`accesspart`) are not drawn; the
  selects' spotlight is not drawn. Yoshi's Island's stage draw differs from the N64 (grey platforms, heart) (TODO). The VS select's spotlight is not drawn (RE-411). With no save data, Mushroom
  Kingdom and the four unlockable fighters stay locked.
- Yoshi's roll egg and egg explosion are not drawn (RE-415, RE-416).
- Sector Z Arwing, bonus stages and stage items remain.
- Yoshi's shield-drop egg-break particles are not made (RE-418).
- Kirby's motion-script head swaps
  (`ftParamSetModelPartID`) are not ported (RE-417).
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
