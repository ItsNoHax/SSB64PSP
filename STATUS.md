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

RE-494 replaced the heuristic link order with a searched one.
`tools/icache/layout.c` replays a cache-model trace for each candidate order
(exact against the relinked binary) and anneals the hot functions' order.
On a PSP-2000, three layouts of one `profile` build ranked as the model
ranked them: 47.6 / 51.2 / 54.8 FPS for 533K / 451K / 372K modelled misses.
The shipped audio build's new order cuts modelled misses 21% (25% in the
draw), about 1.5 ms a frame. That gain is the model's, since only `profile`
builds report FPS. An order fits only its build: search again when hot code
changes size (procedure in RE-494). `link-order.txt` edits now relink. Next:
the flags-derived render-state table, or the next `MS8` lever
([`PLAN.md`](PLAN.md#remaining-work)).

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink falls over now and then (a `kill`, an `opt-level s` build):
  restarting it needs a person at the PSP. `profile` builds tick power
  every frame; other builds still let an unattended PSP auto-sleep
  (RE-481).

## Verification

Baseline after the searched link order (RE-494):

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: 2,277
  pass. With `SSB64_STUB_TABLES=1` and no ROM: 2,086 pass, 191 ignored;
  `cargo fmt --check` passes. Clippy was last run at RE-493 (this batch
  changed no workspace code).
- Both PSP builds (`psp-game`, `psp-asset-viewer`) compile clean in
  release.
- Goldens: `tools/golden.sh verify --filter '^f1-vs'`: 24 of 24 match
  (code placement only; all 198 matched at RE-493).
- PPSSPP smoke test (`tools/run-ppsspp-headless.sh --crate psp-game
  --scene vs4@2400`): runs clean.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1: the production PRX
  ran the attract loop for four minutes without an exception; a chain of
  `profile`/`profile_hw` A/B runs of `vs4@2400` validated each performance
  change (RE-479, RE-481, RE-482, RE-485 through RE-494). The installed
  EBOOT and its `ms0:` pack are still v108.
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10…`.
