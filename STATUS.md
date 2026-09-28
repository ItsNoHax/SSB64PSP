# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Luigi translation scales (RE-368). Pack v50 carries
  the ROM's 29 vectors. Figatree and shield-script translation tracks, plus
  the shield's neutral lookup pose, now use them in game and viewer.
- **Next gameplay batch:** a scripted PSP shield scene (TODO) to observe the
  tilted pose and collision sphere in a live Training run. Stage selection
  remains deferred.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Luigi translation scales | 29 ROM vectors; clip and shield endpoints checked; Luigi golden updated, Mario control unchanged | RE-368 |
| Shield tilt pose and clip start frame | 208 sector tables for 26 fighters, checked against `dobj_lookup`; goldens unchanged | RE-367 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread:
  **1,214 pass**. Pinned 1.98.0 Clippy with warnings denied, rustfmt and
  the `thumbv7em-none-eabi` `no_std` builds pass.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: Luigi fighter golden rebaselined (57,056 changed pixels
  at 2×), then Luigi and Mario controls match twice. No golden raises a
  shield, passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v50: 27,085,696 bytes, SHA-256
  `d8394ea153d52bd24df32415ff6a2ec92fe8dcdc672a80d392627643e7b45902`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- The shield is host-tested only; its bubble is not drawn (RE-367).
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
