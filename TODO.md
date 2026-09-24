# TODO

Deferred work that is not the current batch ([`STATUS.md`](STATUS.md)) and not
a milestone in [`PLAN.md`](PLAN.md). Unordered. Delete an entry once a batch
or evidence record covers it.

## Rendering

| Item | Reason deferred | Evidence |
|---|---|---|
| RDP two-tile fractional blend (`SetLFrac`, `TextureIDNext`) | GE has no one-pass equivalent; needs a measured multipass design | RE-086, RE-301 |
| Dynamic stage colour/light tracks (2 `PrimColor`, 2 light) | Values are baked into vertex colour or lack a stage GE-light context; needs dynamic vertex/combiner lowering | RE-301 |
| Fighter costumes beyond 0 in `psp-game` | All palettes are packed and selectable in the viewer; the game still hardcodes costume 0 | RE-096, RE-261 |
| Independent fighter animation validation | Stage animation has a ROM-derived check (RE-050–052, RE-142); fighter costume/material animation does not | — |
| Per-scene texture residency | Archive-wide textures exceed the ~700 KiB VRAM budget; the measured worst match scene fits. Re-measure once a scene dependency graph exists | RE-076, RE-077 |
| Scene dependency graph | No explicit `scene → nodes → materials → textures → palettes` graph yet | — |
| Strict rendering mode | No fail-fast mode for unresolved textures, palettes or transforms | — |
| `WPAttributes` pairing shape | Only known instance (Link's boomerang) has no sub-objects; revisit if another appears | RE-058 |

## Hardware acceptance

Deferred by user instruction.

| Item | Reason deferred | Evidence |
|---|---|---|
| PSP-1000 support | Pack did not fit in 32 MiB and `MEMSIZE=1` is ignored. The current Donkey-animation pack is 22,200,288 bytes; re-measure before designing a reduced or streaming pack | RE-288, RE-318 |
| 30-minute run on a second unit | Only one unit (Slim) has run 30 minutes with the full pack | RE-273, RE-284 |
| Re-capture current goldens on hardware | RE-320 captured the v32 diagnostic object, not the full current golden matrix | RE-320 |

## Open questions

| ID | Question | Next step |
|---|---|---|
| RE-008 | What each C-button does in-game (taunt, camera) | All four now pass through as raw N64 C-buttons; confirm uses against `ft/ftkey.c` |
| RE-009 | PSP nub deadzone (20 units, linear rescale to ±80 is a guess) | Measure on hardware against decomp thresholds |
| RE-010 | Unused `MObjSub` fields | Find readers in the decomp if a material looks wrong |
| RE-011 | How `sGCDetailLevel` is chosen | Trace during `P5` profiling |

## Technical debt

- Runtime extern-relocation loader (the pack stores them zeroed; [D-011](docs/decisions/D-011.md))
- `AssetArena`, `GameArena`, `FrameArena`, `ObjectPool` allocators ([docs/memory.md](docs/memory.md))
- VFPU math, after `P5` profiling ([D-032](docs/decisions/D-032.md))
- `sceAudio` mixer thread (`P4`)
- Debug HUD uses `sceGuDebugFlush`: software-rasterizer-only in PPSSPP (RE-014) and faults on real hardware (RE-202); replace with GE geometry
