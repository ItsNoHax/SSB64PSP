# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the KO presentation (RE-412): the blast explosion
  with each player's colours, the screen flash, the rebirth halo at TopN,
  the rebirth glow and the star KO's fade, through a `GMColAnim` port
  (`ssb_game::colanim`, `ssb_game::ko`), for every fighter.
- **Next gameplay batch:** hit effects and the in-game particle runtime
  (`efManager`'s hit sparks and `LBParticle` scripts in a match: the
  blast's streaks and the star KO sparkle), with the fighters' damage and
  hit-status colour animations on the new `GMColAnim` port. VS item
  spawning waits on the item kinds.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| KO presentation | `colanim`, `ko`, `draw_ko_effects`; pack v65 adds stage fog colours; `f1-training-rebirth` rebaselined, `f1-training-rebirthblast` added | RE-412 |
| VS select presentation | `players_vs::layer`, `players_screen`; sprite files 0, 17, 19–21 and eight gate LUTs packed; `f1-vs-players` rebaselined | RE-411 |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass (RE-412); `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 111 goldens match twice (RE-412); captures time out
  at 60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v65: 30,405,584 bytes, SHA-256
  `f0e8c31f06a96e9c4b87a12c973159a103c153976fc14ff3d296c98c47bd9f1d`.
- `run` is 20,064 bytes (branch range 128 KB); largest stack frame
  `enter_training`, about 128 KB of the 256 KB main-thread stack.
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the Training and stage selects draw plain
  slots (TODO). The results' emblem and confetti and the VS select's
  spotlight are not drawn (RE-410, RE-411). With no save data, Mushroom
  Kingdom and the four unlockable fighters stay locked.
- KOs: no particles (blast streaks, star sparkle), no quake, and the
  halo's rays are hidden (a converter gap); a star KO is clipped past the
  10,000-unit far plane (RE-412).
- Sector Z Arwing, bonus stages and stage items remain.
- The grey damage shield and Yoshi's egg shield are not drawn (RE-384).
- Kirby's copy hats, Pikachu's Thunder, the Egg Lay egg and most hit
  effects are not drawn; the Fireball does not spin (RE-372, RE-373).
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
