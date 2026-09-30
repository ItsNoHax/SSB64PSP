# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** stage colour fidelity (RE-423, pack v75): strip
  `G_LOADTILE` loads, lists' own lighting state (stages draw unlit), and
  layer 1's depth state for its link's objects. Eight of nine VS stages
  match the N64; Zebes keeps a cutout fringe.
- **Next batch:** check the fighters whose vertices RE-423's lighting rule
  changed (Samus, Kirby, Jigglypuff, Ness) against N64 RDRAM; the
  `TEX_EDGE` cutout coverage (Zebes fringe).
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Stage colours | 5 goldens added, 61 rebaselined (unlit stages, fruit strips, depth) | RE-423 |
| Battle draw order | 4 goldens added, 25 rebaselined (layer 3 over fighters, glows blend) | RE-422 |

## Verification baseline

- Workspace tests (absolute `SSB64_ROM`, one thread), pinned 1.98.0 Clippy
  and rustfmt pass, and `ssb-game` checks as `no_std` (RE-423).
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 136 goldens match (RE-423); captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v75: 34,589,696 bytes, SHA-256
  `dfa1768f6a0b00b2ee6cb009eaf5111412425f18dc6be8d672eea12041130a58`.
- `run` is 20,480 bytes (branch range 128 KB); largest stack frame
  `enter_training`, 159,456 bytes of the 256 KB main-thread stack (RE-423).
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Pikachu's hat and Jigglypuff's bow (`accesspart`) and both selects'
  spotlight are not drawn (RE-411, TODO). With no save data, Mushroom
  Kingdom and the four unlockable fighters stay locked.
- Yoshi's roll egg and egg explosion are not drawn (RE-415, RE-416).
- Sector Z Arwing, bonus stages and stage items remain.
- Yoshi's shield-drop egg-break particles are not made (RE-418).
- Kirby's motion-script head swaps
  (`ftParamSetModelPartID`) are not ported (RE-417).
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
