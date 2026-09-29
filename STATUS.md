# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** VS team battles (RE-407): the team-attack rule in
  every hit and catch search, fighter/weapon/item teams, the thrown
  star's thrower team, the CPU's teammate filter, team emblems and team
  sudden death; new `f1-vs-team` golden. Before it: the battle camera
  (RE-406).
- **Next gameplay batch:** the results' presentation.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Team battles | `ssb_game::team::TeamRules`, `Fighter::team`, pool teams, `is_opponent`, `Battle::with_teams`; new `f1-vs-team` golden | RE-407 |
| Battle camera | `CameraMode::Entry`, `camera_interest(stage)`, `tick_battle_camera`; 39 goldens rebaselined | RE-406 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-407; the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 109 goldens match twice (RE-407). Scene captures
  time out at 60 s. No golden passes through a platform.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v63: 28,254,080 bytes, SHA-256
  `8bfe1acc108ec0305fd7ab8b96c06077c2961d6d244bcb11d0bdfafef1130197`.
- `run` is 20,628 bytes in release (RE-407); the MIPS branch range is
  128 KB. Its stack frame is about 4 KB; the largest frame is
  `enter_training` at about 128 KB, within the 256 KB main-thread stack.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the selects draw plain slots (TODO). Converted
  sprites: damage digits, emblems, stock icons, files 37, 82, 165, 197.
  With no save data, Mushroom Kingdom and the four unlockable fighters
  stay locked.
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The one-frame grey damage shield and Yoshi's egg shield are not drawn (RE-384).
- Kirby's copy hats, Pikachu's Thunder, the Egg Lay victim's egg and most
  hit effects are not drawn. The Fireball does not spin (RE-372; kind 71
  is `func_ovl0_800CA5C8`, RE-373), and a released Charge Shot restarts
  its spin (RE-373). Other item kinds remain.
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
