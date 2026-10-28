# Architecture

Scope: current and target architecture, original-game reference.
Crate ownership: [`AGENTS.md`](../AGENTS.md). Open work: [`PLAN.md`](../PLAN.md).

## Current Architecture

```
ROM (user-supplied) ──romtool pack──> assets/generated/ssb64.pak (gitignored)
                                           │ read at boot and per scene
psp-game ─────────┐                        ▼
                  ├──> psp-runtime ──┬──> crates/ssb-game    portable gameplay
psp-asset-viewer ─┘    (PSP only)    ├──> crates/ssb-engine  math, coord, timing, traits
                                     ├──> crates/ssb-rom     ROM, archive, formats, pack
                                     ├──> crates/ssb-pose    fighter tick and pose (also host tests)
                                     └──> crates/ssb-capture golden-capture scene specs

crates/ssb-tablegen: build scripts of ssb-game and ssb-rom, ROM -> tables in OUT_DIR
```

| Component | Role |
|---|---|
| `tools/romtool` | Verifies the ROM, decodes the relocData archive, converts display lists, textures, animations, sprites and tables, writes the pack |
| `crates/ssb-rom` | `archive`, `vpk0`, F3DEX2 decoding (`dl`), mesh and texture conversion (`mesh`, `texture`, `psp_texture`, `filter_compensation`), `pack`, scene file closures, `reloc_link`, `audio` data |
| `crates/ssb-engine` | Traits (renderer, input, timing), the N64 audio system (`audio`), math incl. `lbCommonSin`, the one N64 → PSP screen mapping (`coord`), allocators (`memory`) |
| `crates/ssb-game` | Gameplay translated from the decomp: fighters, statuses, motion scripts, combat, collision, items, weapons, effects, stages, CPU, camera, HUD, menus, 1P campaign (`spgame`), opening, save data (`backup`) |
| `crates/ssb-pose` | `PosedFighter`: a fighter with its skeleton and clip state, ticked in the match's two halves and settled after the hit phase; joint sampling, played-spawn placement, `quake_translate`; run by `psp-runtime`'s `FighterScene` and the host replays ([D-052](decisions/D-052.md)) |
| `crates/ssb-capture` | Golden-capture scene specs shared by both PSP binaries and host tests |
| `crates/ssb-tablegen` | Build-time generation of the tables the game reads from the ROM (layout and decoders only, [D-048](decisions/D-048.md)) |
| `psp-runtime` | All PSP code: pack and scene files (`assets`, `scene_files`), GE (`gu`, `meshdraw`), `audio`, input, timing, save file, profiler, main thread, transitions, movie and particle draws |
| `psp-game` | Application: scene orchestration and screens adapting portable state to packed assets |
| `psp-asset-viewer` | Asset browser and deterministic render-regression scenes |

- Simulation: fixed 60 Hz, catch-up cap, decoupled from rendering ([D-005](decisions/D-005.md))
- Display lists: converted once at build time; RDP not emulated ([D-001](decisions/D-001.md), [D-002](decisions/D-002.md))
- Pack: mandatory, built separately ([D-028](decisions/D-028.md))
  - boot reads header, tables, shared region
  - each scene reads its files and extern closure at start; Maps loads MAPS + cursor preview, swapped per cursor move (`scene_load::maps_preview`)
  - next scene's files prefetched where known ([D-046](decisions/D-046.md))
  - sizes, budgets: [memory.md](memory.md)
- Extern relocations: zeroed in the pack; converter follows them ([D-011](decisions/D-011.md))
- Save data: N64 SRAM image in `ssb64.sav` beside the pack ([D-045](decisions/D-045.md))
- PSP crates: outside the Cargo workspace; pinned nightly, `-Z build-std` ([D-026](decisions/D-026.md))
- Portable crates: `no_std` by default ([D-027](decisions/D-027.md))

## Target Architecture

| Area | Current | Target |
|---|---|---|
| Asset loading | Scene files read whole into heap allocations; `reloc_link` unused at runtime | Runtime loader on `ssb_rom::reloc_link`, one closure layout per scene as `lbRelocGetAllocSize` |
| Memory | Global size-class heap over `sceKernelAllocPartitionMemory`; fallible large loads | `ssb_engine::memory` arenas and pools in `psp-runtime` ([memory](memory.md#allocators)) |
| Textures | Sampled from the pack in main RAM; large and repeated textures copied into the idle VRAM photo region (RE-539, [memory](memory.md#vram)) | Per-scene VRAM residency where it measurably helps |
| Audio | N64 audio system ported whole, synthesized on its own thread ([D-049](decisions/D-049.md)) | Same, within the PSP-2000's CPU budget |
| Optimization | Scalar math, GE state cache | VFPU and state batching only where profiling shows a cost ([D-032](decisions/D-032.md), [D-036](decisions/D-036.md)) |
| Debug HUD | `sceGuDebugFlush` (PPSSPP software renderer only) | GE geometry |

## Major Subsystems

### Rendering

- `ssb-rom`: relocData display list → PSP vertex buffers and GE state
- `psp-runtime::meshdraw`: draws them through a GE state cache
- Domain detail, deviations, validation: [`rendering.md`](rendering.md)

### Gameplay

- `ssb-game` mirrors decomp modules `ft/`, `wp/`, `it/`, `ef/`, `gr/`, `gm/`, `if/`, `mn/`, `sc/`
- Code-segment data of the original, read from the user's ROM at build time by `crates/ssb-tablegen`, never committed ([D-048](decisions/D-048.md)):
  - motion scripts, colour-animation scripts, CPU input scripts
  - status tables, animation file tables
- Archive data, from the pack: models, animations, `FTAttributes`, collision, sprites
- Objects: explicit state machines, replacing the original's `GObjProcess` coroutine threads

#### Match loop

- One frame: `ssb_game::matchtick::step`, run by `psp-game` (`training_step`) and the host replays (`tick_pair_pads`) ([D-052](decisions/D-052.md)).
- Passes, in order: `interrupt_pass`, `board_and_stage`, `physics_pass` → `fighter_physics`, `effects_and_camera`, `hit_pass`; then the settle after the hit phase.
- Platform seams: `Seat<G>` (fighter and its ground), `Ground`, `Arena`, `MatchHooks` (poison, profiler spans, `camera_stage`); `HasFighter::rig` gives the camera seat's `CameraRig`.
- Camera: `ssb_game::matchtick::camera_pass` ticks the `CameraRig` (`ssb-game` `camera/rig.rs`); the rig is stored on `FighterScene`. CPU sight: `ssb_game::computer::sight::CpuSight`.
- `explain_movement` replays run on `step`.
- Pack-bound halves: `psp-runtime` `arena.rs` (`PackGround`, `MatchArena`); `FighterScene` implements `HasFighter`.
- Order is pinned by `matchtick/tests.rs`: decomp `PROCESSES` table vs port order, with named departures (RE-556).

### Asset Pipeline

- `romtool pack`: reads ROM, decompresses VPK0, relocates archive files, writes the pack
- Pack contents: shared region, each archive file's bytes, audio section
- Version: `ssb-rom/src/pack.rs`; current in [STATUS.md](../STATUS.md)
- Paletted textures stay paletted ([D-003](decisions/D-003.md))
- ROM-backed audits in `romtool`: `verify`, `scene-deps`, `residuals`, `matcolors`, `texgen`

### Audio

[D-049](decisions/D-049.md). Original has three layers, all ported:

- libultra `n_audio`: synthesizer, compressed-sequence player
- FGM sound-effect engine in `n_env.c`
- `syAudio` layer of `src/sys/audio.c`

| Original | Port |
|---|---|
| `S1_music.sbk`, `B1_sounds1/2.ctl/.tbl`, `fgm.unk/.tbl/.ucd`, the microcode's tables | The pack's audio section, read once at boot and never freed (`ssb_rom::audio`, `ssb_engine::audio::data`) |
| RSP `n_aspMain` commands (ADPCM, resample, envelope mixer, mixer, interleave, pole filter) | `ssb_engine::audio::dsp`, integer fixed point |
| `n_alSyn*`, `n_alAudioFrame`, the pull chain, the custom reverb | `synth`, `reverb` |
| `n_alCSP*`, `__n_CSP*`, `n_alCSeq*`, the event queue, `n_seqplayer.c` helpers | `csplayer`, `cseq`, `evtq` |
| `syAudioInitOsc`/`UpdateOsc`, `alCents2Ratio` | `osc` |
| FGM engine (`func_80027460`, `func_80026B90`, LFOs, pools) | `fgm` |
| `syAudioThreadMain`'s frame (552/368 samples), BGM status machine, fades, mono | `system::AudioSystem` |
| The game's calls (`syAudioPlayBGM`, `func_800269C0_275C0`, ...) | `ssb_game::sound`, through `AudioApi` |
| `syAudioThreadMain`'s thread (priority 110 > game 50) | `psp_runtime::audio`: a thread at priority 0x1C (game 32), 32 kHz SRC channel, 512-sample blocks |

- Game calls the API synchronously
- `SharedAudio`: serialises those calls with audio-thread frames under a kernel semaphore
  - why: N64 masks interrupts around the same list operations
- Each frame: sequencer and FGM handlers, then 184-sample sub-frame mixing
- Rates: synth at N64 32006 Hz; PSP plays 32000 Hz

### Frontend

- Scene logic: `ssb-game::menu`, `opening`, `spgame`, select and results modules
- Drawing: `psp-game` `*_screen.rs` modules, from packed sprites and models
- Scene manager follows `scManagerRunLoop`: N64 logo, opening, title and attract loop, menus, selects, battles, results, 1P campaign

### Platform (PSP)

| N64 | PSP |
|---|---|
| `osCreateThread` / `osStartThread` | `sceKernelCreateThread` / `sceKernelStartThread` |
| VI retrace | `sceDisplayWaitVblankStart` |
| PI cart DMA | file reads of the pack (`sceIoRead`; background chunks on the `ssb_audio_pages` streamer thread) |
| SI controller thread | `sceCtrlReadBufferPositive` |
| RCP task scheduler | dropped; `sceGuStart`/`sceGuFinish`/`sceGuSync` |
| SRAM | `ssb64.sav` |

Memory, VRAM, stack, `MEMSIZE`, cache-coherency rules: [`memory.md`](memory.md).

## Key Boundaries

- `ssb-game`, `ssb-engine`, `ssb-rom`: never depend on `psp-runtime` ([D-044](decisions/D-044.md))
- `psp-runtime`: no gameplay logic
- `psp-game`: no gameplay or PSP backend code
- `psp-asset-viewer`: no game logic
- ROM-derived assets: only in the gitignored pack ([D-033](decisions/D-033.md)); EBOOT never contains it
- `unsafe`: only PSP APIs, VFPU, GE memory ([D-031](decisions/D-031.md))

## Original Game Reference

Source: [`ssb-decomp-re`](https://github.com/VetriTheRetri/ssb-decomp-re), 100% matched (7,165/7,165 functions, US and JP).

### ROM

| | |
|---|---|
| Internal name | `SMASH BROTHERS` (0x20) |
| Game code | `NALE` (0x3B); SHA-1 and MD5 in [README](../README.md#requirements) |
| Format | 16 MiB big-endian `.z64` |
| Graphics microcode | F3DEX2 (`gspF3DEX2_fifo`) |
| Audio microcode | `n_aspMain` |

- Supported: US ROM only
- JP: needs archive constants from the decomp's JP linker script, checked against a real dump

### Threads

```
thread 1 idle
└─ thread 5 (pri 50)  main game thread → scManagerRunLoop()
   ├─ thread 3 (pri 120) sySchedulerThreadMain   RCP task scheduler
   ├─ thread 4 (pri 110) syAudioThreadMain       audio synthesis
   └─ thread 6 (pri 115) syControllerThreadMain  controller
```

- Game thread has the lowest priority: audio and display never starve behind a slow frame

### Objects

| Type | Role |
|---|---|
| `GObj` | Generic object: id, link/priority lists, run and display callbacks, camera mask, payload |
| `DObj` | Transform node (translate/rotate/scale, parent/child/sibling), display list, `AObj` channels |
| `MObj` | Material: texture, palette, prim/env/blend colour, UV scroll, light colours |
| `AObj` | One animation channel |
| `CObj` | Camera: projection and draw-layer mask |

- A fighter is a `DObj` tree; `objdisplay.c` walks it with `gSPMatrix`
  push/pop and calls display lists stored in the ROM.
- `GObj::camera_mask` selects draw layers per camera; `DObjDistDL` selects
  a list by camera distance and `sGCDetailLevel` picks a global tier.
- Animation scripts: `AObjEvent16` (fighter figatrees) and `AObjEvent32`
  (everything else) ([D-020](decisions/D-020.md)).

### relocData archive

- Original: `src/lb/lbreloc.c`; port: `crates/ssb-rom/src/archive.rs`
- US: 9.07 MiB at ROM `0x1AC870`, 2,132 files

```
table_lo 0x1AC870 → [LBTableEntry; 2133]  (2132 files + sentinel)
table_hi 0x1B2C6C → file data | extern-id list | ...
```

```c
struct LBTableEntry {           // all sizes in 32-bit words
    ub32 is_compressed : 1;
    u32  data_offset   : 31;    // relative to table_hi
    u16  reloc_intern_offset;   // first intern slot, or 0xFFFF
    u16  compressed_size;       // on-ROM size, even if uncompressed
    u16  reloc_extern_offset;   // first extern slot, or 0xFFFF
    u16  decompressed_size;
};
```

Load steps:

1. Read `compressed_size` words; VPK0-decode if compressed.
2. Walk the intern chain: slot `{u16 next, u16 target_word}` becomes `base + target_word * 4`.
3. Walk the extern chain the same way.
   - target file IDs: `u16` array after the file data
   - consumed in chain order

US archive: 61,343 intern and 3,092 extern slots ([D-010](decisions/D-010.md)).

### VPK0

- `syDmaDecodeVpk0` in `src/sys/dma.c`: LZ77 with two Huffman trees
- US: 499 of 2,132 files compressed (9.07 → 16.29 MiB) ([D-009](decisions/D-009.md))
- Trees are postfix:
  - `0` pushes a leaf
  - `1` pops two, pushes a node
  - `1` with fewer than two nodes ends the tree
- Leaf value = bit width: read that many more bits for the value
- `sample_method` selects the distance encoding
  - two-sample form: `src = dst - value*4 - correction + 8`

### Fighters and physics

- Physics is `f32`, not fixed point; `+Y` is up
  ([D-021](decisions/D-021.md)).
- `Z` is a shallow depth axis clamped to ±60 by trimming velocity.
- Ground (`vel_ground`, X only), air (`vel_air`) and `vel_knockback` are
  separate, with explicit transfer on takeoff and landing.
- Per-character tuning is data (`FTAttributes`) ([D-022](decisions/D-022.md)).
- Floor material scales traction:
  `dMPCollisionMaterialFrictions[floor_flags & MAP_VERTEX_MAT_MASK] * attr->traction`.

`FTKind` ordinals index asset tables and must not be renumbered:

| Ordinal | Fighters |
|---|---|
| 0–11 | Mario, Fox, Donkey, Samus, Luigi, Link, Yoshi, Captain, Kirby, Pikachu, Purin, Ness |
| 12–13 | Boss (Master Hand), MMario (Metal Mario) |
| 14–25 | Fighting Polygon Team (`NMario`…`NNess`) |
| 26 | GDonkey (Giant DK) |

### Where each original subsystem lands

| Original | Port |
|---|---|
| `sys/main.c` | `psp-game/src/main.rs`, `psp-runtime::thread` |
| `sys/scheduler.c` | dropped |
| `sys/taskman.c` | heap logic → planned arenas; rest dropped |
| `sys/objman.c`, `objanim.c` | `ssb-game` |
| `sys/objdisplay.c` | traversal → `ssb-game`; emission → `psp-runtime::meshdraw`, `gu` |
| `sys/matrix.c`, `vector.c` | `ssb-engine::math` |
| `sys/dma.c` | `ssb-rom::vpk0` |
| `sys/controller.c` | `ssb-engine::input` + `psp-runtime::input` |
| `sys/audio.c` | `ssb-engine::audio` (`system::AudioSystem`), thread in `psp-runtime::audio` |
| `sys/video.c`, `rdp.c` | `psp-runtime::gu` |
| `lb/lbreloc.c` | `ssb-rom::archive`, `reloc_link` |
| `lb/lbbackup.c` | `ssb-game::backup`, `psp-runtime::savedata` |
| `ft/*`, `wp/*`, `it/*`, `ef/*`, `gr/*`, `gm/*` | `ssb-game` |
| `mn/*`, `sc/*`, `mv/*` | `ssb-game::menu`, `spgame`, `opening` and select/results modules; drawn by `psp-game` |
| `libultra/*` | dropped |
