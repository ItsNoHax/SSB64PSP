# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the N64's battle draw order (RE-422, pack v74): six
  link passes, opaque before translucent lists, so stage layers 2 and 3
  cover fighters; vertex-alpha glows and shadows blend. CPU costume was
  already right.
- **Next gameplay batch:** stage colour fidelity: Yoshi's Island's
  platforms and centre, Zebes' tube and tint, Saffron's door lights.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Battle draw order | 4 goldens added, 25 rebaselined (layer 3 over fighters, glows blend) | RE-422 |
| Near plane 256, effect no-Z | 49 goldens rebaselined (near-plane depth, Sing/Magnet over ground) | RE-421 |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass, and `ssb-game` checks as `no_std` (RE-422).
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 131 goldens match (RE-422); captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v74: 34,455,072 bytes, SHA-256
  `2dbc2a68fba81d63c078acc8ea55fa01d1efe0ac19fd577d685fe4bf9eaabdd9`.
- `run` is 20,480 bytes (branch range 128 KB); largest stack frame
  `enter_training`, 159,456 bytes of the 256 KB main-thread stack (RE-422).
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Pikachu's hat and Jigglypuff's bow (`accesspart`) are not drawn; the
  selects' spotlight is not drawn. Yoshi's Island's stage draw differs from the N64 (grey platforms, heart) (TODO). The VS select's spotlight is not drawn (RE-411). With no save data, Mushroom
  Kingdom and the four unlockable fighters stay locked.
- Yoshi's roll egg and egg explosion are not drawn (RE-415, RE-416).
- Sector Z Arwing, bonus stages and stage items remain.
- Yoshi's shield-drop egg-break particles are not made (RE-418).
- Kirby's motion-script head swaps
  (`ftParamSetModelPartID`) are not ported (RE-417).
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
