# Memory Architecture

## The good news

| | N64 | PSP-1000 | PSP-2000/3000 |
|---|---|---|---|
| Main RAM | 4 MiB (8 with Expansion Pak) | **32 MiB** | **64 MiB** |
| Video memory | shared with main | **2 MiB VRAM** | 2 MiB |
| Fast scratch | 4 KiB RDP TMEM | **16 KiB scratchpad** | 16 KiB |

The PSP has 8x the RAM of a stock N64 and a dedicated 2 MiB VRAM. The entire
`relocData` archive decompresses to **16.29 MiB** — meaning that on a
PSP-2000/3000 the whole game's asset set could in principle be resident at
once, and even on a PSP-1000 a generous working set fits.

**Memory pressure is not expected to be this port's binding constraint.** CPU
time is. Design accordingly: prefer trading memory for CPU (preconverted
assets, cached meshes, no runtime decompression) rather than the reverse.

The one place the N64 wins is TMEM's 4 KiB of texture cache, and that only
matters if you are streaming textures per-primitive the way the RDP does. We
are not — PSP textures live in VRAM.

## How the original manages memory

`src/sys/malloc.c` is 41 lines; the real allocator is
`syTaskmanMalloc(size, align)` in `taskman.c`. The pattern that matters is in
`lbreloc.c`:

```c
lbRelocLoadFilesExtern(
    file_ids, count, out_ptrs,
    syTaskmanMalloc(lbRelocGetAllocSize(file_ids, count), 0x10)
);
```

That is: **compute the size of the whole dependency closure up front, make one
allocation, load everything into it.** `lbRelocGetExternBytesNum` recurses
through extern references to total it.

This is a good pattern and transfers directly. It gives contiguous assets, no
fragmentation, and a single free.

## Planned PSP layout

```
Main RAM
├── Game arena        game state, fighters, scene graph        (static size)
├── Asset arena       loaded relocData closure for the current
│                     scene, laid out contiguously             (per-scene)
├── Render arena      CPU-side mesh/material records
├── Frame arena       per-frame scratch, reset every tick      (bump alloc)
└── Audio arena       decoded samples, sequencer state

VRAM (2 MiB)
├── Framebuffer 0     480x272x4 =  522 KiB
├── Framebuffer 1     480x272x4 =  522 KiB
├── Depth buffer      480x272x2 =  261 KiB
└── Texture pool      remainder ≈ 700 KiB

Scratchpad (16 KiB)
└── Reserved for VFPU staging / hot inner loops, after profiling
```

Framebuffers and depth already consume ~1.3 MiB of the 2 MiB VRAM, leaving
roughly 700 KiB for textures. That is the real constraint, and it is why CI4/CI8
paletted textures matter (see `docs/rendering.md`) — a 64x64 CI4 texture is
2 KiB where RGBA8888 would be 16 KiB.

The packed texture set currently measures **1059 KiB**, 1.5x the ~700 KiB
budget (RE-067 added mirrored-texture pre-baking on top of RE-053's mip
chains, both correctness fixes that cost real VRAM). Every texture cannot
be resident at once; texture streaming (`TODO.md` Phase G, "scene-aware
residency") is not yet implemented and is no longer optional headroom —
it is required for the game to run within the PSP's real VRAM budget.

## Allocator plan

Per plan §11, explicit pools rather than a general heap:

* `GameArena` — long-lived, freed on scene change.
* `AssetArena` — one contiguous block per scene, sized by the dependency
  closure, mirroring the original's approach.
* `FrameArena` — bump allocator, reset every tick. **No heap allocation in
  per-frame hot paths.**
* `ObjectPool<T>` — fixed-capacity pools for fighters, items, particles,
  matching the original's fixed object counts.

None of these are implemented yet.

## Extern relocations and layout

The archive's extern relocations point *between* files, so their final values
depend on where each file lands in RAM. `romtool` deliberately leaves them
unresolved (zeroed) and records them in the manifest instead. The runtime
loader will:

1. Compute the closure for a scene.
2. Assign each file an offset in the asset arena.
3. Apply intern relocations (rebasing the file-relative offsets already
   recorded) and extern relocations (using the assigned offsets).

This is the same three-step shape as `lbRelocLoadAndRelocFile`, just with the
addresses decided by us rather than by a DMA into a fixed heap.

## Cache

The PSP has a writeback data cache. Anything handed to the GE by DMA — vertex
buffers, textures, display lists — must either be flushed
(`sceKernelDcacheWritebackRange`) or written through an uncached pointer.
Forgetting this produces intermittent corruption that looks like a race.
`rust-psp`'s `Align16` handles alignment but not coherency; the mesh upload
path will need explicit flushes.

## Not to do

Per plan §29: do **not** try to emulate the N64's memory layout. Nothing in the
game depends on a specific physical address — the relocation system exists
precisely because assets are position-independent. Preserve the *semantics*
(closure sizing, contiguous placement, pointer patching), not the addresses.
