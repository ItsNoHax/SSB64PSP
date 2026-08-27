# Porting Status

Per Rule 11. Percentages are of *intended scope for that subsystem*, not of the
original's line count. A subsystem is only `COMPLETE` when it has been
functionally validated, not merely compiled (Rule 12).

Last updated: 2026-08-27.

## Milestones

| Milestone | Status | Notes |
|---|---|---|
| **M0 — Research** | ✅ COMPLETE | `docs/ssb-architecture.md`, this file, `docs/reverse-engineering.md` |
| **M1 — Rust PSP bootstrap** | ✅ COMPLETE | Boots in PPSSPP at a locked **60 FPS**; renders, animates, reads input. Screenshot: `docs/images/m1-ppsspp-60fps.png` |
| **M2 — Resource pipeline** | ✅ COMPLETE | Archive + VPK0 verified; meshes (1768 lists, 0 failures) and textures (336/469) packed into a 1.7 MB runtime pack that round-trips |
| **M3 — Rendering** | 🟡 65% | **ROM geometry renders on device at 60 FPS** (`docs/images/m3-rom-geometry.png`). Textures/lighting not yet validated |
| **M4 — Gameplay vertical slice** | 🔴 5% | Physics core ported; no match loop |
| **M5 — Audio** | 🔴 0% | Traits only |
| **M6 — Full gameplay** | 🔴 0% | |
| **M7 — Menus / save** | 🔴 0% | |
| **M8 — Optimization** | 🔴 0% | Blocked on M3 per "do not optimize before profiling" |
| **M9 — Hardware validation** | 🔴 0% | |

## Subsystems

| Subsystem | Status | Validation |
|---|---|---|
| ROM validation | ✅ COMPLETE | SHA-1/MD5 checked against the real dump; byte-order and size rejection unit-tested |
| VPK0 decompression | ✅ COMPLETE | All 499 compressed files cross-verified against independent ROM geometry (RE-002) |
| relocData archive | ✅ COMPLETE | 2132/2132 files load; 61,343 intern + 3,092 extern relocations, 0 mismatches |
| Asset extraction CLI | ✅ COMPLETE | `romtool extract` produces 16.29 MiB + manifest |
| F3DEX2 DL parser | ✅ COMPLETE | All opcodes Smash emits, verified against real lists; `G_VTX` encoding regression-tested (RE-017) |
| N64 texture decode | 🟢 70% | RGBA16/32, IA4/8/16, I4/8, CI4/8 decoded and unit-tested; not yet run on real ROM textures |
| Texture → PSP conversion | 🟢 70% | 336/469 packed, 75.4% VRAM saved, 77% swizzled. 133 need cross-file/TLUT tracking (RE-019) |
| DL discovery | ✅ COMPLETE | 1,864 lists across 135 files; converter used as validator (RE-017) |
| Mesh conversion | 🟢 80% | 25,562 tris, 2.09x vertex reuse, material merging; 0 failures. No skinning yet |
| Model conversion | 🟡 30% | Meshes extracted; DObj hierarchy/animation not applied |
| Asset pack format | ✅ COMPLETE | Zero-copy, 16-byte aligned, little-endian; writer + reader unit-tested, 1768 meshes round-trip |
| PSP asset loading | ✅ COMPLETE | 1.7 MB pack loads aligned, cache-flushed, verified on device |
| PSP mesh drawing | 🟢 65% | Indexed GE draws verified on device: 396 tris, 2 draws, 60 FPS, 168us CPU. Lit materials still draw normals as colours |
| Coordinate conversion | 🟢 80% | Matrix/UV/viewport unit-tested; needs on-hardware confirmation (RE-004, RE-005) |
| Math (scalar) | 🟢 80% | 36 unit tests; no VFPU path yet (correctly — profile first) |
| VFPU optimization | 🔴 0% | Deliberately not started |
| Engine traits (Layer B) | 🟢 70% | Renderer / Audio / Input / Timing / Clock defined |
| Timing / fixed clock | ✅ COMPLETE | Catch-up cap, backwards-clock, 60-ticks-per-second all unit-tested |
| Input mapping | 🟢 75% | Mapping + nub scaling unit-tested; deadzone and C-buttons unresolved (RE-008, RE-009) |
| PSP GU backend | 🟡 40% | Init, frame lifecycle, matrices, untextured triangles. No textures, no mesh path |
| PSP input backend | 🟢 70% | `sceCtrl` analog read wired to the shared mapping |
| PSP audio backend | 🔴 0% | |
| Physics | 🟢 40% | 15 functions ported with original addresses cited; ground/air/gravity/friction/drift verified |
| Fighter state | 🟡 25% | Roster, facing, situation, hitlag/hitstun timers, land/takeoff |
| Collision | 🔴 0% | |
| Animation | 🔴 0% | |
| Scene graph (GObj) | 🔴 0% | Architecture understood, not implemented |
| Stages | 🔴 0% | |
| Items | 🔴 0% | |
| CPU AI | 🔴 0% | |
| Menus | 🔴 0% | |
| Save data | 🔴 0% | |
| Debug/profiler | 🟡 20% | Frame timing sections defined; on-screen text overlay working |
| CI | ✅ COMPLETE | fmt, clippy, host tests, PSP build, EBOOT artifact — no ROM required |

**Per-fighter progress: all 12 at 0%.** Correctly so — the vertical slice
(M4) comes before any character work.

## Test coverage

86 host tests passing across `ssb-rom` (25), `ssb-engine` (36) and
`ssb-game` (25).

## M1 verification (PPSSPP)

Verified 2026-08-27 under PPSSPP 1.20.4 (OpenGL and software rasteriser), X11:

* Module loads — `tag=ELF/ssb64_psp` at `0x08804000`, imports resolved for
  `sceGeListEnQueue`, `sceCtrlReadBufferPositive`, `sceCtrlSetSamplingMode`,
  `sceDisplaySetMode`, `sceDisplayWaitVblankStart`.
* `PARAM.SFO` title reads correctly ("Super Smash Bros. 64").
* GE display lists submit; `sceDisplaySetMode(0, 480, 272)` and
  `sceDisplaySetFrameBuf` run each frame.
* **Locked 60.0 FPS.**
* Geometry renders with correct vertex-colour interpolation and depth.
* Animation advances (rotation differs between captures).
* Physics runs on-device: the test object falls under gravity and lands on the
  test floor at exactly y = -3.00.

Measured from the on-screen diagnostics (RE-016):

```
frame 701  tick 701          <- exact 60 Hz lockstep, no drift over 700 frames
ticks/frame 1  dropped 0     <- no catch-up, no dropped ticks
cpu 13us / budget 16667us    <- 0.08% of the frame budget
frame 16682us  view 362x272  <- 59.94 Hz; pillarboxed 4:3 viewport confirmed
```

These are *baseline* numbers on a four-triangle scene under an emulator, not a
performance prediction for a real match on real hardware.

Reproduce with `tools/run-ppsspp.sh`.

## Known gaps and honest caveats

1. **Nothing has run on real PSP hardware.** Per §37 of the plan, PPSSPP is
   not proof of hardware behaviour. This is now the single biggest unknown.

2. **The debug overlay only displays under PPSSPP's software rasteriser.**
   Resolved as an emulator limitation, not a port bug (RE-014):
   `sceGuDebugFlush` paints VRAM with the CPU, and PPSSPP's hardware backends
   do not reflect those writes. `tools/run-ppsspp.sh` forces the software
   renderer so diagnostics are always visible. The real HUD will render as GE
   geometry (Renderer 3), which removes the dependency entirely.

3. **Extracted assets are unparsed.** `romtool extract` produces byte-exact
   file payloads, but nothing yet interprets them as textures, meshes or
   attribute tables. The archive layer is trustworthy; the layers above it
   do not exist.

4. **`PhysicsAttributes::default()` is not a real character.** It is a neutral
   baseline for tests. Real values must come from the extracted `FTAttributes`
   files before any gameplay claim can be made.

5. **The extern relocation slots are zeroed, not resolved.** `romtool` records
   them in the manifest rather than patching them, because the target address
   depends on runtime layout. The runtime loader that applies them does not
   exist yet.
