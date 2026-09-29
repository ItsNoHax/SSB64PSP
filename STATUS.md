# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the weapons' own effects and clashes (RE-416):
  `wpProcessProcSearchHitWeapon` with its priorities, records and
  `proc_setoff`s, every ported weapon's dust, sparkles, shocks and glows in
  link order, and the quake's camera shake (pack v67).
- **Next gameplay batch:** drawing Pikachu's Thunder with its fading
  segments, Kirby's copy hats and the Egg Lay egg in `psp-game`.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Weapon effects, clashes | `wpeffect`, clash search, quake shake; 4 goldens rebaselined (shake, random draws) | RE-416 |
| Fighter effects | `fteffect`, display effects; 27 goldens rebaselined (effects, random draws) | RE-415 |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass, and `ssb-game` checks as `no_std` (RE-416).
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 114 goldens match (RE-416); captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v67: 30,491,216 bytes, SHA-256
  `954ce58095a7eb524c79cb9ad11919a35ed7883861a23e6bd0d1a4e73082ab37`.
- `run` is 18,480 bytes (branch range 128 KB); largest stack frame
  `enter_training`, 159,432 bytes of the 256 KB main-thread stack (RE-416).
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the Training and stage selects draw plain
  slots (TODO). The results' emblem and confetti and the VS select's
  spotlight are not drawn (RE-410, RE-411). With no save data, Mushroom
  Kingdom and the four unlockable fighters stay locked.
- KOs: the halo's rays are hidden and no quake is made; a star KO is
  clipped past the 10,000-unit far plane (RE-412). Yoshi's roll egg and
  egg explosion and the Thunder segments are not drawn (RE-415, RE-416).
- Sector Z Arwing, bonus stages and stage items remain.
- The grey damage shield and Yoshi's egg shield are not drawn (RE-384).
- Kirby's copy hats, Pikachu's Thunder and the Egg Lay egg are not
  drawn; the Fireball does not spin (RE-372, RE-373).
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
