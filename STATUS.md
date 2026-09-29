# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the fighters' effects (RE-415): `ftParamMakeEffect`
  for the motion scripts, colour animations and status code, the display
  hit effects (slash, orbs, sparks), weapon set-offs and the Boomerang's
  spark, with the source's random draws.
- **Next gameplay batch:** the weapons' own effects and weapon-against-weapon
  clashes (`wpProcessProcSearchHitWeapon`).
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Fighter effects | `fteffect`, display effects; 27 goldens rebaselined (effects, random draws) | RE-415 |
| Colour animations | generated `colanim` scripts; pack v66 adds the skeletons; 11 goldens rebaselined, 3 new | RE-414 |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass (RE-415); `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 114 goldens match (RE-415); captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v66: 30,491,056 bytes, SHA-256
  `d5e4ba7460a28330a3d9e8818e3e146945234c18f429d2c215989c121d4bfd89`.
- `run` is 19,004 bytes (branch range 128 KB); largest stack frame
  `enter_training`, about 128 KB of the 256 KB main-thread stack.
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the Training and stage selects draw plain
  slots (TODO). The results' emblem and confetti and the VS select's
  spotlight are not drawn (RE-410, RE-411). With no save data, Mushroom
  Kingdom and the four unlockable fighters stay locked.
- KOs: no quake, and the halo's rays are hidden; a star KO is clipped
  past the 10,000-unit far plane (RE-412). Weapons make none of their own
  effects; Yoshi's roll egg is not drawn (RE-415).
- Sector Z Arwing, bonus stages and stage items remain.
- The grey damage shield and Yoshi's egg shield are not drawn (RE-384).
- Kirby's copy hats, Pikachu's Thunder and the Egg Lay egg are not
  drawn; the Fireball does not spin (RE-372, RE-373).
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
