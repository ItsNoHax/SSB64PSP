# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Saffron City's five Pokémon, flame/Razor Leaf
  weapons, gate callbacks, ROM clocks and rendering (RE-430, pack v81).
  Chansey's Egg dependency remains explicitly disabled.
- **Next gameplay batch:** normal container items, starting with Egg,
  plus the normal-item switches needed to enable Chansey's maker.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Saffron Pokémon | Five real items, two weapons, direct texture frames; 5 new goldens | RE-430 |
| Castle / Mushroom Kingdom items | Bumper, POW and two plants; ROM clocks; 2 new goldens | RE-429 |

## Verification baseline

- Workspace: 1,619 tests pass (absolute SSB64_ROM, pinned 1.98.0, one thread).
  Clippy, workspace rustfmt, gameplay/ROM no_std and docs validation pass.
- Both production PSP release builds pass. Game has no warnings; viewer
  retains five existing warnings. Production binaries are restored.
- Ten relevant game goldens pass twice at four-way concurrency, zero pixel
  differences, no existing rebaselines. Manifest: 157 scenes. Last full
  matrix: RE-429's 152 scenes; not rerun for this gameplay batch.
- Final production PPSSPPHeadless 20 s software smoke finishes without
  faults. No menu-navigation, N64 visual-equivalence or hardware proof.
- Pack v81: 35,435,520 bytes, SHA-256
  `350217cf1be1383e5ae13358668d8d4e69689206a66b4d3a7a354918280d7065`.
- Production `run` frame 26,048; largest frame `enter_training` 212,304.
  Combined 238,352 of the 256 KiB stack, leaving 23,792 before alignment
  and nested calls. ELF total 3,415,316 bytes. Details: RE-430.
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).
  Scripted Training held 16,682 µs/frame in RE-360.

## Blockers and remaining scope

- Chansey Egg/container and switches; bonus stages and pipe traversal.
  Pipe entry still must notify a plant. Bumper lit flash, audio, Pokémon
  and launched-plant N64 appearance comparisons remain.
- Both selects' spotlight is absent; no-save-data unlocks remain locked.
- Arwing laser appearance, 3D facing, pilot manoeuvres and wing collision
  remain unverified against the N64 (RE-428).
- Entry cockpit (264), near-camera Poké Ball (224), focus 2, Luigi pipe and
  Kirby rightward-star comparisons remain (RE-425).
- Four-fighter Dream Land texture closure exceeds VRAM by 22,412 bytes
  at low detail (RE-426). Results fade/wipe and broader scene equality
  remain unverified. Stage draw performance belongs to `P5`.
