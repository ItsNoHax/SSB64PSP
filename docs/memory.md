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
  - tick 2 of boot: audio heads + 48-slot page cache (1.94 MB), never freed; samples stream from the pack ([D-049](decisions/D-049.md), RE-573, RE-582)
  - scene start: each scene's files with extern closure
  - runs `ssb_rom::loader_plan::Plan`: closure, `reloc_link::place_runs` Ascending runs, per-run blocks (RE-571)
  - `battle` and `opening` scenes: fighter figatrees stream per status; 20 pinned slots per kind plus a lazy 64 KiB LRU slab per anim kind, sync sector-superset read on a miss (`stream_pool.rs`, `scene_files.rs` `stream_miss`; RE-572, RE-573)
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
  - covers the next scene's FuncStart loads only; loads made on input (Maps preview) happen on that input, as on the N64 (RE-572)
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
- Allocation failure: large loads allocate fallibly; an infallible one reaches rust-psp's alloc error handler → `__psp_external_alloc_error` (`psp-runtime/src/heap.rs`) → `memory::alloc_failed`: fatal screen with size, align, scene, thread
- Battle: 0 allocations after scene load except FxPool high-water grows (first-use object-pool growth as the N64, scvsbattle.c:24, objman.c:686-706; SmallHeap slack, no kernel call)
  - streamed-figatree slabs and bookkeeping made eagerly at the battle's first safe point (`scene_files::make_groups`)
  - magnifier depth-mask lists from the frame arena (`depth_mask::scratch_bytes`, `MAGNIFY_SCALE_MAX`)
- No allocation in per-frame paths
- Bounded lists: `ssb_engine::array_vec::ArrayVec`
- `profile` `allocs`: per-frame count

### Stack

- Main thread: 512 KiB (`GAME_STACK_BYTES`, `psp-game/src/main.rs:2127`; `psp::module!` argument, RE-469)
  - why: 256 KiB default overflowed; PPSSPP does not detect it
- Largest battle-entry frame: `PosedFighter::set_damage_detect_size` (29 KB, the aside `Snapshot`); scene state built in place (`box_in_place!`, `write_new` writers, `all_fields!` guard in `crates/ssb-engine/src/memory.rs`), never by value (`panic = unwind` blocks copy elision)
- Optional payload built in place: `ssb_engine::memory::InlineOption` (bool tag + `MaybeUninit`; `write_with`), used for the results wipe `Transition`
- Other frames over 16 KB: `new_session` 32 KB, `init_weapon_pool` 25 KB, `enter_training` 25 KB, `opening_screen::start` and `players_screen::start` 18 KB
- Frame stack pinned: `psp_main` enters `run` at d-cache way offset 0x1600 (`stack_pin::enter`); re-screen with `tools/icache/stack-set.py` when `.bss` pin or map changes (RE-583)
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
  - `Arena` → `Scope` (`begin`, drop releases; `checkpoint` nests, drop rewinds); typed allocs reject drop glue at compile time
  - `AssetArena`: block table over a `BlockAlloc` (PSP: `CacheLineBlocks`)
  - `ObjectPool<T, N>`: serial-checked handles (serial bumps at insert); `FreeOrder::Lifo` = original item/weapon free lists; users: `ssb_game::item::ItemPool` (16), `ssb_game::weapon::WeaponPool` (32); `FxPool` of 70 = N64 `EFFECT_ALLOC_NUM` 38 + `WEAPON_ALLOC_NUM` 32 (`efdef.h:4`, `efmanager.c:1762`); item handle `u32` = `slot | serial << 16`
- `psp-runtime`: `AssetArena`; `FrameArena` (static 4 KiB 64-byte-aligned `.bss`, `memory::frame_arena`)

| Allocator | Lifetime |
|---|---|
| `GameArena` | One kernel block per battle scene (`BATTLE_SCENE_BYTES` by CPU count: 691,707 / 729,186 / 766,665 / 804,144 B), taken at scene entry, freed at exit; holds `BattleWorld` and CPU fighters as `SceneBox`es (drop glue runs; `end` refuses while boxes live); overflow → `memory::fatal` (N64 `syTaskmanInitGeneralHeap`). `ManagerCounts` are statics, as in the original. Objects ≤32 KiB stay on SmallHeap chunks (never returned). Peak metric: `peak_use` is relative to boot free; compare absolute `low_free` (battle `peak_use` +12,288 B is `boot_free` rising from a 20,956 B smaller ELF) |
| `AssetArena` | One block per read run (≤ 4 MiB load, ≤ 1 MiB prefetch), refcounted by its files; freed at the GE safe point after its last file drops |
| `FrameArena` | One `Scope` per tick (`psp-game` loop); users: movie camera draw order, magnifier depth-mask lists (2,923 B of 4,096 at scale 3.0) |
| `ObjectPool<T>` | Fixed capacity; free order `LowestFirst` or `Lifo`; users: items, weapons |

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
