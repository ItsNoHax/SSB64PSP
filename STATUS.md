# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Castle's Bumper, Mushroom Kingdom's POW Block and
  both Piranha Plants: gameplay, item/stage hookup and rendering (RE-429,
  pack v80). The POW quake spares its hitter.
- **Next gameplay batch:** Saffron City's monster items (Pokémon).
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Castle / Mushroom Kingdom items | Real stage items; ROM clocks and transforms; N64 reference views; 2 new goldens | RE-429 |
| Sector Z Arwing | Paths match an N64 RDRAM trace; 1 golden added, 7 bonus-2 rebaselined | RE-428 |

## Verification baseline

- Workspace tests: 1,609 pass (absolute SSB64_ROM, one thread).
  Pinned 1.98.0 Clippy, workspace rustfmt and gameplay/ROM no_std pass.
- Both production PSP release builds pass; game has no warnings, viewer
  has five existing warnings.
- PPSSPPHeadless: full 152-scene matrix run twice at four-way concurrency;
  every golden and paired capture matches. No existing rebaselines.
- Production PPSSPPHeadless 20 s software smoke finishes without faults;
  no menu-navigation or hardware proof.
- Pack v80: 35,419,520 bytes, SHA-256
  `7d37d021e090d4d4248a018a4b6ed3b9f9cec74a1274cd2d519e6ee32edba3b5`.
- Production `run`: 23,080 code bytes, frame 18,576. Largest frame
  `enter_training`: 211,992 of the 256 KiB main-thread stack. Entry plus
  caller consumes 230,568 bytes, leaving 31,576 before nested calls.
- N64 warp sources remain restored; rebuilt and original ROM SHA-1 e2929e10…
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).
  Scripted Training held 16,682 µs/frame in RE-360.

## Blockers and remaining scope

- Bonus stages, Saffron's Pokémon and pipe traversal remain. Pipe entry
  does not yet notify a plant. Bumper lit flash and an N64 plant-knockout
  comparison remain; RE-429's normal views establish qualitative shape,
  not pixel equality or full animation/material phase agreement.
- Both selects' spotlight is not drawn; no-save-data unlocks remain locked.
- Arwing lasers' look, 3D patterns' facing, pilot manoeuvres and wing
  collision are not yet compared with the N64 (RE-428).
- Entry Arwing cockpit (tick 264) and near-camera Poké Ball (224) remain
  unresolved; entry focus 2 and Luigi pipe/Kirby rightward-star comparisons
  are outstanding (RE-425).
- Four-fighter Dream Land texture closure is 726,924 bytes at low detail,
  exceeding the 704,512-byte VRAM pool by 22,412 (RE-426).
- Results fade/wipe and broader N64 scene equality remain unverified.
  Stage draw performance (4.5 ms/frame, RE-360) belongs to `P5`.
