# Super Smash Bros. 64 → PSP — Rust Native Port Plan

## Objective
Create a native Rust port of the original Nintendo 64 Super Smash Bros. to the Sony PSP.

- **Not an emulator** — a reimplementation for PSP hardware
- Written primarily in Rust
- Target: PSP-1000/2000/3000
- Build using `rust-psp` / `cargo-psp`
- Use the SSB64 decompilation as primary reference for game logic
- Extract assets at build time from user-supplied ROM
- Never commit Nintendo IP
- Preserve original gameplay behavior
- Replace N64 subsystems with PSP-native implementations
- Exploit VFPU and GPU where useful
- Eventually run on real PSP hardware

**Primary references:**
1. [ssb-decomp-re](https://github.com/VetriTheRetri/ssb-decomp-re) — 100% complete
2. [sf64-psp](https://github.com/TheMrIron2/sf64-psp)
3. [n64psp](https://github.com/TheMrIron2/n64psp)
4. [rust-psp](https://github.com/overdrivenpotato/rust-psp)

---

## Architecture

### Three-Layer Separation
```
Layer A — Game            crates/ssb-game
    fighters, physics, collision, animation, stages, items, AI, menus, match state
                  │
Layer B — Engine          crates/ssb-engine
    traits: Renderer, AudioBackend, Input, Clock; math, coordinate conversion, fixed timestep
                  │
Layer C — PSP backend     psp/
    sceGu, sceCtrl, sceAudio, VFPU, timing
```

`crates/ssb-rom` sits beside all three: reads ROM at build time, runtime pack on device.

### Crate Structure
| Crate | Purpose | `no_std` | Target |
|-------|---------|----------|--------|
| `crates/ssb-rom` | ROM validation, VPK0, relocData, N64 formats, animation, runtime pack | yes (+alloc) | host + PSP |
| `crates/ssb-engine` | Layer B traits, math, coordinate conversion | yes | host + PSP |
| `crates/ssb-game` | Layer A game logic | yes | host + PSP |
| `tools/romtool` | Build-time extraction/verification CLI | no | host |
| `psp/` | Layer C backend + executable | yes | `mipsel-sony-psp` |

---

## Milestones

### M0 — Research ✅ COMPLETE
- Architecture document (`docs/ssb-architecture.md`)
- Dependency map
- N64 subsystem map
- Asset format inventory
- Rendering command inventory
- Audio inventory

### M1 — Rust PSP Bootstrap ✅ COMPLETE
- Cargo project builds
- EBOOT.PBP generates
- Controller input works
- GU initialization
- Triangle renderer
- Locked 60 FPS in PPSSPP

### M2 — Resource Pipeline ✅ COMPLETE
- ROM validation (SHA-1/MD5)
- VPK0 decompression (499/499 files cross-verified)
- relocData archive (2132/2132 files, 61,343 intern + 3,092 extern relocations)
- Texture conversion (617/647 packed, 77% VRAM saved)
- Basic model conversion (2450 meshes, 42,417 triangles, 0 failures)
- Runtime asset pack (3.6 MB, zero-copy, 16-byte aligned)

### M3 — Rendering 🟢 90%
- SSB stage geometry renders
- Textures + CLUTs work
- Camera + depth
- Transparency (cutout/translucent)
- Lighting/materials (majority-vote heuristic)
- **Outstanding:** Dream Land canopy incorrect, wrap/clamp/mirror, MObj unknown fields, 119 failed textures, material animation, per-layer render state

### M4 — Gameplay Vertical Slice 🟡 65%
- Fighter stands on stage at 60 FPS
- Physics driven tick-by-tick through collision
- 158/158 spawns settle with zero drift
- Movement status machine: Wait, Walk×3, Dash, Run, RunBrake, Turn, KneeBend, Jump F/B, JumpAerial F/B, Fall, FallAerial, Squat, Landing, Pass
- All 27 fighters' real constants extracted and verified
- Animation: 532 movement animations packed, skeleton ticks on device
- Stage collision: all 41 stages, swept floor query, `mpprocess` ported
- **Missing:** attacks, hitboxes, damage, knockback, opponent, stocks, match loop, stage loader

### M5 — Audio 🔴 0%
- AudioBackend trait only
- Build-time VADPCM decode + sequence conversion
- Software mixer on dedicated thread
- SFX → music → correct timing → Media Engine

### M6 — Full Gameplay 🔴 0%
- All 12 original characters
- All original stages
- CPU AI
- Items
- Game modes

### M7 — Menus / Save 🔴 0%
- Title, character select, mode select, pause, results, credits
- PSP-native save system (unlocks, records, settings)

### M8 — Optimization 🔴 0%
- 60 FPS target
- VFPU acceleration
- GPU batching/state sorting
- VRAM/memory optimization
- Audio optimization

### M9 — Hardware Validation 🔴 0%
- PSP-1000 test
- PSP-2000/3000 test
- PPSSPP test
- Long-duration stability test

---

## Subsystem Targets (for M3-M4)

| Subsystem | Target | Validation |
|-----------|--------|------------|
| Renderer | Textured, shaded models at 60 FPS | On-device screenshots match decomp expectations |
| Physics | All 27 fighters' constants field-verified | 158/158 spawns zero drift |
| Collision | All 41 stages, swept query, floor solver | Spawn drop test passes |
| Animation | Figatree + stage AObj at 60 FPS | Pose match ROM, no bone stretch, feet planted |
| Input | N64→PSP mapping, deadzone, C-buttons | Unit tests + device feel |
| Asset Pack | Zero-copy load, <1s init | PPSSPP boot + pack verification |

---

## Architectural Decisions (Binding)

1. **No RDP emulation** — Parse display lists at build time, emit PSP meshes
2. **Preconversion over runtime** — Static ROM geometry → build-time conversion
3. **Layer separation** — Game logic never mentions PSP; PSP never mentions fighters
4. **Fixed 60 Hz simulation** — Decoupled from rendering (59.94 Hz display)
5. **Coordinate systems match** — No handedness flip; N64 row-major/row-vector cancels with PSP column-major/column-vector
6. **Explicit allocators** — GameArena, AssetArena, FrameArena, ObjectPool<T>; no heap in hot paths
6. **Extern relocations resolved at load** — Pack records them; runtime loader patches
7. **VFPU after profiling** — Scalar first, then accelerate hot paths

---

## Definition of Done (Project Complete)

- User supplies supported SSB64 .z64
- Build validates ROM
- Assets extract + convert automatically
- No Nintendo assets committed
- Rust PSP application builds
- EBOOT.PBP boots
- Title screen works
- Character select works
- All 12 original characters work
- All original stages work
- Original gameplay mechanics work
- CPU players work
- Items work
- Audio + music work
- Save data works
- Pause/results/menus work
- Stable 60 FPS target
- Real PSP hardware tested
- Debug/profiling tools exist
- Build is reproducible
- No copyrighted ROM/assets distributed

---

## Immediate Next Work (Combat Vertical Slice)

1. **Grounded attack end-to-end** — Input → hitbox → hurtbox → damage → knockback
2. **Hitbox/hurtbox system** — From `FTAttributes` hurtbox descriptors (RE-032)
3. **Damage/knockback physics** — Port from `ftphysics.c` attack logic
4. **Opponent + match loop** — Second fighter, stock system, blast zones, KO
5. **Stage loader** — Match selects stage, not viewer browse

---

## References
- `docs/ssb-architecture.md` — Original game architecture
- `docs/reverse-engineering.md` — Open questions (RE-XXX)
- `docs/rendering.md` — N64→PSP rendering translation
- `docs/memory.md` — Memory layout
- `docs/porting-status.md` — Per-subsystem progress (source of truth)
- `STATUS.md` — Current working state
- `TODO.md` — Discovered future work
- `DECISIONS.md` — Permanent technical decisions
- `ARCHITECTURE.md` — Current code architecture