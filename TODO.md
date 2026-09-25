# TODO

Deferred work that is not the current batch ([`STATUS.md`](STATUS.md)) and not
a milestone in [`PLAN.md`](PLAN.md). Unordered. Delete an entry once a batch
or evidence record covers it.

## Rendering

| Item | Reason deferred | Evidence |
|---|---|---|
| Fighter costumes beyond 0 in `psp-game` | All palettes are packed and selectable in the viewer; the game still hardcodes costume 0 | RE-096, RE-261 |
| Independent fighter animation validation | Stage animation has a ROM-derived check (RE-050–052, RE-142); fighter costume/material animation does not | — |
| Per-scene texture residency | Archive-wide textures exceed the ~700 KiB VRAM budget; the measured worst match scene fits. Re-measure once a scene dependency graph exists | RE-076, RE-077 |
| Scene dependency graph | No explicit `scene → nodes → materials → textures → palettes` graph yet | — |
| Strict rendering mode | No fail-fast mode for unresolved textures, palettes or transforms | — |
| `WPAttributes` pairing shape | Only known instance (Link's boomerang) has no sub-objects; revisit if another appears | RE-058 |

## Gameplay

| Item | Reason deferred | Evidence |
|---|---|---|
| Damage to a held fighter | `ftCommonDamageCheckCaptureKeepHold` needs its own hit response while the capture link persists; this batch leaves such hits unregistered | RE-330 |
| Throw collisions against bystanders | Mario and Fox back-throw attack boxes cannot hit another fighter in the current two-fighter match | RE-330 |
| Throw stale-move and handicap modifiers | The current Training match has no stale-move queue or handicap state; throw damage uses descriptor values | RE-330 |
| Restore condensed same-valued attack boxes | Some Mario, Fox and Donkey motion commands attach otherwise identical boxes to different joints; the existing `MoveData` kept one copy, so joint placement now exposes this old omission | RE-332 |
| Draw Samus's Charge Shot and Bomb | Gameplay weapons exist; their meshes (`dSamusSpecial3` and the `SamusModel` bomb display list with palette blink) are not packed or drawn | RE-333 |
| Mario down-air landing | `dFTMarioMotionDescs` has no `LandingAirLw` motion, so the source enters `LandingAirNull` from `AttackAirLw`; the Mario port still enters `LandingAirLw` (Luigi's port follows the source) | RE-334 |
| Select and draw Samus, Luigi and Link in `psp-game` | The movesets are host-only; Luigi's Fireball needs Mario's mesh with palette frame 1; Link's Boomerang and Spin Attack effect are not drawn | RE-333–335 |
| Weapon map-bound removal | `wpProcessProcWeaponMain` deletes weapons outside `map_bound_*`; the pool keeps a missed Blaster or Charge Shot until a map contact | RE-333 |
| Boomerang off-camera removal | `wpLinkBoomerangCheckOffCamera` needs the battle camera's projection; the pool keeps the Boomerang until its lifetime ends | RE-335 |
| Item system | Link's Bomb (`itLinkBomb`) and the item-throw branch of his down special need held items and `ftCommonItemThrow*` | RE-335 |
| Weapon shield and hop callbacks | Weapons pass through shields; the Boomerang's `ProcShield`/`ProcHop` are not reached | RE-335 |
| Escape (roll) statuses | Samus's Charge Shot loop reads `ftCommonEscapeGetStatus`; `EscapeF`/`EscapeB` are ordinals only | RE-333 |
| Hit-status intangibility | `SetHitStatusAll(2)` (Screw Attack start, throws) and Luigi's Super Jump Punch and up-smash intangibility have no effect on the root-sphere hurtbox | RE-333, RE-334 |
| `romtool matcolors` texel check | Textures 160 and 161 differ from `sprites[0]` in the alpha bit of 307 texels, and the resolver check exits with an error. The HEAD pack before RE-335 (SHA-256 `096a03c9…`) fails the same way, so the earlier "145/145" baseline is stale | RE-335 |
| Non-unit held fighter scale | Held TopN placement currently uses the first-child offset at normal fighter size; giant/shrunken capture needs the root scale applied as in `ftCommonCapturePulledRotateScale` | RE-332 |

## Hardware acceptance

Deferred by user instruction.

| Item | Reason deferred | Evidence |
|---|---|---|
| PSP-1000 support | Pack did not fit in 32 MiB and `MEMSIZE=1` is ignored. The current pack (v36, with Samus, Luigi and Link slots) is 22,738,640 bytes; re-measure before designing a reduced or streaming pack | RE-288, RE-318, RE-327 |
| 30-minute run on a second unit | Only one unit (Slim) has run 30 minutes with the full pack | RE-273, RE-284 |
| Re-capture current goldens on hardware | RE-320 captured the v32 diagnostic object and RE-326 three v35 stages, not the full current golden matrix | RE-320, RE-326 |

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
