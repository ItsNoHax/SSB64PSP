# Super Smash Bros. 64 — Original Architecture

Findings from `VetriTheRetri/ssb-decomp-re`, recorded so that the PSP port can
be designed against how the game actually works rather than against
assumptions.

The single most important fact up front:

> **The decompilation is 100% complete.** 7165 / 7165 functions matched,
> 100% of code and 100% of data, for both US and JP. `us_report.json` confirms
> `matched_code_percent: 100.0`, `matched_functions: 7165`.

There is no reverse-engineering left to do on game behaviour. Every question
about what the original does has an answer in the source. The port's job is
platform translation, not discovery.

---

## 1. ROM baseline

| | |
|---|---|
| Internal name | `SMASH BROTHERS` (ROM 0x20) |
| Game code | `NALE` (ROM 0x3B) |
| SHA-1 | `e2929e10fccc0aa84e5776227e798abc07cedabf` |
| MD5 | `f7c52568a31aadf26e14dc2b6416b2ed` |
| Size | 16 MiB, big-endian `.z64` |
| Microcode | **F3DEX2** (`gspF3DEX2_fifo`, registered in `sys/taskman.c`) |
| Audio microcode | `n_aspMain` (libultra "n_audio") |

Verified against the user's dump: both hashes match exactly.

The JP revision is also 100% decompiled but is **not** supported by this port's
tooling yet — its archive constants must be read from the decomp's JP linker
script and checked against a real JP dump before being added.

---

## 2. Boot and thread architecture

`src/sys/main.c`. The game is a conventional libultra multi-threaded title:

```
syMainLoop
  └─ osInitialize
  └─ thread 1 (idle, pri APPMAX → IDLE)
       └─ thread 5 (pri 50)  ── the main game thread
            ├─ osCreateViManager, osCartRomInit, osCreatePiManager
            ├─ thread 3 (pri 120) sySchedulerThreadMain   ← RCP task scheduler
            ├─ thread 4 (pri 110) syAudioThreadMain       ← audio synthesis
            ├─ thread 6 (pri 115) syControllerThreadMain  ← SI / controller
            └─ scManagerRunLoop()                         ← scene manager
```

Two details worth carrying over:

* **Priority order is meaningful.** The scheduler (120) outranks the controller
  (115), which outranks audio (110), which outranks the game (50). The game
  thread is the *lowest* priority — it runs in whatever time the RCP-servicing
  threads leave. The PSP backend should preserve that relationship: audio and
  display must not be starved by a slow simulation frame.
* **Stack canaries.** Every thread stack is stamped with `0xFEDCBA98` at word 7
  and checked by `syMainVerifyStackProbes`. Cheap and worth keeping.

### PSP mapping

| N64 | PSP |
|---|---|
| `osCreateThread` / `osStartThread` | `sceKernelCreateThread` / `sceKernelStartThread` |
| `OSMesgQueue` | `sceKernelCreateMsgPipe` or a lock-free ring |
| `osRecvMesg(BLOCK)` | `sceKernelWaitSema` |
| VI manager, retrace event | `sceDisplayWaitVblankStart` |
| PI manager / cart DMA | file I/O against the converted asset pack |
| SI / controller thread | `sceCtrlReadBufferPositive` |

The scheduler thread has **no PSP equivalent and should not get one**. It
exists to feed RCP tasks to hardware we are not emulating.

---

## 3. The scheduler and RCP task model

`src/sys/scheduler.c` (1319 lines), `src/sys/scheduler.h`.

The game submits typed tasks to a priority queue:

```c
enum SYTaskType {
    nSYTaskTypeNone, nSYTaskTypeGfx, nSYTaskTypeAudio, nSYTaskTypeAddClient,
    nSYTaskTypeVi, nSYTaskTypeFramebuffers, nSYTaskTypeGfxEnd, nSYTaskTypeNoOp,
    nSYTaskTypeRdpBuffer, nSYTaskTypeCustomBuffer, nSYTaskTypeDefaultBuffer,
    SC_TASK_TYPE_11
};
```

`SYTaskGfx` wraps an `OSTask` plus a framebuffer index and an RDP buffer size.
Tasks can be queued, suspended and resumed (`nSYSchedulerStatusTaskSuspending`
etc.), because a graphics task that overruns its frame gets pre-empted by
audio.

Triple-buffered: `SYTaskFramebuffer` carries `void *framebuffers[3]`.

**Port decision.** Do not reproduce the task queue. On PSP, `sceGuStart` /
`sceGuFinish` / `sceGuSync` already provide the submit-and-wait primitive, and
audio runs on its own thread. The only thing worth preserving is the
*triple-buffering* and the fact that the game tolerates a frame taking longer
than a vblank.

---

## 4. Scene graph: the GObj system

`src/sys/objtypes.h`, `objman.c` (2442 lines), `objdisplay.c` (3394 lines).

This is the heart of the engine, and it is the part the port must understand
best. Everything on screen — fighters, stages, items, HUD, menus — is a `GObj`.

```
GObj  ── generic object; has an id, link/priority lists, a run function,
      │  a display function, a camera mask, and a payload
      └─ obj: NULL | DObj | SObj | CObj      (obj_kind selects)

DObj  ── "Draw Object": a node in a transform hierarchy
      │  translate / rotate / scale (each an XObj + vector)
      │  parent / child / sib_next / sib_prev
      │  a display list, or an array, or an LOD table, or a layer table
      └─ aobj: animation channels

MObj  ── "Material Object": texture, palette, prim/env/blend colour,
         UV scroll, light colours

AObj  ── one animation channel (track, kind, length, base/target value,
         interpolation)

CObj  ── camera; carries the projection XObj and a draw-layer mask
```

### Key structural facts

* **DObjs form a skeleton.** `parent`/`child`/`sib_next` is a joint hierarchy;
  `objdisplay.c` walks it pushing and popping the modelview matrix
  (`gSPMatrix(..., G_MTX_PUSH | G_MTX_MUL | G_MTX_MODELVIEW)` /
  `gSPPopMatrix`). A fighter is a DObj tree.
* **Geometry is not a mesh format.** `DObj::dl` is a pointer to an
  **F3DEX2 display list stored in the ROM**. `objdisplay.c` sets up state and
  then calls `gSPDisplayList(head++, dobj->dl)`. See §6.
* **Draw layers.** `GObj::camera_mask` is a 64-bit mask of "rooms";
  `COBJ_MASK_DLLINK(r)` builds it. A camera renders only the layers its mask
  selects. This is how the game separates background, stage, fighters and HUD.
* **LOD exists.** `DObjDistDL { f32 target_dist; Gfx *dl; }` selects a display
  list by camera distance, and `sGCDetailLevel` picks a global detail tier.
  The PSP port gets this for free and should use it.
* **Animation is script-driven.** `AObjScript` is a union of `AObjEvent16*`
  (fighters only — the compact format later games call "figatrees") and
  `AObjEvent32*` (everything else). Opcodes are bitfields:
  `{ opcode:5, flags:10, toggle:1 }` for 16-bit,
  `{ opcode:7, flags:10, payload:15 }` for 32-bit.
* **GObjs can own coroutines.** `GObjProcess` dispatches either a plain
  function or a `GObjThread` with its own libultra thread and stack. Fighter
  logic uses these. On PSP these should become explicit state machines rather
  than real threads — spawning an OS thread per object would be ruinous.

---

## 5. relocData: the asset filesystem

**This is the port's single most important subsystem**, and it is fully
understood. See `docs/reverse-engineering.md` for the derivation and
`crates/ssb-rom/src/archive.rs` for the implementation.

`src/lb/lbreloc.c` + `relocData.md`.

A 9.07 MiB region at ROM `0x1AC870` holding **2132 numbered files** (US):
sprites, animations, hitbox data, stage models, texture atlases, fighter
attribute tables, display lists, vertex arrays, palettes.

```
table_lo (0x1AC870) ──► [LBTableEntry; 2133]   ← 2132 files + 1 sentinel
table_hi (0x1B2C6C) ──► file0 data | file0 extern-id list | file1 data | ...
```

`LBTableEntry` is 12 bytes; **every size field counts 32-bit words, not
bytes**:

```c
struct LBTableEntry {
    ub32 is_compressed : 1;
    u32  data_offset   : 31;  // relative to table_hi
    u16  reloc_intern_offset; // word index of first intern slot, or 0xFFFF
    u16  compressed_size;     // ON-ROM size, even when uncompressed
    u16  reloc_extern_offset; // word index of first extern slot, or 0xFFFF
    u16  decompressed_size;
};
```

Loading a file (`lbRelocLoadAndRelocFile`):

1. DMA `compressed_size` words; if `is_compressed`, VPK0-decode instead.
2. Walk the **intern** chain: a singly linked list threaded *through the
   pointer slots being patched*. Slot at word `n` holds
   `{ u16 next, u16 target_word }`; it is overwritten with
   `base + target_word * 4`.
3. Walk the **extern** chain identically, except targets live in other files.
   The target file IDs are a `u16` array in ROM immediately after this file's
   data, consumed in chain order.

Measured over the whole US archive: **61,343 intern slots** and **3,092 extern
slots**, across 2132 files.

### Compression: VPK0

`syDmaDecodeVpk0` in `src/sys/dma.c`. LZ77 with two Huffman trees. 499 of the
2132 files are compressed (23.4%); overall ratio 1.80x (9.07 MiB → 16.29 MiB).

Two things are easy to get wrong from a format description:

* Trees are stored in **postfix** order — `0` pushes a leaf, `1` pops two and
  pushes an internal node, and a `1` with fewer than two nodes on the stack
  ends the tree.
* A leaf's value is a **bit width**, not a value: after reaching a leaf you
  read that many more bits to get the actual offset or length.

There are two distance encodings, selected by a `sample_method` byte in the
header. The two-sample form computes `src = dst - value*4 - correction + 8`.

---

## 6. Rendering

`src/sys/objdisplay.c`, `src/sys/rdp.c`, `src/sys/video.c`, `src/gr/grdisplay.c`.

The game renders at **320x240**. Microcode is **F3DEX2**, which matters because
F3DEX2 renumbers the SP opcodes relative to F3DEX (`G_VTX` is `0x01`, not
`0x04`) and inverts the `G_MTX` parameter byte.

### What the game actually emits

Confirmed present in `objdisplay.c`:

* `gSPMatrix` / `gSPPopMatrix` / `gSPMvpRecalc` — the DObj hierarchy walk
* `gSPDisplayList` / `gSPBranchList` — calls into ROM-resident geometry
* `gSPSegment(0xE, ...)` — segment 0xE is the graphics heap
* `gDPSetTextureImage`, `gDPSetTile`, `gDPLoadBlock`, `gDPLoadTLUTCmd`
* `gDPSetPrimColor`, `gDPSetEnvColor`, `gDPSetBlendColor`
* `gSPLightColor(LIGHT_1/LIGHT_2, ...)` — two-light setup
* `gSPTexture(s, t, 0, 0, G_ON)`

Texture formats seen in the MObj path: `G_IM_FMT_RGBA` at `G_IM_SIZ_16b` and
`32b`, plus `G_IM_SIZ_4b` and `8b` paletted (CI) with TLUT loads
(`gDPLoadTLUTCmd(..., 5, siz == 8b ? 0xFF : 0xF)` — 256- and 16-entry
palettes).

### Port strategy

Do **not** emulate the RDP. Parse the display lists at build time
(`crates/ssb-rom/src/dl.rs`) and lower them to PSP vertex buffers and `sceGu`
state. The N64 vertex cache (`G_VTX` loads N vertices at an index, `G_TRI1` /
`G_TRI2` index into it) maps naturally onto an indexed draw.

Detail in `docs/rendering.md`.

---

## 7. Physics and fighters

`src/ft/` — `ftphysics.c`, `ftmain.c` (4823 lines), `fttypes.h` (1321 lines),
`ftcollision.txt`, plus `ftchar/ft{mario,fox,...}` per character.

* **Float, not fixed point.** `ftPhysicsApplyGravityClampTVel` does
  `fp->physics.vel_air.y -= gravity` on `f32`. There is no fixed-point
  representation to recover.
* **`+Y` is up.** Gravity subtracts from Y.
* **`Z` is a shallow depth axis, clamped to ±60.** Smash is a 2D fighter staged
  in a 3D scene; `ftPhysicsSetGroundVelTransferAir` enforces the bound by
  trimming *velocity*, so fighters decelerate into it.
* **Ground and air velocity are separate.** `vel_ground` (X only) and
  `vel_air` (3D) with an explicit transfer on takeoff/landing, plus a third
  `vel_knockback`.
* **Per-character tuning is data.** `FTAttributes` holds gravity, `tvel_base`,
  `tvel_fast`, `air_accel`, `air_speed_max_x`, `air_friction`, `traction`,
  `dash_speed`, `run_speed`, `weight`, jump parameters, `jumps_max`, shield and
  shadow sizes, camera offsets, SFX ids, and per-move availability bitfields.
  These live in relocData, one file per character.
* **Floor material scales traction**:
  `dMPCollisionMaterialFrictions[floor_flags & MAP_VERTEX_MAT_MASK] * attr->traction`.

Ported so far in `crates/ssb-game/src/physics.rs`, function-for-function with
the original's addresses quoted.

### Roster

`enum FTKind` ordinals, which asset tables are indexed by and must not be
renumbered:

| 0-11 | Mario, Fox, Donkey, Samus, Luigi, Link, Yoshi, Captain, Kirby, Pikachu, Purin (Jigglypuff), Ness |
| 12-13 | Boss (Master Hand), MMario (Metal Mario) |
| 14-25 | The Fighting Polygon Team (`NMario`…`NNess`) |
| 26 | GDonkey (Giant DK) |

---

## 8. Audio

`src/sys/audio.c`, `src/audio/`, `audio.md`, `MUSIC_AND_SFX_DISCOVERIES.md`.

Standard libultra "n_audio":

* **Music**: 47 sequences in `S1_music.sbk`, an `ALSeqFile` of compressed-MIDI
  bytecode.
* **Samples**: two instrument banks, `B1_sounds1` and `B1_sounds2`
  (`.ctl` + `.tbl`), 117 and 322 waveforms, **VADPCM**-compressed.
* **SFX engine**: "FGM", with ids enumerated by `gmFGMVoiceID`.
* **Synthesis**: RSP microcode (`n_aspMain`) on a dedicated thread.

**Port strategy.** Do not reproduce this. Convert sequences and decode VADPCM
at build time; play back through `sceAudio` with a software mixer on its own
thread. Note that one PSP audio block (1024 samples @ 44.1 kHz ≈ 23 ms)
*outlasts* a 16.67 ms frame, so mixing cannot be inline in the frame loop.

---

## 9. Memory

`src/sys/malloc.c` is 41 lines. `src/sys/taskman.c` provides
`syTaskmanMalloc(size, align)`. The relocData loader takes a caller-provided
heap (`lbRelocGetAllocSize` sizes the dependency closure up front, then one
allocation serves the whole set).

That pattern — *compute the closure, allocate once, load into it* — transfers
directly to PSP and is better than incremental allocation. Detail in
`docs/memory.md`.

| N64 | Size | PSP | Size |
|---|---|---|---|
| RDRAM | 4 MiB (8 with Expansion Pak) | Main RAM | 32 MiB (PSP-1000) / 64 MiB |
| RDP TMEM | 4 KiB | VRAM | 2 MiB |
| — | | Scratchpad | 16 KiB |

The PSP has strictly more memory than the N64 in every category except TMEM,
which we do not need because textures live in VRAM rather than being streamed
per-primitive. **Memory pressure is not expected to be the port's constraint.**

---

## 10. Where each subsystem lands

| Original | Lines | Port destination |
|---|---|---|
| `sys/main.c` | 235 | `psp/src/main.rs` — rewritten, not ported |
| `sys/scheduler.c` | 1319 | **dropped** — no RCP to schedule |
| `sys/taskman.c` | 1377 | partly dropped; heap logic → `engine/memory` |
| `sys/objman.c` | 2442 | `ssb-game` scene graph |
| `sys/objdisplay.c` | 3394 | split: traversal → `ssb-game`, emission → `psp/renderer` |
| `sys/objanim.c` | 3028 | `ssb-game/animation` |
| `sys/matrix.c`, `vector.c` | 1888 | `ssb-engine/math` (+ VFPU later) |
| `sys/dma.c` | 631 | `ssb-rom/vpk0` + resource loading |
| `sys/controller.c` | 494 | `ssb-engine/input` + `psp/input` |
| `sys/audio.c` | 1506 | `ssb-engine/audio` + `psp/audio` |
| `sys/video.c`, `rdp.c` | 291 | `psp/gu` |
| `lb/lbreloc.c` | 449 | `ssb-rom/archive` ✅ **done** |
| `ft/*` | ~20k | `ssb-game/fighter`, `physics`, … |
| `gr/*` | — | `ssb-game/stage` |
| `gm/gmcollision.c` | 2174 | `ssb-game/collision` |
| `mn/*`, `sc/*` | — | `ssb-game/menus`, scene management |
| `libultra/*` | — | **dropped** — replaced by PSP equivalents |

---

## 11. Architecture comparison

| Concern | SSB64 (N64) | sf64-psp | n64psp | This port |
|---|---|---|---|---|
| Language | C (IDO) | C | C | **Rust** |
| Approach | native | decomp + PSP backend | reusable N64→PSP runtime | decomp-informed rewrite |
| Rendering | F3DEX2 → RDP | GU translation of N64 DLs | backend-registration only | build-time DL → PSP mesh |
| Runtime DL translation | n/a | yes | n/a | **no — preconverted** |
| Audio | RSP `aspMain` | PSP audio | not implemented | build-time convert + SW mixer |
| Threading | libultra, 5 threads | PSPSDK | PSPSDK sema/threads | 2 threads (game + audio) |
| Layering | monolithic | game + compat layer | runtime / bridge / backend | **A game / B traits / C PSP** |

`n64psp` is early — a runtime skeleton with message queues, a platform-callback
interface and a *trace* renderer backend that returns
`N64PSP_ERROR_UNSUPPORTED`. Its value here is the **shape** of the layering
(runtime must not know about the game; the graphics backend is registered, not
hardcoded), which this port adopts as Layers A/B/C. There is no working
VFPU/GU implementation to borrow.

`sf64-psp` is the more mature reference for actual N64→PSP rendering, but it
translates display lists at runtime because Star Fox 64 generates them
dynamically. Smash's geometry is static ROM data, so we can preconvert — which
is strictly cheaper on a 333 MHz CPU.

---

## 12. Open questions

Tracked with evidence and confidence levels in
`docs/reverse-engineering.md`. The significant ones:

* Which C-button functions matter in Smash 64 (affects the control mapping).
* Whether `MObjSub`'s unknown fields (`unk08`…`unk74`) carry anything the
  renderer needs.
* How `sGCDetailLevel` is chosen, and whether the PSP should force a tier.
* Exact semantics of `SC_TASK_TYPE_11` (may not matter — likely scheduler-only).
