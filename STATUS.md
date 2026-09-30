# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the Training character select's presentation
  (`mnPlayers1PTraining`, pack v71), matched to two N64 warp-boot
  captures. No RE record (routine port); evidence in the commit message.
- **Next gameplay batch:** the results' emblem and confetti, and the KO
  halo's rays, quake and star-KO far-plane clip (RE-410, RE-412).
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Training character select | 2 goldens added, none rebaselined | commit |
| Wallpapers, stage select | 4 goldens added, 46 rebaselined (wallpaper behind every battle) | RE-419 |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass, and `ssb-game` checks as `no_std` (RE-419).
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 125 goldens match; captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v71: 34,423,136 bytes, SHA-256
  `daefb176e12fadc8ce82b7071844c8c08bf6a99277d2f6c1cba6bd4ca18bb1e6`.
- `run` is 20,464 bytes (branch range 128 KB); largest stack frame
  `enter_training`, 159,456 bytes of the 256 KB main-thread stack (RE-419).
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Pikachu's hat and Jigglypuff's bow (`accesspart`) are not drawn; the
  selects' spotlight is not drawn. Yoshi's Island's stage draw differs from the N64 (grey platforms, heart) (TODO). The results' emblem and confetti and the VS select's
  spotlight are not drawn (RE-410, RE-411). With no save data, Mushroom
  Kingdom and the four unlockable fighters stay locked.
- KOs: the halo's rays are hidden and no quake is made; a star KO is
  clipped past the 10,000-unit far plane (RE-412). Yoshi's roll egg and
  egg explosion are not drawn (RE-415, RE-416).
- Sector Z Arwing, bonus stages and stage items remain.
- Yoshi's shield-drop egg-break particles are not made (RE-418).
- Kirby's motion-script head swaps
  (`ftParamSetModelPartID`) are not ported (RE-417).
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
