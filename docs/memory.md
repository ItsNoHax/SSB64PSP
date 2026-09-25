# Memory

## Budget

| | N64 | PSP-1000 | PSP-2000 and later |
|---|---|---|---|
| Main RAM | 4 MiB (8 with Expansion Pak) | 32 MiB | 64 MiB |
| Video memory | shared | 2 MiB VRAM | 2 MiB VRAM |
| Fast scratch | 4 KiB TMEM | 16 KiB scratchpad | 16 KiB scratchpad |

- The current asset pack (v36, with Samus and Luigi slots) is 22.6 MB. It needs
  `PARAM.SFO` `MEMSIZE=1` (64 MiB process mode). v34 ran on a PSP-2000 via
  PSPLink (RE-322), v33 before it (RE-321); the earlier stock v32 pack loaded on a PSP-2000 via PSPLink (RE-320); the mode was also
  confirmed with earlier packs (RE-255, RE-260).
- PSP-1000 ignores `MEMSIZE=1`; the full pack does not fit (RE-288). Support
  needs a per-scene or reduced pack.
- The stage `MaterialAnimator` allocates one 448-byte joint per
  `MatAnimDesc` on the heap (103, 46 KiB; RE-322).
- CPU time, not RAM, is the main constraint. Trade memory for CPU:
  preconverted assets, cached meshes, no runtime decompression.

## VRAM

```
Framebuffer 0   480×272×4   522 KiB
Framebuffer 1   480×272×4   522 KiB
Depth buffer    480×272×2   261 KiB
Texture pool    remainder  ≈700 KiB
```

Paletted textures keep the pool usable: a 64×64 CI4 texture is 2 KiB versus
16 KiB as RGBA8888 ([D-003](decisions/D-003.md)).

The archive-wide texture set is larger than the pool, but no scene needs all
of it. Dream Land plus the four largest playable fighters measured 217.1 KiB
of deduplicated textures (RE-076, RE-077). That covers 12 of 27 fighter kinds
and 1 of 41 stages, so re-measure once a scene dependency graph exists before
designing streaming. Build-time filter compensation (RE-305–313) has since
grown some textures; the per-scene figure has not been re-measured.

## Original pattern

`lbreloc.c` computes the size of a scene's whole dependency closure, makes one
allocation, and loads everything into it:

```c
lbRelocLoadFilesExtern(file_ids, count, out_ptrs,
    syTaskmanMalloc(lbRelocGetAllocSize(file_ids, count), 0x10));
```

Contiguous, unfragmented, one free. The port keeps the semantics, not the
addresses: assets are position-independent through relocations.

## Planned allocators

None is implemented yet.

| Allocator | Lifetime |
|---|---|
| `GameArena` | Long-lived game state; freed on scene change |
| `AssetArena` | One contiguous block per scene, sized by the dependency closure |
| `FrameArena` | Bump allocator reset every tick; no heap allocation in per-frame paths |
| `ObjectPool<T>` | Fixed capacity for fighters, items, particles, matching original counts |

Scratchpad is reserved for VFPU staging and hot loops, after profiling.

## Extern relocations

`romtool` leaves extern relocations zeroed in the pack and records them in the
manifest ([D-011](decisions/D-011.md)). The planned loader:

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
