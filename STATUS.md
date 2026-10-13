# Status

Where the project is now. The roadmap and every open task are in
[`PLAN.md`](PLAN.md); history is in git.

## Current

| Area | State |
|---|---|
| Build | Tables read from the ROM are generated at build time ([D-048](docs/decisions/D-048.md)); host workspace on Rust 1.98.0; `psp-game` and `psp-asset-viewer` on the pinned nightly; pack v109 (`d3efddb2…`) with the audio section |
| Gameplay | All fighters, VS, Training, 1P campaign through the ending; How to Play matches the N64 on every frame |
| Audio | The N64 audio system ported whole, every call site wired, Sound Test; sample-exact against N64 captures of the opening, menus and How to Play ([D-049](docs/decisions/D-049.md), RE-479) |
| Rendering | CRT overscan crop ([D-047](docs/decisions/D-047.md), RE-477) |
| Frontend | N64 logo, opening, title and attract loop; every menu including Sound Test; per-scene loading with background reads (RE-475, RE-476) |
| PSP | 60 FPS under PPSSPP. PSP-2000 (RE-479): the production build plays the attract loop clean; the opening averages 55.8 FPS (58.7 without audio); four-player VS 48.27 without audio (`profile_hw`, RE-493; 40.6 at RE-479), and 54.8 against 51.2 FPS for a `profile` build with an order searched for it (RE-494), held under 60 by the game thread; no underruns. PSP-1000 unsupported |
| Tests | 2,277 workspace tests with the ROM (2,086 with stub tables, as CI), clippy and rustfmt pass; 198 of 198 goldens pass by pixel hash |

## Current Work

RE-496 followed up on RE-495's single-seed finding: sixteen anneal seeds
at 30,000 iterations and twelve more at 90,000, scored against a held-out
trace, found an order for the shipped audio build 3.7% below HEAD's
modelled misses (556 fewer a frame). On a `profile,capture_scene_file,
no_audio` proxy build the best of eight seeds ran 0.90 FPS faster than
the single-seed method (56.50 vs 55.60 FPS) and hardware ranked all three
candidate orders exactly as the model did. `psp-game/link-order.txt` now
ships that order. Returns flattened past ~16 seeds / 90,000 iterations;
a future re-search after hot code changes size should expect similar, not
larger, gains. Next: the next `MS8` lever
([`PLAN.md`](PLAN.md#remaining-work)), or add a trace-replay scorer that
places blocks at a relinked ELF's real addresses (RE-496 found `layout`'s
own "as linked" score can't be used to grade a relinked ELF directly).

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink falls over now and then (a `kill`, an `opt-level s` build):
  restarting it needs a person at the PSP. `profile` builds tick power
  every frame; other builds still let an unattended PSP auto-sleep
  (RE-481).

## Verification

Baseline after the link-order multi-seed search (RE-496; only
`psp-game/link-order.txt` changed):

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: 2,277
  pass. With `SSB64_STUB_TABLES=1` and no ROM: 2,086 pass, 191 ignored;
  `cargo fmt --check` passes. Clippy was last run at RE-493 (no workspace
  code changed since).
- Both PSP builds (`psp-game`, `psp-asset-viewer`) compile clean in
  release.
- Goldens: `tools/golden.sh verify`: 198 of 198 match (pack v109, link
  order change only — no rendering code changed).
- PPSSPP smoke test (`tools/run-ppsspp-headless.sh --crate psp-game
  --scene vs4@2400`): runs clean.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1: the production PRX
  ran the attract loop for four minutes without an exception; a chain of
  `profile`/`profile_hw` A/B runs of `vs4@2400` validated each performance
  change (RE-479, RE-481, RE-482, RE-485 through RE-496). The installed
  EBOOT and its `ms0:` pack are still v108.
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10…`.
