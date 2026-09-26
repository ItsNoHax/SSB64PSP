# TODO

Deferred work that is not the current batch ([`STATUS.md`](STATUS.md)) and not
a milestone in [`PLAN.md`](PLAN.md). Unordered. Delete an entry once a batch
or evidence record covers it.

## Rendering

| Item | Reason deferred | Evidence |
|---|---|---|
| Costume validation and dummy costume | `psp-game` picks the player's costume (RE-334) but no PPSSPP capture shows costumes 1–3; the dummy keeps costume 0 instead of the CPU's first free costume, which needs a golden refresh | RE-096, RE-261, RE-334 |
| Independent fighter animation validation | Stage animation has a ROM-derived check (RE-050–052, RE-142); fighter costume/material animation does not | — |
| Per-scene texture residency | Archive-wide textures exceed the ~700 KiB VRAM budget; the measured worst match scene fits. Re-measure with `romtool scene-deps` | RE-076, RE-077, RE-334 |
| Strict mode over the real pack | `romtool strict` and `strict_render` exist but have not run over pack v36 | RE-334 |
| Material-animation command 22 | `ssb-rom::matanim` rejects it; its writes are never read, so it can be skipped | RE-010 |
| `WPAttributes` pairing shape | Only known instance (Link's boomerang) has no sub-objects; revisit if another appears | RE-058 |

## Gameplay

| Item | Reason deferred | Evidence |
|---|---|---|
| Same-frame catcher and held hits | `ftCommonDamageUpdateMain`'s simultaneous-hit branches and catcher hitlag need a deferred per-frame damage queue; hits resolve one at a time | RE-333 |
| `recent_damage` for fighter hits | The source passes the frame's `damage_queue`; the hit path passes zero | RE-333 |
| Weapon staling | `wpMainGetStaledDamage` and weapon queue updates are not ported | RE-333 |
| Re-run goldens after RE-333 | Handicap rounding moves some knockback by one ulp, and repeated moves now deal staled damage | RE-333 |

## Hardware acceptance

Deferred by user instruction.

| Item | Reason deferred | Evidence |
|---|---|---|
| PSP-1000 support | Pack did not fit in 32 MiB and `MEMSIZE=1` is ignored. The current pack (v36) is 22,247,328 bytes; re-measure before designing a reduced or streaming pack | RE-288, RE-318, RE-327 |
| 30-minute run on a second unit | Only one unit (Slim) has run 30 minutes with the full pack | RE-273, RE-284 |
| Re-capture current goldens on hardware | RE-320 captured the v32 diagnostic object and RE-326 three v35 stages, not the full current golden matrix | RE-320, RE-326 |

## Open questions

| ID | Question | Next step |
|---|---|---|
| RE-009 | PSP nub deadzone (20 units, linear rescale to ±80 is a guess) | Measure on hardware against decomp thresholds |
| RE-011 | How `sGCDetailLevel` is chosen | Trace during `P5` profiling |

## Technical debt

- Check `ssb_rom::reloc_link` against real closures and use it from a runtime loader ([D-011](docs/decisions/D-011.md), RE-334)
- Wire `ssb_engine::memory` arenas and pools into `psp-runtime` ([docs/memory.md](docs/memory.md))
- VFPU math, after `P5` profiling ([D-032](docs/decisions/D-032.md))
- `sceAudio` mixer thread (`P4`)
- Debug HUD uses `sceGuDebugFlush`: software-rasterizer-only in PPSSPP (RE-014) and faults on real hardware (RE-202); replace with GE geometry
