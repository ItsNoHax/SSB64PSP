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
| Asset pipeline | ROM tables generated at build time by `ssb-tablegen` ([D-048](docs/decisions/D-048.md)); `romtool` packs all 2,132 archive files, every reachable asset and the audio section; scenes load their own files ([D-046](docs/decisions/D-046.md)) | Trimmed wallpaper padding |
| Rendering | Build-time display-list conversion; GE lowering with measured deviations; original display-link passes; CRT overscan crop ([D-047](docs/decisions/D-047.md)) | |
| Gameplay | 12 fighters, Master Hand, Metal Mario, Giant DK, Polygon Team; How to Play matches an N64 trace every frame (RE-472, RE-473) | None known (by-design rows below) |
| CPU | VS, Training and 1P behaviours, traits, objectives (RE-390, RE-391, RE-437, RE-438, RE-447, RE-568) | Real menu boot (infeasible) |
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
  (RE-494–RE-500, RE-539, RE-577: names hash-independent).
- Stack: deepest 126,888 B measured (f1-training-fighter-select); frames over 16 KB listed in `docs/memory.md`; shrink options in RE-568.

| Task | Remaining / acceptance | Refs |
|---|---|---|
| Stage clip cost | Guard-band clip: test 0.63 ms + sub-triangle 0.31 ms vs4, 0.59 + 0.78 ms Zebes; per-mesh box pre-test and sign-OR fast path rejected (more work at ~7 vertices per mesh, RE-577, RE-578) | RE-519, RE-577, RE-578 |
| `.rodata` line re-screen | Batch 114 grew `.rodata` ~39 KB (`END_BLOCKS`, `MotionDesc` 16 B): re-screen `PSP_LINK_RODATA_LINE` + `relink.py` on the PSP-2000, five standard scenes | RE-584 |
| Scene clip ratio | Game clips at `FRUSTRATIO_1`; stages and movies clip at 2w. Measure GE shade interpolation on hardware; if screen-linear, clip all scene draws at the viewport and cost it | RE-519, D-051 |
| Master Hand stage | onepbosshit comets 17.3-19.3 ms, Fall 18.4-19.9 ms busy on the PSP-2000: effect CPU skinning; cut to 16.7 ms | RE-584 |
| Electric hit drop | vs4@1340 4xL9 Pikachu on the PSP-2000: 17.48 no hit, 18.23 shock ms busy; cut to 16.7 ms. Fighters 6.9 ms (profile): skin 1.2, corners 0.9, material 0.7, compose 0.8. Memo, normal-length table, skin product: measured loss. Left: prepass matrix reuse alone (≤0.2 ms, `.bss` 8 KB pad, re-search); profile audio (~22% of instructions) and stage draw/update | RE-584, RE-585 |
| Boot time | Owner boot is slow, unexplained on the PSP-2000 (534 to about 417 ms to frame 1, RE-582); suspect the stale 64 MB `ms0:/PSP/GAME/ssb64/ssb64.pak` (now a fatal mismatch screen). Acceptance: owner XMB `boot.log` (PSP-1000 and Slim) with pack v140; then pack bytes (fighter-only tables out of the resident prefix, audio prefix); `load_pack` errors to the fatal screen | RE-582 |
| Opening figatree waits | Misses behind a chunk in flight: Room tic 57 (8 ms), Jungle battle start (≤13.7 ms), fighter-scene posed fighter tic 15 (≤8 ms). Smaller chunk or later prefetch start; acceptance: no frame over 16.7 ms from reads | RE-575 |
| Baseline method | Pack on ms0, not host0 (host0 adds 0.3-1.0 ms); verify opening report-window mapping | RE-574 |
| Select NOISE cost | Measure `noise` span (~1.0 ms avg, 3.0 peak on PPSSPP) on a PSP-2000; acceptance: busy time unchanged within noise or cheaper path | RE-569 |
| Acid tile-0 duplicates | Zebes acid `textures[1..3]` duplicate the TEXEL1 images (~520 KiB): drop the duplicate slots | RE-364 |
| `vs4@2400` | Busy time back under RE-500's 13.64 ms; rise = small gameplay and draw additions | RE-533–RE-539 |
| Opening | Profile Clash (link order no effect), each fight's first frame (21–40 ms). Levers: `bind_texture` (~121/frame, ~11 µs each), lit-expand traffic (~1.2 µs/vertex) | RE-469–RE-471, RE-476, RE-532, RE-539 |
| Other scenes | RE-469 scenes, RE-471 worst cases on PSP; explain `vs4@4000` 5.2 ms vs PPSSPP 4.5. Memory stalls 55% of cycles; draw I-cache misses dominate | RE-480, RE-481 |
| D-047 fill | Measure +19% GE fill on PSP-2000: `vs4@4000`, opening, selects (`ge` span) | RE-477 |
| Texture residency | Cost of remaining main-RAM texel reads after photo-region pool (`texture_probe`). More resident textures need VRAM → only via 16-bit colour buffers (changes every golden) | RE-076, RE-486, RE-489–RE-492, RE-539 |
| Read-ahead gaps | 1P next stage, intro → battle (no side-effect-free next-stage prediction); bonus select → bonus battle; Characters scene; 1P intro proceed after a clear (N64 tic 81 vs PSP 60) and tally start (11-12 VIs) | RE-501 |
| Select clip files | Players-training entry: 10–33 scattered files (+16–18 ms). `enter` drains unrelated in-flight job (battle waits 28 ms when maps left after 13 ticks) | RE-501 |
| Wallpaper padding | Trim twelve 5551 wallpapers' 512 × 256 padding (~1.5 MiB) | RE-419 |
| Unpalettized mips | Mip levels never given to GE → drop (memory, not bandwidth) | RE-127, RE-491 |
| GE state batching | Only where profiling shows submission cost. Refs: `sf64-psp` material hash, `oot-PSP` sampler cache | RE-124, [D-036](docs/decisions/D-036.md) |
| VFPU | Only after profiling | [D-032](docs/decisions/D-032.md) |
| VFPU block copies | `lv.q`/`sv.q` 16-byte copies for large aligned transfers (vertex expansion, frozen picture, photo) vs rust-psp word `memcpy` | RE-471 |
| CPU/GE overlap | Double-buffered display lists: +1 frame display latency, never a game tick. `vs4` already overlaps ≥4.8 ms GE; no bus-contention relief. Measure GE time first | RE-469, RE-486, [D-047](docs/decisions/D-047.md) |
| `meshdraw` audit | Indexed draws, transformed-vertex reuse. sf64-psp: non-indexed rebatching beat reuse. Lit primitives stay indexed | RE-514, RE-530 |
| I-cache locality | 16 KB I + 16 KB D; per-mesh path of `draw_object_posed_nodes` dominates misses. Outline only cold blocks. Rejected: per-primitive outlining, `opt-level = "s"`, `#[optimize(size)]`, shadowed render-state table; LTO re-inlines single-call helpers | RE-481, RE-487, RE-488, RE-493, RE-495 |
| Stack depth | Deepest 127 KB of 512 KiB (`set_damage_detect_size` 29 KB leaf) | RE-469 |
| `run` branch range | Keep within 128 KiB MIPS reach | RE-431 |
| Debug HUD | `sceGuDebugFlush` (PPSSPP software rasterizer only; faults on PSP) → GE geometry | RE-014, RE-202 |
| Auto-sleep | Unattended release builds (no `profile`/`frame_timer`) auto-sleep → keep PSP awake or document why not | RE-481 |
| Campaign by hand | PSP-2000, current production EBOOT: campaign past Race to the Finish; then RE-476/RE-477 checks | RE-476–RE-478 |
| Hardware acceptance | 30-minute run on second unit; re-capture goldens on PSP; hand-input shield, grab, throws, costume picks | RE-273, RE-284, RE-320, RE-326, RE-339, RE-341 |
| Nub deadzone | Measure on hardware (20 units, linear rescale to ±80: unmeasured) | RE-009 |
| `sGCDetailLevel` | Trace how it is chosen | RE-011 |

Audio cost ([D-049](docs/decisions/D-049.md) amendment, RE-479), in order:

1. A/B audio vs `no_audio` on PSP-2000, current build.
2. VFPU reverb path (exact: constant coefficients, split products).
3. Media Engine probe:
   - throwaway crate; mcidclan `kcall.prx` (me-core-mapper) from user EBOOT,
     ARK 6.61;
   - `Dmem::envmix`, `adpcm_16` (`crates/ssb-engine/src/audio/dsp.rs`) on ME
     via 64-byte-aligned uncached mailbox;
   - accept: outputs equal SC over 10,000 repeats; ME ≤ 1.3× SC's 72 µs per
     voice pull; SC slowdown < 5%; survives sleep/wake;
   - SC fallback mandatory (PPSSPP has no ME);
   - no VME (24-bit; breaks D-049 sample-exactness).

### Gameplay math, tables and goldens

| Task | Remaining / acceptance | Refs |
|---|---|---|
| vs-four golden | Optional: narrow change to catch/thrown/relz/pulled | RE-511 |

### Audio and rumble (`MS7`)

| Task | Remaining / acceptance | Refs |
|---|---|---|
| LLE audio harness | Land `~/ppsspp-test/b64a/lle_harness/` as `--core/--rsp` options of `tools/n64-headless` (RMG core, parallel-RSP/cxd4) so `audio::n64ref` re-records from the repo | RE-479 |

### Match systems (`MS5`)

| Task | Remaining / acceptance | Refs |
|---|---|---|
| Yoshi | Effect-room flag0 breakout fallback unported by design | RE-337, RE-417, RE-432, RE-466, RE-468, RE-552, RE-567 |
| Window matanim blobs | Script packs whole source file; window the blob (pack format, MS8 size only) | RE-010, RE-378, RE-381 |
| CPU | Real menu boot (part B) infeasible | RE-568 |
| `WPAttributes` pairing | Revisit if another case like Link's boomerang appears | RE-058 |

### Front end and 1P (`MS6`)

| Task | Remaining / acceptance | Refs |
|---|---|---|
| Master Hand | Fall wallpaper: order `draw_lod_texel_alpha` and add a Fall trace to the training mix (untested; Fall +0.44 ms, Slim) | RE-585 |

### Architecture

| Task | Remaining / acceptance | Refs |
|---|---|---|
| ROM-free release | User puts the US ROM next to the EBOOT; first launch builds `ssb64.pak` on the PSP (ROM check, progress screen); `romtool pack` stays as the PC alternative for users. Measure first: solver (f64 vs f32) and ms0 speed on PSP-2000/1000. Build-time ROM tables (`ssb-tablegen`) move into that pack step. Acceptance: EBOOT carries no ROM data; pack equals the host-built one | [D-046](docs/decisions/D-046.md), [D-048](docs/decisions/D-048.md) |

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
| No ROM data committed: scripts, status flags, anim table generated at build time; goldens as pixel hashes | [D-048](docs/decisions/D-048.md) |

[decomp]: https://github.com/VetriTheRetri/ssb-decomp-re
