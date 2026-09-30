# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the grey damage shield, Yoshi's egg shield and the
  Fireball's spin (RE-418, pack v69). The egg's `(SHADE - ENV) * TEXEL0`
  combiner is a new pack flag; the spin is camera-relative (kind 0x47).
- **Next gameplay batch:** stage wallpapers (`grWallpaperMakeDecideKind`)
  and the Training and stage selects' presentation.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Shields, Fireball spin | Drawn; 2 goldens added, 2 rebaselined (Fireball spin) | RE-418 |
| Thunder, copy hats, Egg Lay egg | Drawn; 3 goldens added, none rebaselined | RE-417 |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass, and `ssb-game` checks as `no_std` (RE-418).
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 119 goldens match (RE-418); captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v69: 30,710,256 bytes, SHA-256
  `a768f845423d7ba18fa5583bf72286bb3ab47cfcf8f1801f410d320ae1ed6f12`.
- `run` is 20,740 bytes (branch range 128 KB); largest stack frame
  `enter_training`, 159,456 bytes of the 256 KB main-thread stack (RE-418).
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the Training and stage selects draw plain
  slots (TODO). The results' emblem and confetti and the VS select's
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
