# Original Game Architecture

How Super Smash Bros. 64 is structured, from
[`ssb-decomp-re`](https://github.com/VetriTheRetri/ssb-decomp-re). The
decompilation is 100% matched (7,165/7,165 functions, code and data, US and
JP), so questions about original behavior are answered from source, not
guessed.

## 1. ROM

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

## 2. Threads

`src/sys/main.c`:

```
thread 1 idle
└─ thread 5 (pri 50)  main game thread → scManagerRunLoop()
   ├─ thread 3 (pri 120) sySchedulerThreadMain   RCP task scheduler
   ├─ thread 4 (pri 110) syAudioThreadMain       audio synthesis
   └─ thread 6 (pri 115) syControllerThreadMain  controller
```

The game thread has the lowest priority. The port keeps that relationship:
audio and display must not starve behind a slow simulation frame. Thread
stacks carry a `0xFEDCBA98` canary checked by `syMainVerifyStackProbes`.

| N64 | PSP |
|---|---|
| `osCreateThread` / `osStartThread` | `sceKernelCreateThread` / `sceKernelStartThread` |
| `OSMesgQueue` / `osRecvMesg(BLOCK)` | message pipe or ring / `sceKernelWaitSema` |
| VI retrace | `sceDisplayWaitVblankStart` |
| PI cart DMA | file I/O on the asset pack |
| SI controller thread | `sceCtrlReadBufferPositive` |

The RCP scheduler (`scheduler.c`, triple-buffered, preemptible task queue) is
dropped. `sceGuStart`/`sceGuFinish`/`sceGuSync` cover submit-and-wait.

## 3. Objects

`src/sys/objtypes.h`, `objman.c`, `objdisplay.c`. Everything on screen is a
`GObj`.

| Type | Role |
|---|---|
| `GObj` | Generic object: id, link/priority lists, run and display callbacks, camera mask, payload |
| `DObj` | Transform-hierarchy node (translate/rotate/scale, parent/child/sibling), display list, `AObj` channels |
| `MObj` | Material: texture, palette, prim/env/blend colour, UV scroll, light colours |
| `AObj` | One animation channel |
| `CObj` | Camera: projection and draw-layer mask |

- A fighter is a `DObj` tree. `objdisplay.c` walks it with `gSPMatrix` push/pop.
- `DObj::dl` points to an F3DEX2 display list stored in the ROM.
- `GObj::camera_mask` selects draw layers per camera.
- `DObjDistDL` selects a display list by camera distance; `sGCDetailLevel`
  picks a global tier.
- Animation scripts: `AObjEvent16` (fighter figatrees) and `AObjEvent32`
  (everything else).
- `GObjProcess` may run a coroutine on its own libultra thread. The port uses
  explicit state machines instead.

## 4. relocData archive

`src/lb/lbreloc.c`. Implemented in `crates/ssb-rom/src/archive.rs`.

9.07 MiB at ROM `0x1AC870`, 2,132 files (US): models, display lists, textures,
palettes, animations, fighter attributes.

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

Loading (`lbRelocLoadAndRelocFile`):

1. Read `compressed_size` words; VPK0-decode if compressed.
2. Walk the intern chain. Each slot holds `{u16 next, u16 target_word}` and is
   overwritten with `base + target_word * 4`.
3. Walk the extern chain the same way. Target file IDs are a `u16` array
   after the file data, consumed in chain order.

US archive: 61,343 intern and 3,092 extern slots.

### VPK0

`syDmaDecodeVpk0` in `src/sys/dma.c`. LZ77 with two Huffman trees. 499 of
2,132 files are compressed (9.07 → 16.29 MiB).

- Trees are stored postfix: `0` pushes a leaf, `1` pops two and pushes a
  node; `1` with fewer than two nodes ends the tree.
- A leaf value is a bit width: read that many more bits for the value.
- `sample_method` selects the distance encoding. Two-sample form:
  `src = dst - value*4 - correction + 8`.

## 5. Rendering

`objdisplay.c`, `rdp.c`, `video.c`, `grdisplay.c`. 320×240, F3DEX2 (`G_VTX`
is `0x01`; `G_MTX`'s parameter byte is inverted relative to F3DEX).

Commands the game emits from `objdisplay.c`: `gSPMatrix`/`gSPPopMatrix`,
`gSPDisplayList`/`gSPBranchList`, `gSPSegment(0xE)`, texture and TLUT loads,
prim/env/blend colours, two-light `gSPLightColor`, `gSPTexture`.
Material textures are RGBA16/32 and CI4/CI8 with 16- or 256-entry TLUTs.

The port converts display lists at build time and does not emulate the RDP.
See [`rendering.md`](rendering.md).

## 6. Fighters and physics

`src/ft/`: `ftphysics.c`, `ftmain.c`, `fttypes.h`, `ftchar/ft*`.

- Physics is `f32`, not fixed point. `+Y` is up.
- `Z` is a shallow depth axis clamped to ±60 by trimming velocity.
- Ground (`vel_ground`, X only), air (`vel_air`) and `vel_knockback` are
  separate, with explicit transfer on takeoff and landing.
- Per-character tuning is data (`FTAttributes`): gravity, terminal
  velocities, air acceleration, traction, dash/run speed, weight, jumps,
  shield and shadow size, camera offsets, SFX IDs.
- Floor material scales traction:
  `dMPCollisionMaterialFrictions[floor_flags & MAP_VERTEX_MAT_MASK] * attr->traction`.

`FTKind` ordinals index asset tables and must not be renumbered:

| Ordinal | Fighters |
|---|---|
| 0–11 | Mario, Fox, Donkey, Samus, Luigi, Link, Yoshi, Captain, Kirby, Pikachu, Purin, Ness |
| 12–13 | Boss (Master Hand), MMario (Metal Mario) |
| 14–25 | Fighting Polygon Team (`NMario`…`NNess`) |
| 26 | GDonkey (Giant DK) |

## 7. Audio

libultra n_audio: 47 compressed-MIDI sequences (`S1_music.sbk`), two VADPCM
sample banks (117 and 322 waveforms), the FGM SFX engine, synthesized by RSP
microcode on its own thread.

Port plan: convert sequences and decode VADPCM at build time, mix in software
on a dedicated `sceAudio` thread. One 1024-sample PSP block (~23 ms) outlasts
a 16.67 ms frame, so mixing cannot run inline in the frame loop.

## 8. Memory

`syTaskmanMalloc(size, align)` in `taskman.c`. The relocData loader sizes a
scene's whole dependency closure with `lbRelocGetAllocSize`, then makes one
allocation. The port keeps that pattern; see [`memory.md`](memory.md).

## 9. Where each subsystem lands

| Original | Port |
|---|---|
| `sys/main.c` | `psp-game/src/main.rs`, rewritten |
| `sys/scheduler.c` | dropped |
| `sys/taskman.c` | heap logic → planned arenas; rest dropped |
| `sys/objman.c`, `objanim.c` | `ssb-game` |
| `sys/objdisplay.c` | traversal → `ssb-game`; emission → `psp-runtime::meshdraw`, `psp-runtime::gu` |
| `sys/matrix.c`, `vector.c` | `ssb-engine::math` |
| `sys/dma.c` | `ssb-rom::vpk0` |
| `sys/controller.c` | `ssb-engine::input` + `psp-runtime::input` |
| `sys/audio.c` | `ssb-engine` traits; no PSP backend yet |
| `sys/video.c`, `rdp.c` | `psp-runtime::gu` |
| `lb/lbreloc.c` | `ssb-rom::archive` |
| `ft/*` | `ssb-game` fighter, physics, status |
| `gr/*` | `ssb-game` stage |
| `gm/gmcollision.c` | `ssb-game::collision` |
| `mn/*`, `sc/*` | future menus and scene management |
| `libultra/*` | dropped |

## 10. Comparison with other ports

| Concern | SSB64 (N64) | sf64-psp | n64psp | This port |
|---|---|---|---|---|
| Language | C (IDO) | C | C | Rust |
| Approach | native | decomp + PSP backend | reusable N64→PSP runtime | decomp translation |
| Rendering | F3DEX2 → RDP | runtime DL → GU | backend registration only | build-time DL → PSP mesh |
| Audio | RSP `aspMain` | PSP audio | none | build-time convert + software mixer |
| Threading | 5 libultra threads | PSPSDK | PSPSDK | game + audio |
| Layering | monolithic | game + compat layer | runtime / bridge / backend | game / engine / PSP runtime |

`n64psp` contributes the layering shape: the runtime never knows the game, and
the graphics backend is registered, not hardcoded. `sf64-psp` translates
display lists at runtime because Star Fox 64 builds them dynamically; Smash's
are static ROM data, so this port preconverts them.
