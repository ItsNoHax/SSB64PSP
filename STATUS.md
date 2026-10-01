# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** texture-part faces, select/results demo scripts,
  and original low detail for three/four fighters (RE-426, pack v78).
- **Next gameplay batch:** Yoshi's roll egg and egg explosion, including
  shared egg-break particles (RE-415, RE-416, RE-418).
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Faces, demo scripts, low detail | 2 goldens added, 17 rebaselined; Kirby Win mouth fixed | RE-426 |
| Accessories, parts, vehicles | 7 goldens added, 8 rebaselined | RE-425 |

## Verification baseline

- Workspace tests: 1,584 pass (absolute SSB64_ROM, one thread).
  Pinned 1.98.0 Clippy, workspace rustfmt and gameplay/ROM no_std pass.
- Both production PSP release builds pass; five existing viewer warnings.
- PPSSPPHeadless: 145/145 goldens match twice in two separate runs;
  all 19 new/changed goldens reject disabled RE-426 behavior.
- Pack v78: 35,392,000 bytes, SHA-256
  `19ea571b50173348505470168fbf2fe3d322e44d577b0a3e7ed1827c9843120e`.
- Production `run`: 21,856 bytes; largest frame `enter_training`:
  159,568 bytes of the 256 KiB main-thread stack.
- N64 warp sources restored; rebuilt and original ROM SHA-1 e2929e10…
  Inspected blink, Win mouth, demo parts and low models agree (RE-426).
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).
  Scripted Training held 16,682 µs/frame in RE-360.

## Blockers and remaining scope

- Both selects' spotlight is not drawn; no-save-data unlocks remain locked.
- Yoshi roll egg/explosion and shield-drop egg-break particles are not drawn.
- Sector Z Arwing, bonus stages and stage items remain.
- Entry Arwing cockpit (tick 264) and near-camera Poké Ball (224) remain
  unresolved; entry focus 2 and Luigi pipe/Kirby rightward-star comparisons
  are outstanding (RE-425).
- Four-fighter Dream Land texture closure is 726,924 bytes at low detail,
  saving 324,608 bytes but exceeding the 704,512-byte VRAM pool by 22,412.
  Residency remains open; measurement is not physical-PSP proof (RE-426).
- Results fade/wipe and broader N64 scene equality remain unverified.
  Stage draw performance (4.5 ms/frame, RE-360) belongs to `P5`.
