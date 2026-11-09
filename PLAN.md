# Plan

- Target: whole original game (every mode, menu, fighter, audio), 60 FPS,
  PSP-2000 or later; N64 frame-for-frame wherever the PSP allows.
- Gameplay: translated from [`ssb-decomp-re`][decomp]. Display lists → GE
  geometry at build time.
- Scope: only roadmap and task list. Current figures and work:
  [`STATUS.md`](STATUS.md). Rules: [`AGENTS.md`](AGENTS.md#documentation).
- "Verified" = PPSSPP unless a physical PSP is named.

## Project State

| Area | Current | Target |
|---|---|---|
| Architecture | Portable `ssb-rom`/`ssb-engine`/`ssb-game`, shared `psp-runtime`, thin `psp-game`, separate `psp-asset-viewer` ([D-044](docs/decisions/D-044.md)) | `ssb_engine::memory` arenas (`GameArena`, `FrameArena`, `ObjectPool`) |
| Asset pipeline | ROM tables read into the pack's `TABLES` section by `ssb-tablegen` ([D-048](docs/decisions/D-048.md)); `romtool` packs all 2,132 archive files, every reachable asset and the audio section; scenes load their own files ([D-046](docs/decisions/D-046.md)) | — |
| Rendering | Build-time display-list conversion; GE lowering with measured deviations; original display-link passes; CRT overscan crop ([D-047](docs/decisions/D-047.md)) | |
| Gameplay | 12 fighters, Master Hand, Metal Mario, Giant DK, Polygon Team; How to Play matches an N64 trace every frame (RE-472, RE-473) | None known (by-design rows below) |
| CPU | VS, Training and 1P behaviours, traits, objectives (RE-390, RE-391, RE-437, RE-438, RE-447, RE-568) | Opening-movie RNG draws vs N64 (PPSSPP `rng_trace`; N64 per-scene counts in RE-568) |
| Game modes | VS (four players, teams), Training, 1P to the ending, staff roll, challengers, bonus stages, Bonus Practice | None |
| Menus/UI | N64 logo, opening, title, attract loop, every menu and select, HUD | None known |
| Save data | N64 SRAM image in `ssb64.sav` ([D-045](docs/decisions/D-045.md)) | None (validators cannot fail on a PSP, RE-460) |
| Audio | N64 audio system ported whole, sample-exact ([D-049](docs/decisions/D-049.md), RE-479) | Within the PSP-2000 CPU budget everywhere |
| PSP performance | 60 FPS under PPSSPP; PSP-2000 figures in STATUS | 60 FPS on a PSP-2000 in every scene |
| Platform | PSP-2000 and later (64 MiB); PSP-1000 unsupported (RE-288, RE-475) | PSP-1000 (32 MiB) and later |
| Testing | Host tests (CI: no ROM, stub tables), ROM tests, PPSSPPHeadless goldens as pixel hashes ([visual regression](docs/visual-regression/README.md)), N64 RDRAM traces | Hardware acceptance matrix |

## Milestones

| ID | Area | State |
|---|---|---|
| `MS1` | Architecture, toolchain, CI | Done |
| `MS2` | Asset pipeline and pack | Done |
| `MS3` | Renderer fidelity | Done; goldens not re-captured on a PSP |
| `MS4` | Fighters, Master Hand, special fighters | Done; fidelity remainders below |
| `MS5` | Match systems: VS, Training, stages, items, CPU, KO | In progress |
| `MS6` | Front end, 1P campaign, save data | In progress |
| `MS7` | Audio and rumble | Done |
| `MS8` | PSP performance and hardware acceptance | In progress; never gates `MS4`–`MS7` ([D-032](docs/decisions/D-032.md), [D-036](docs/decisions/D-036.md)) |

- Retired labels (`P0`–`P5`, `M0`–`M4`, `R0`–`R3`, `F1`, `G0`–`G5`) →
  [`docs/evidence/INDEX.md`](docs/evidence/INDEX.md#retired-labels).

## Remaining Work

- Row = one open task: what remains / acceptance, then refs. Closed → delete row.


### PSP performance and hardware (`MS8`)

- Figures: STATUS (PSP-2000).
- Link orders: searched with `tools/icache/`, chosen on hardware by busy time
  (`tools/hw-link-order.sh`). Hot code changes size → re-search
  (RE-494–RE-500, RE-539, RE-577: names hash-independent); upkeep and
  measurement baseline: psp-hardware skill.
- Stack: deepest 126,888 B measured (f1-training-fighter-select); frames over 16 KB listed in `docs/memory.md`; shrink options in RE-568.

| Task | Remaining / acceptance | Refs |
|---|---|---|
| Boot time | Owner boot is slow, unexplained on the PSP-2000 (534 to about 417 ms to frame 1, RE-582); suspect the stale 64 MB `ms0:/PSP/GAME/ssb64/ssb64.pak` (now a fatal mismatch screen). Acceptance: owner XMB `boot.log` (PSP-1000 and Slim) with pack v149; then pack bytes (fighter-only tables out of the resident prefix, audio prefix); `load_pack` errors to the fatal screen | RE-582 |
| Campaign by hand | PSP-2000, current production EBOOT: campaign past Race to the Finish; then RE-476/RE-477 checks | RE-476–RE-478 |
| Hardware acceptance | 30-minute run on second unit; re-capture goldens on PSP; hand-input shield, grab, throws, costume picks | RE-273, RE-284, RE-320, RE-326, RE-339, RE-341 |
| Nub deadzone | Measure on hardware (20 units, linear rescale to ±80: unmeasured) | RE-009 |

Audio cost ([D-049](docs/decisions/D-049.md) amendment, RE-479): DSP runs on
the Media Engine (batch 159), started by the game's own kernel module
(batch 160). Remaining:

1. PSP-1000: ME path, memory.

### Gameplay math, tables and goldens

| Task | Remaining / acceptance | Refs |
|---|---|---|

### Audio and rumble (`MS7`)

| Task | Remaining / acceptance | Refs |
|---|---|---|

### Match systems (`MS5`)

| Task | Remaining / acceptance | Refs |
|---|---|---|

### Front end and 1P (`MS6`)

| Task | Remaining / acceptance | Refs |
|---|---|---|

### Architecture

| Task | Remaining / acceptance | Refs |
|---|---|---|
| ROM-free release | User puts the US ROM next to the EBOOT; first launch builds `ssb64.pak` on the PSP (ROM check, progress screen); `romtool pack` stays as the PC alternative for users. Owner limit: under 10 min on a real PSP. Owner's 37 compensated textures (RE-588 item 8). Portable packer `ssb-pack` (`no_std`), every `ssb-tablegen` table in the pack, builds without a ROM, EBOOT scan clean (RE-589); first-launch installer `INSTALL.PBP` ([D-053](docs/decisions/D-053.md)): PSP-2000 at the PSP-1000 start budget 8.78 min, identical, lowest free 9.8 MB (RE-588 item 15). XMB hand-off works (owner, PSP-2000). Remaining: HOME mid-build, NoSpace screen, a real PSP-1000 run. Acceptance: EBOOT carries no ROM data; pack equals the host-built one | [D-046](docs/decisions/D-046.md), [D-048](docs/decisions/D-048.md), RE-588, RE-589 |

## Completed

| Capability | Refs |
|---|---|
| Architecture: portable crates, shared `psp-runtime`, `psp-game`, `psp-asset-viewer`; pinned toolchains; CI without ROM | [D-026](docs/decisions/D-026.md), [D-030](docs/decisions/D-030.md), [D-044](docs/decisions/D-044.md) |
| ROM: validation; VPK0 (499 files); relocData (2,132 files, 61,343 intern, 3,092 extern relocs); F3DEX2 (1,864 lists, 0 failures) | RE-001, RE-002, RE-017 |
| Loader on `reloc_link` placement; per-run blocks owned by `AssetArena` (`ssb_rom::loader_plan`) | RE-571, [D-011](docs/decisions/D-011.md) |
| Memory arenas: `GameArena`, `AssetArena`, `FrameArena`, LIFO item/weapon pools, fixed effect pool, KO counters as plain data | RE-571, [memory](docs/memory.md#allocators) |
| Pack: meshes, textures (RGBA, IA, I, CI + palette banks), mat/joint anims, figatrees, sprites, particles, collision, stage and fighter tables; per-scene files | [D-046](docs/decisions/D-046.md), RE-475 |
| Renderer: geometry, projection, texture addressing, 3-point compensation, combiner, `MObj`, lighting, texgen, alpha/blend/depth, billboards, shadows, framebuffer effects, wipes, UI sprites, GE state cache | [rendering](docs/rendering.md) |
| Physical PSP rendering: PSP-2000, PSP-3000; two 10-minute runs, one 30-minute run | RE-270, RE-271, RE-273, RE-284 |
| Fighters: all 12 (Kirby with all copies), Master Hand, Metal Mario, Giant DK, Polygon Team | RE-344–RE-347, RE-457, RE-458 |
| Fighter systems: physics, collision, status machine, figatree, motion scripts, hits, shields, grabs, throws, ledges, recovery, KO, rebirth, colour animations | RE-330–RE-355, RE-366–RE-371, RE-388, RE-412–RE-414 |
| How to Play, opening battles: N64 RDRAM traces match every frame | RE-466, RE-468, RE-472–RE-474 |
| Weapons and items: all fighter weapons, 20 normal items, Poké Ball Pokémon, stage items, hazards | RE-354, RE-372–RE-381, RE-416, RE-430–RE-446 |
| Stages: 41 maps; 9 VS stages with controllers, hazards; wallpapers; Sector Z Arwing; bonus stages | RE-385, RE-419, RE-428, RE-429, RE-446, RE-452–RE-455 |
| CPU AI: input scripts, objectives, traits, item use | RE-390, RE-391, RE-437, RE-438 |
| VS: rules, countdown, HUD, pause, teams, four fighters, results | RE-389–RE-411, RE-420, RE-464 |
| Training: menu, stats | RE-438–RE-440 |
| 1P: every stage, bonus stages, Master Hand, ending, staff roll, congratulations, challengers, unlocks; full transition run | RE-447–RE-459, RE-478 |
| Save: `LBBackupData` in `ssb64.sav`; options, data menus | [D-045](docs/decisions/D-045.md), RE-460, RE-461 |
| Front end: N64 logo, opening, title, attract loop (How to Play, Characters, auto demo), mode/1P/VS menus, Bonus Practice | RE-462, RE-465, RE-467 |
| Per-scene loading, background reads: no intro load hitch on PSP-2000 | RE-475, RE-476 |
| CRT overscan crop: N64 visible box fills PSP height | [D-047](docs/decisions/D-047.md), RE-477 |
| CPU performance: 60 FPS under PPSSPP | RE-469–RE-471 |
| Audio: synthesizer, sequence player, FGM engine, `syAudio`; all gameplay/menu/scene calls; Sound Test; PSP thread on 32 kHz SRC channel | [D-049](docs/decisions/D-049.md), RE-479 |
| No ROM data committed or compiled in: tables in the pack, builds without a ROM; goldens as pixel hashes | [D-048](docs/decisions/D-048.md) |

[decomp]: https://github.com/VetriTheRetri/ssb-decomp-re
