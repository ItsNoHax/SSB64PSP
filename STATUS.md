# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** drawing Pikachu's Thunder, Kirby's copy hats and
  the Egg Lay egg in `psp-game` (RE-417, pack v68). Thunder segments roll
  a random bolt frame each update and end on frame 3 turned 180°.
- **Next gameplay batch:** drawing the grey damage shield and Yoshi's egg
  shield, and the Fireball's spin, in `psp-game` (RE-384, RE-372).
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Thunder, copy hats, Egg Lay egg | Drawn; 3 goldens added, none rebaselined | RE-417 |
| Weapon effects, clashes | `wpeffect`, clash search, quake shake; 4 goldens rebaselined (shake, random draws) | RE-416 |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass, and `ssb-game` checks as `no_std` (RE-417).
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 117 goldens match (RE-417); captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v68: 30,710,256 bytes, SHA-256
  `a4e979efa307e0d09f6501ddbacbd954ef9110e4f29abaf93604e86430542d57`.
- `run` is 20,740 bytes (branch range 128 KB); largest stack frame
  `enter_training`, 159,456 bytes of the 256 KB main-thread stack (RE-417).
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
- The grey damage shield and Yoshi's egg shield are not drawn (RE-384).
- The Fireball does not spin (RE-372). Kirby's motion-script head swaps
  (`ftParamSetModelPartID`) are not ported (RE-417).
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
