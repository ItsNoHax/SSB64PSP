# Memory

## Budget

| | N64 | PSP-1000 | PSP-2000 and later |
|---|---|---|---|
| Main RAM | 4 MiB (8 with Expansion Pak) | 32 MiB | 64 MiB |
| Video memory | shared | 2 MiB VRAM | 2 MiB VRAM |
| Fast scratch | 4 KiB TMEM | 16 KiB scratchpad | 16 KiB scratchpad |

- Scenes load their archive files (RE-475, [D-046](decisions/D-046.md)).
  The pack (v107) holds a shared region and then each ROM archive file's
  bytes; the game reads the header, tables and shared region at boot
  (about 5.5 MB) and each scene's files, with their extern closure, when
  the scene starts (`ssb_psp_runtime::scene_files`). Every scene measured
  under PPSSPP uses at most about 17 MB in all (the VS results, which hold
  all twelve fighters; four-player battles about 16 MB); the whole pack
  needed 48 to 51 MB.
  `MEMSIZE=1` stays set. The menus' and the opening's own content is in
  the pack; `ssb64-menus.pak` and the opening models pack (RE-461, RE-467)
  are gone.
- From the XMB, a PSP-2000 with 6.61 ARK gives the game 47,891,712 free
  bytes at `psp_main`, 4 MiB less than under PSPLink (RE-475). The
  `memory_ballast` feature reproduces any such budget under PSPLink or
  PPSSPP.
- `psp`'s allocator takes each Rust allocation from
  `sceKernelAllocPartitionMemory` and its allocation-error handler spins
  forever: an allocation failure is a black screen and a hung HOME exit.
  Large loads therefore allocate fallibly and end on
  `ssb_psp_runtime::memory::fatal`'s text screen.
- While a scene runs, the next scene's files are read into memory beside
  it (`scene_files::prefetch`, RE-476): the attract loop's lowest free
  memory at the XMB's budget is 24.9 MB under PPSSPP. A prefetch stops
  queuing reads with less than 6 MB free.
- VRAM (2 MiB): two 32-bit colour buffers, the 16-bit depth buffer and
  the 512 KiB copy of the wallpaper photo the frozen picture samples
  (RE-476); about 180 KiB free.
- PSP-1000 ignores `MEMSIZE=1` (RE-288). With about 18 MB free at boot
  (PPSSPP, `memory_ballast`), the heaviest scenes measured fit with
  2–4 MB left except the VS results, which end on the out-of-memory
  screen (RE-475).
- The stage `MaterialAnimator` allocates one 448-byte joint per
  `MatAnimDesc` on the heap (103, 46 KiB; RE-322).
- The game's main thread has a 512 KiB stack (`module_with_stack!`,
  RE-469); battle entry peaks at 239–294 KB. `psp::module!`'s fixed
  256 KiB overflowed, which PPSSPP does not detect.
- CPU time, not RAM, is the main constraint. Trade memory for CPU:
  preconverted assets, cached meshes, no runtime decompression.

## VRAM

```
Framebuffer 0   512×272×4   544 KiB
Framebuffer 1   512×272×4   544 KiB
Depth buffer    512×272×2   272 KiB
Texture pool    remainder  688 KiB (704,512 bytes)
```

Paletted textures keep the pool usable: a 64×64 CI4 texture is 2 KiB versus
16 KiB as RGBA8888 ([D-003](decisions/D-003.md)).

The archive-wide texture set is larger than the pool, and so are some
scenes. `romtool scene-deps` over pack v37 (RE-341), counting every packed
mip level and CLUT:

| Scene | Bytes |
|---|---:|
| Training (Dream Land, two Marios) | 171.2 KiB |
| Hyrule Castle alone | 915.3 KiB |
| Donkey Kong, one costume | 531.3 KiB |
| Worst VS scene: Hyrule Castle + four Donkey Kong costumes | 3,024.6 KiB |

Training fits the pool; four-player VS scenes do not, so they need
per-scene residency (textures sampled from main RAM or streamed).

RE-426 measures v78 Dream Land with Mario, Fox, DK and Kirby costume 0,
including the VS wallpaper, all reachable model/texture parts and skeletons:
high detail needs 1,051,532 bytes (103 textures), low 726,924 (98).
The original three/four-fighter low-detail rule saves 324,608 bytes (30.9%),
but the low closure still exceeds the pool by 22,412. This is packed
residency demand, not a physical-PSP allocation measurement; the runtime
samples textures directly from the pack in main RAM.

The production game's largest frame is `enter_training`: it builds
`GroundObjects`, including Sector Z's Arwing, by value. Preview model
states are heap-owned. Current frame sizes and the remaining 256 KiB
main-thread stack budget live in [STATUS.md](../STATUS.md) and RE-436.

## Original pattern

`lbreloc.c` computes the size of a scene's whole dependency closure, makes one
allocation, and loads everything into it:

```c
lbRelocLoadFilesExtern(file_ids, count, out_ptrs,
    syTaskmanMalloc(lbRelocGetAllocSize(file_ids, count), 0x10));
```

Contiguous, unfragmented, one free. The port keeps the semantics, not the
addresses: assets are position-independent through relocations.

## Allocators

`ssb_engine::memory` implements them over borrowed storage (RE-340):
`Arena` scopes back `AssetArena`, `GameArena` and `FrameArena`, and
`ObjectPool<T, N>` gives generation-checked handles. `psp-runtime` does not
use them yet.

| Allocator | Lifetime |
|---|---|
| `GameArena` | Long-lived game state; freed on scene change |
| `AssetArena` | One contiguous block per scene, sized by the dependency closure |
| `FrameArena` | Bump allocator reset every tick; no heap allocation in per-frame paths |
| `ObjectPool<T>` | Fixed capacity for fighters, items, particles, matching original counts |

Scratchpad is reserved for VFPU staging and hot loops, after profiling.

## Extern relocations

`romtool` leaves extern relocations zeroed in the pack and records them in the
manifest ([D-011](decisions/D-011.md)). `ssb_rom::reloc_link` implements
the loader's layout and patching over `Archive::load_closure` output
(RE-340). Its layout matches `lbRelocGetAllocSize` and the original's live
RDRAM placement, resident files included (RE-341); no runtime path calls it
yet. It:

1. Computes the scene's file closure.
2. Assigns each file an offset in the asset arena.
3. Applies intern relocations (rebase) and extern relocations (assigned
   offsets).

## Cache coherency

The PSP data cache is write-back. Anything the GE reads by DMA (vertices,
textures, display lists) must be flushed with
`sceKernelDcacheWritebackRange` or written through an uncached pointer.
`psp::Align16` handles alignment, not coherency. Missing flushes cause
intermittent corruption that looks like a race.

Statics the GE reads (`DISPLAY_LIST`, `TRANSITION_PHOTO`,
`WALLPAPER_PHOTO` in `psp-runtime/src/gu.rs`) use 64-byte
`CacheLineAligned` storage, a whole number of lines long. A 16-byte
aligned buffer can share a D-cache line with a cached `.bss` neighbour,
and that line's writeback overwrites GE-visible data (RE-360). The two
capture buffers are CPU-filled and written back after each capture
(RE-361).
