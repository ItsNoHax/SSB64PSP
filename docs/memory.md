# Memory

## Budget

| | N64 | PSP-1000 | PSP-2000 and later |
|---|---|---|---|
| Main RAM | 4 MiB (8 with Expansion Pak) | 32 MiB | 64 MiB |
| Video memory | shared | 2 MiB VRAM | 2 MiB VRAM |
| Fast scratch | 4 KiB TMEM | 16 KiB scratchpad | 16 KiB scratchpad |

### Main RAM

- Pack loading (`psp_runtime::scene_files`; RE-475, [D-046](decisions/D-046.md)):
  - boot: header, tables, shared region
  - boot: audio section, never freed (audio thread reads it any time, [D-049](decisions/D-049.md))
  - scene start: each scene's files with extern closure
  - `ssb_rom::reloc_link` (closure, placement, run planner) is host-tested; the runtime does not use `place_into` yet (RE-571)
- Scene peaks, boot and XMB free bytes (PSP-2000): RE-475
  - XMB launch: 4 MiB less than PSPLink
- Feature `memory_ballast`: reproduces any budget under PSPLink or PPSSPP
- `MEMSIZE=1` stays set
- PSP-1000:
  - ignores `MEMSIZE=1` (RE-288)
  - VS results exceed its budget: out-of-memory screen (RE-475)
- `scene_files::prefetch`:
  - reads the next scene's files beside the running scene (RE-476)
  - stops queuing reads under `PREFETCH_MARGIN`, 6 MiB (`scene_files.rs:55`)
- Static state (RE-534, RE-535):
  - texture bind-block cache: 1024 slots (`meshdraw.rs:808`)
  - texture-mapping cache: 256 slots (`MAP_CACHE_SLOTS`, `meshdraw.rs`)
  - `.bss` sizes: RE-534, RE-535
- Stage `MaterialAnimator`: heap-allocates one 448-byte joint per `MatAnimDesc` (RE-322)
- Measured free-memory minima: RE-475, RE-479, RE-535
- Current baseline: [STATUS.md](../STATUS.md)

### Allocation

- Global allocator: `psp_runtime::heap` (RE-484)
  - blocks up to 32 KiB (`ssb-engine/src/heap.rs:23`): size-class free lists over 256 KiB chunks (`psp-runtime/src/heap.rs:24`), taken once and kept
  - larger blocks: `sceKernelAllocPartitionMemory`
    - evicts the caller's cache lines
    - cost per allocate-and-free: RE-483
- rust-psp allocation-error handler spins forever (black screen, hung HOME exit)
  - large loads allocate fallibly
  - failure ends on `memory::fatal` text screen
- No allocation in per-frame paths
- Bounded lists: `ssb_engine::array_vec::ArrayVec`
- `profile` `allocs`: per-frame count

### Stack

- Main thread: 512 KiB (`GAME_STACK_BYTES`, `psp-game/src/main.rs:2127`; `psp::module!` argument, RE-469)
  - why: 256 KiB default overflowed; PPSSPP does not detect it
- Largest frame: `enter_training` (builds `GroundObjects` by value)
- Deepest measured stack: [STATUS.md](../STATUS.md)

### Constraint

- CPU time, not RAM, is the main constraint
- Trade memory for CPU: preconverted assets, cached meshes, no runtime decompression

## VRAM

Allocated in `Gpu::init` (`psp-runtime/src/gu.rs`), 2 MiB in total:

| Region | Size | Use |
|---|---:|---|
| Framebuffer 0 and 1 | 2 × 544 KiB (512×272×4) | Draw and display buffers |
| Depth buffer | 272 KiB (512×272×2) | 16-bit depth |
| Photo region (`PHOTO_REGION`) | 512 KiB (512×256×4) | Copy of the wallpaper photo the frozen picture samples (RE-476); lent to a large resident texture (`vram_resident`, RE-492) or, when idle for `TEXTURE_POOL_IDLE_FRAMES` (60, `gu.rs:714`), to the texture pool |
| Vertex region (`VRAM_VERTICES`) | rest, 176 KiB on a PSP-2000 | Per-frame lit-expand vertex blocks (`ge_vertices`), falling back to list memory |

- Texture pool (`TEXTURE_POOL`, RE-539):
  - `pool_texture` copies textures from main RAM into the idle photo region (`sceGuCopyImage`)
  - up to 384 copies (3/4 of `TEXTURE_POOL_SLOTS` 512, `gu.rs:710`), keyed by main-RAM texel address
  - dropped on residency generation change, or photo or wallpaper claim
- All other textures: sampled from the pack in main RAM
  - paletted textures stay paletted ([D-003](decisions/D-003.md))
- Vertex region (RE-486, RE-539):
  - GE reads it without the shared bus
  - CPU writes through the cached alias, writes back before the draw, never reads back
  - `reset_vertex_chunk` rewinds it at each `sceGuStart`
- Four-player VS: texture demand exceeds VRAM (RE-341, RE-426) → residency per scene, not whole-scene

Main-RAM statics used by the GE or the frozen picture:

| Static | Size | Note |
|---|---:|---|
| `DISPLAY_LIST` | 1 MiB | |
| `WALLPAPER_PHOTO` | 512 KiB | source of the photo copy |
| `TRANSITION_PHOTO` | 256 KiB | |

## Original pattern

`lbreloc.c`: sizes a scene's whole dependency closure, one allocation, loads everything into it.

```c
lbRelocLoadFilesExtern(file_ids, count, out_ptrs,
    syTaskmanMalloc(lbRelocGetAllocSize(file_ids, count), 0x10));
```

- Port keeps the semantics, not the addresses
- Assets are position-independent through relocations

## Allocators

- `ssb_engine::memory` (RE-340)
  - `Arena` scopes back `AssetArena`, `GameArena`, `FrameArena`
  - `ObjectPool<T, N>`: generation-checked handles
- `psp-runtime` does not use them yet

| Allocator | Lifetime |
|---|---|
| `GameArena` | Long-lived game state; freed on scene change |
| `AssetArena` | One contiguous block per scene, sized by the dependency closure |
| `FrameArena` | Bump allocator reset every tick; no heap allocation in per-frame paths |
| `ObjectPool<T>` | Fixed capacity for fighters, items, particles, matching original counts |

Scratchpad: reserved for VFPU staging and hot loops, after profiling.

## Extern relocations

- `romtool`: leaves extern relocations zeroed in the pack, records them in the manifest ([D-011](decisions/D-011.md))
- `ssb_rom::reloc_link` (RE-340, RE-341):
  1. takes `Archive::load_closure` output
  2. assigns each file an arena offset (layout of `lbRelocGetAllocSize`, original RDRAM placement)
  3. applies intern (rebase) and extern (assigned offset) relocations
- No runtime path calls it yet

## Cache coherency

- PSP data cache: write-back
- GE DMA reads (vertices, textures, display lists) need one of:
  - `sceKernelDcacheWritebackRange` flush
  - write through an uncached pointer
- `psp::Align16`: alignment only, not coherency
- Missed flush → intermittent corruption that looks like a race
- GE-visible statics (`DISPLAY_LIST`, `TRANSITION_PHOTO`, `WALLPAPER_PHOTO`):
  - 64-byte `CacheLineAligned` storage, whole number of lines
  - why: 16-byte-aligned buffer can share a D-cache line with a cached `.bss` neighbour; its writeback overwrites GE-visible data (RE-360)
- Capture buffers: CPU-filled, written back after each capture (RE-361)
