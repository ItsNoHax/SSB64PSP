# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Yoshi's roll egg, egg explosion and shield-release
  shell fragments (RE-427, unchanged pack v78).
- **Next gameplay batch:** Sector Z's Arwing stage actor.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Yoshi egg effects | 4 goldens added; joint attachment, forced allocation and status cleanup | RE-427 |
| Faces, demo scripts, low detail | 2 goldens added, 17 rebaselined; Kirby Win mouth fixed | RE-426 |

## Verification baseline

- Workspace tests: 1,590 pass (absolute SSB64_ROM, one thread).
  Pinned 1.98.0 Clippy, workspace rustfmt and gameplay/ROM no_std pass.
- Both production PSP release builds pass; five existing viewer warnings.
- PPSSPPHeadless: 11 targeted scenes match twice; all four new goldens
  reject disabled RE-427 effects. Manifest now has 149 scenes; the last
  full matrix was 145/145 twice in two runs (RE-426).
- Production PPSSPP boots and renders; no new menu-navigation proof.
- Pack v78: 35,392,000 bytes, SHA-256
  `19ea571b50173348505470168fbf2fe3d322e44d577b0a3e7ed1827c9843120e`.
- Production `run`: 21,856 bytes; largest frame `enter_training`:
  159,584 bytes of the 256 KiB main-thread stack; `run` frame 18,544.
- N64 warp sources restored; rebuilt and original ROM SHA-1 e2929e10…
  Roll joint angles match traced frame 12; inspected roll, explosion and
  shield-release fragments agree (RE-427).
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).
  Scripted Training held 16,682 µs/frame in RE-360.

## Blockers and remaining scope

- Both selects' spotlight is not drawn; no-save-data unlocks remain locked.
- Sector Z Arwing, bonus stages and stage items remain.
- Entry Arwing cockpit (tick 264) and near-camera Poké Ball (224) remain
  unresolved; entry focus 2 and Luigi pipe/Kirby rightward-star comparisons
  are outstanding (RE-425).
- Four-fighter Dream Land texture closure is 726,924 bytes at low detail,
  saving 324,608 bytes but exceeding the 704,512-byte VRAM pool by 22,412.
  Residency remains open; measurement is not physical-PSP proof (RE-426).
- Results fade/wipe and broader N64 scene equality remain unverified.
  Stage draw performance (4.5 ms/frame, RE-360) belongs to `P5`.
