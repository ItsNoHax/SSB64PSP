# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Sector Z's Arwing: path splines (`TraI`), the
  controller, both lasers and the wing's collision group (RE-428, pack v79).
- **Next gameplay batch:** the stage items (Bumper, POW Block, Piranha
  Plants, Pokémon) the controllers already request.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Sector Z Arwing | Paths match an N64 RDRAM trace; 1 golden added, 7 bonus-2 rebaselined | RE-428 |
| Yoshi egg effects | 4 goldens added; joint attachment, forced allocation and status cleanup | RE-427 |

## Verification baseline

- Workspace tests: 1,599 pass (absolute SSB64_ROM, one thread).
  Pinned 1.98.0 Clippy, workspace rustfmt and gameplay/ROM no_std pass.
- Both production PSP release builds pass; five existing viewer warnings.
- PPSSPPHeadless: full matrix of 150 scenes run twice (RE-428); all match
  after the 7 explained bonus-2 rebaselines, the 3 VS time-up scenes only
  when not run 24-way.
- Production PPSSPP 20 s smoke runs without faults; no menu-navigation
  proof.
- Pack v79: 35,412,176 bytes, SHA-256
  `f0054b2d8d421194654d66be4c421059841a4d75a3e872ee21f58890c82b88ca`.
- Production `run`: 28,696 bytes; largest frame `enter_training`:
  166,536 bytes of the 256 KiB main-thread stack; `run` frame 18,608.
- N64 warp sources restored; rebuilt and original ROM SHA-1 e2929e10…
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).
  Scripted Training held 16,682 µs/frame in RE-360.

## Blockers and remaining scope

- Both selects' spotlight is not drawn; no-save-data unlocks remain locked.
- Bonus stages and stage items remain. The Arwing's lasers' look, its 3D
  patterns' facing, pilot manoeuvres and wing collision are not yet
  compared with the N64 (RE-428).
- Entry Arwing cockpit (tick 264) and near-camera Poké Ball (224) remain
  unresolved; entry focus 2 and Luigi pipe/Kirby rightward-star comparisons
  are outstanding (RE-425).
- Four-fighter Dream Land texture closure is 726,924 bytes at low detail,
  exceeding the 704,512-byte VRAM pool by 22,412 (RE-426).
- Results fade/wipe and broader N64 scene equality remain unverified.
  Stage draw performance (4.5 ms/frame, RE-360) belongs to `P5`.
