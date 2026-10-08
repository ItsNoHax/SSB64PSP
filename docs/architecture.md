# Architecture

How SSB64PSP is built today and what it is built towards. Crate ownership
rules are in [`AGENTS.md`](../AGENTS.md); open work is in
[`PLAN.md`](../PLAN.md).

## Current Architecture

```
ROM (user-supplied) ──romtool pack──> assets/generated/ssb64.pak (gitignored)
                                           │ read at boot and per scene
psp-game ─────────┐                        ▼
                  ├──> psp-runtime ──┬──> crates/ssb-game    portable gameplay
psp-asset-viewer ─┘    (PSP only)    ├──> crates/ssb-engine  math, coord, timing, traits
                                     ├──> crates/ssb-rom     ROM, archive, formats, pack
                                     └──> crates/ssb-capture golden-capture scene specs
```

| Component | Role |
|---|---|
| `tools/romtool` | Verifies the ROM, decodes the relocData archive, converts display lists, textures, animations, sprites and tables, and writes the pack |
| `crates/ssb-rom` | ROM and archive readers (`archive`, `vpk0`), F3DEX2 decoding (`dl`), mesh and texture conversion (`mesh`, `texture`, `psp_texture`, `filter_compensation`), pack format (`pack`), scene file closures, `reloc_link` |
| `crates/ssb-engine` | Engine traits (renderer, input, timing), the N64 audio system (`audio`: synthesizer, sequence player, FGM engine, `syAudio`), math including the original's `lbCommonSin` table, the one N64 → PSP screen mapping (`coord`), allocators (`memory`) |
| `crates/ssb-game` | Gameplay translated from the decomp: fighters, statuses, motion scripts, combat, collision, items, weapons, effects, stages, CPU, camera, HUD, menus, 1P campaign (`spgame`), opening, save data (`backup`) |
| `crates/ssb-capture` | Golden-capture scene specs shared by both PSP binaries and host tests |
| `psp-runtime` | All PSP code: pack loading and per-scene files (`assets`, `scene_files`), GE setup and drawing (`gu`, `meshdraw`), input, timing, save file, profiler, the main thread, framebuffer transitions, movie and particle draws |
| `psp-game` | The game application: scene orchestration and screens that adapt portable state to packed assets |
| `psp-asset-viewer` | Asset browser and deterministic render-regression scenes |

- The simulation runs at a fixed 60 Hz with a catch-up cap, decoupled from
  rendering ([D-005](decisions/D-005.md)).
- Display lists are converted once at build time; the RDP is not emulated
  ([D-001](decisions/D-001.md), [D-002](decisions/D-002.md)).
- The pack is mandatory and built separately ([D-028](decisions/D-028.md)).
  At boot the game reads its header, tables and shared region (~5.5 MB);
  each scene reads its archive files and their extern closure when it
  starts, and the next scene's files are read in the background where the
  next scene is known ([D-046](decisions/D-046.md), RE-475, RE-476).
- Extern relocations are zeroed in the pack; the converter follows them
  ([D-011](decisions/D-011.md)).
- Save data is the N64 SRAM image in `ssb64.sav` beside the pack
  ([D-045](decisions/D-045.md)).
- The game's main thread has a 512 KiB stack (RE-469).
- The PSP crates sit outside the Cargo workspace and build with a pinned
  nightly and `-Z build-std` ([D-026](decisions/D-026.md)); portable crates
  are `no_std` by default ([D-027](decisions/D-027.md)).

## Target Architecture

| Area | Current | Target |
|---|---|---|
| Asset loading | Scene files read whole into heap allocations; `reloc_link` unused at runtime | A runtime loader on `ssb_rom::reloc_link`, laying out each scene's closure as `lbRelocGetAllocSize` does |
| Memory | `psp`'s allocator over `sceKernelAllocPartitionMemory`; fallible large loads | `ssb_engine::memory` arenas (`GameArena`, `AssetArena`, `FrameArena`) and pools in `psp-runtime` ([memory](memory.md#allocators)) |
| Textures | Sampled from the pack in main RAM | Per-scene VRAM residency where it measurably helps |
| Audio | The N64 audio system ported whole, synthesized at run time on its own thread ([D-049](decisions/D-049.md)) | Same, within the PSP-2000's CPU budget |
| Optimization | Scalar math, GE state cache | VFPU and state batching only where profiling shows a cost ([D-032](decisions/D-032.md), [D-036](decisions/D-036.md)) |
| Debug HUD | `sceGuDebugFlush` (PPSSPP software renderer only) | GE geometry |

## Major Subsystems

### Rendering

`ssb-rom` turns each relocData display list into PSP vertex buffers and GE
state; `psp-runtime::meshdraw` draws them through a GE state cache. Domain
detail, deviations and validation are in [`rendering.md`](rendering.md).

### Gameplay

`ssb-game` mirrors the decomp's `ft/`, `wp/`, `it/`, `ef/`, `gr/`, `gm/`,
`if/`, `mn/` and `sc/` modules. Data the original keeps in its code segment
(motion scripts, colour-animation scripts, CPU input scripts, status tables,
animation file tables) is read from the user's ROM at build time by
`crates/ssb-tablegen` and never committed ([D-048](decisions/D-048.md));
data that lives in the ROM's archive (models, animations, `FTAttributes`,
collision, sprites) comes from the pack. Objects use explicit state
machines in place of the original's `GObjProcess` coroutine threads.

### Asset Pipeline

`romtool pack` reads the ROM, decompresses VPK0, relocates archive files and
converts them into pack v107: one shared region and then each archive
file's bytes. Paletted textures stay paletted ([D-003](decisions/D-003.md)).
`romtool` also hosts the ROM-backed audits (`verify`, `scene-deps`,
`residuals`, `matcolors`, `texgen`).

### Audio

[D-049](decisions/D-049.md). The original's audio is three layers, all
ported: libultra `n_audio` (the synthesizer and the compressed-sequence
player), the FGM sound-effect engine in `n_env.c`, and the `syAudio` layer
of `src/sys/audio.c`.

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

The game calls the API synchronously; `SharedAudio` serialises those calls
with the audio thread's frames under a kernel semaphore, as the N64 masks
interrupts around the same list operations. Each frame runs the
sequencer's and the FGM engine's handlers, then mixes 184-sample
sub-frames. The synth runs at the N64's 32006 Hz; the PSP plays it at
32000 Hz.

### Frontend

`ssb-game::menu`, `opening`, `spgame` and the select and results modules
hold the scene logic; `psp-game`'s `*_screen.rs` modules draw it from packed
sprites and models. The scene manager follows `scManagerRunLoop`: the N64
logo, opening, title and attract loop, menus, selects, battles, results and
the 1P campaign.

### Platform (PSP)

| N64 | PSP |
|---|---|
| `osCreateThread` / `osStartThread` | `sceKernelCreateThread` / `sceKernelStartThread` |
| VI retrace | `sceDisplayWaitVblankStart` |
| PI cart DMA | file reads of the pack (`sceIoRead`, `sceIoReadAsync`) |
| SI controller thread | `sceCtrlReadBufferPositive` |
| RCP task scheduler | dropped; `sceGuStart`/`sceGuFinish`/`sceGuSync` |
| SRAM | `ssb64.sav` |

Memory, VRAM and cache-coherency rules are in [`memory.md`](memory.md). The
EBOOT targets PSP-2000 and later with `MEMSIZE=1`; a PSP-1000 ignores it
(RE-288).

## Key Boundaries

- `ssb-game`, `ssb-engine` and `ssb-rom` never depend on `psp-runtime`
  ([D-044](decisions/D-044.md)).
- `psp-runtime` holds no gameplay logic; `psp-game` holds no gameplay or PSP
  backend code; `psp-asset-viewer` holds no game logic.
- ROM-derived assets exist only in the gitignored pack
  ([D-033](decisions/D-033.md)); the EBOOT never contains it.
- `unsafe` only for PSP APIs, VFPU and GE memory ([D-031](decisions/D-031.md)).

## Original Game Reference

From [`ssb-decomp-re`](https://github.com/VetriTheRetri/ssb-decomp-re),
100% matched (7,165/7,165 functions, US and JP).

### ROM

| | |
|---|---|
| Internal name | `SMASH BROTHERS` (0x20) |
| Game code | `NALE` (0x3B) |
| SHA-1 | `e2929e10fccc0aa84e5776227e798abc07cedabf` |
| MD5 | `f7c52568a31aadf26e14dc2b6416b2ed` |
| Format | 16 MiB big-endian `.z64` |
| Graphics microcode | F3DEX2 (`gspF3DEX2_fifo`) |
| Audio microcode | `n_aspMain` |

Only the US ROM is supported. JP needs its archive constants read from the
decomp's JP linker script and checked against a real dump.

### Threads

```
thread 1 idle
└─ thread 5 (pri 50)  main game thread → scManagerRunLoop()
   ├─ thread 3 (pri 120) sySchedulerThreadMain   RCP task scheduler
   ├─ thread 4 (pri 110) syAudioThreadMain       audio synthesis
   └─ thread 6 (pri 115) syControllerThreadMain  controller
```

The game thread has the lowest priority: audio and display must not starve
behind a slow simulation frame.

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

`src/lb/lbreloc.c`; implemented in `crates/ssb-rom/src/archive.rs`. 9.07 MiB
at ROM `0x1AC870`, 2,132 files (US).

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

1. Read `compressed_size` words; VPK0-decode if compressed.
2. Walk the intern chain: each slot holds `{u16 next, u16 target_word}` and
   becomes `base + target_word * 4`.
3. Walk the extern chain the same way; target file IDs are a `u16` array
   after the file data, consumed in chain order.

US archive: 61,343 intern and 3,092 extern slots ([D-010](decisions/D-010.md)).

### VPK0

`syDmaDecodeVpk0` in `src/sys/dma.c`: LZ77 with two Huffman trees. 499 of
2,132 files are compressed (9.07 → 16.29 MiB) ([D-009](decisions/D-009.md)).

- Trees are postfix: `0` pushes a leaf, `1` pops two and pushes a node; `1`
  with fewer than two nodes ends the tree.
- A leaf value is a bit width: read that many more bits for the value.
- `sample_method` selects the distance encoding. Two-sample form:
  `src = dst - value*4 - correction + 8`.

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
| `sys/audio.c` | `ssb-engine::audio` trait; no PSP backend |
| `sys/video.c`, `rdp.c` | `psp-runtime::gu` |
| `lb/lbreloc.c` | `ssb-rom::archive`, `reloc_link` |
| `lb/lbbackup.c` | `ssb-game::backup`, `psp-runtime::savedata` |
| `ft/*`, `wp/*`, `it/*`, `ef/*`, `gr/*`, `gm/*` | `ssb-game` |
| `mn/*`, `sc/*`, `mv/*` | `ssb-game::menu`, `spgame`, `opening` and select/results modules; drawn by `psp-game` |
| `libultra/*` | dropped |
