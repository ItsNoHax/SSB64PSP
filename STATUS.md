# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed). Order:
  fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Samus's normals, specials and grab/throw motion scripts
  (`P2`), continuing the remaining fighter movesets.
- **Parallel track:** rendering fidelity (`P5`). Not a gate for gameplay.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Grab combat and ROM-free deferred work | Held fighters take halved damage and break loose at 6; stale-move queue and Training handicaps; Mario/Fox back throws hit bystanders; held offset scales by `size`; condensed Mario/Fox boxes restored; R expands to A + Z. Costume picks in `psp-game`, `strict_render`, scene dependency graph, extern linker and arenas/pools, none ROM-validated | RE-333, RE-334, RE-008, RE-010 |
| Gameplay joint attachment | Attack/catch boxes on posed source joints; held TopN through the catcher's heavy-item joint | RE-332 |

## Verification baseline

- `cargo test --workspace`: 933 tests pass without the ROM; clippy with
  `-D warnings` is clean. ROM-backed tests, `romtool anims --verify` and
  `romtool matcolors` were not run this batch (last passing results: RE-330,
  RE-332).
- Both PSP builds pass, `psp-game` also with `strict_render`.
- Pack v36 unchanged: 22,446,048 bytes, SHA-256
  `b5c86c5efe53030626fac4629561140da0d8596f4c39542ec07173c4ae5e6686`.
- PPSSPPHeadless: not run this batch. The 69-scene baseline is RE-331 and
  the grab golden RE-332; hit scenes need a re-run after RE-333.
- PSP-2000 Slim, firmware 6.61 ARK, PSPLink v3.2.1: stage 40 held
  1.026 ms/tick over ticks 601–3,600 (RE-329). v35 Mushroom Kingdom, Meta
  Crystal and Final Destination agree with PPSSPP (RE-326). The rest of the
  golden matrix and the Donkey gameplay are not hardware-validated.

## Blockers

- Fighter map collision is floor-only (no wall/ceiling solver).
- Hurtboxes remain one root sphere.
- Hits resolve one at a time; same-frame catcher/held hits need a deferred
  damage queue (RE-333).
