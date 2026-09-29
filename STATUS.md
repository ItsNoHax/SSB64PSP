# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the VS countdown's traffic light and "GO!"
  (RE-394), with the entry focus's RNG draw and the `Blend` sprite
  combiner. Before it: fighter emblems (RE-393).
- **Next gameplay batch:** the battle timer (`ifCommonTimer*`, file 165)
  and "TIME UP"/"GAME SET" (`ifCommonAnnounce*`), on the sprite path. The
  Appear entry is parked in TODO.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Countdown | `ssb_game::countdown`, file 82 sprites, IA env colour; 35 goldens rebaselined | RE-394 |
| Emblems | `FTSprites` walk, pack v62 roles and emblem colours; 35 goldens rebaselined | RE-393 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-394; the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 101 goldens match twice (RE-394). Scene captures
  time out at 60 s. No golden passes through a platform.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v62: 27,770,800 bytes, SHA-256
  `4c1c7045adcb8ea09400e785f3674143527b99f86c2d52bb1cc25c0c332850dd`.
- `run` is 119,984 bytes in release and 124,692 in `golden_capture`; the
  MIPS branch range is 128 KB.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the selects draw plain slots (TODO). Converted
  sprites: damage digits, emblems, stock icons and file 82.
  With no save data, Mushroom Kingdom and the four unlockable fighters
  stay locked.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The one-frame grey damage shield and Yoshi's egg shield are not drawn (RE-384).
- Kirby's copy hats, Pikachu's Thunder, the Egg Lay victim's egg and most
  hit effects are not drawn. The Fireball does not spin (RE-372; kind 71
  is `func_ovl0_800CA5C8`, RE-373), and a released Charge Shot restarts
  its spin (RE-373). Other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
