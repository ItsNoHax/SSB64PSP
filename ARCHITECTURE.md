# SSB64PSP — Current Code Architecture

**Generated from actual source code** — describes what exists, not what's planned.

---

## Repository Layout

```
SSB64PSP/
├── Cargo.toml                    # Workspace root (stable)
├── Cargo.lock
├── rust-toolchain.toml           # stable 1.98.0
├── AGENTS.md                     # Agent operating protocol
├── PLAN.md                       # Milestone roadmap
├── STATUS.md                     # Current working state
├── TODO.md                       # Discovered future work
├── DECISIONS.md                  # Permanent technical decisions
├── ARCHITECTURE.md               # This file
├── README.md                     # Project overview
├── PLAN.MD                       # Legacy plan (superseded by PLAN.md)
├── .github/workflows/ci.yml      # CI pipeline
├── docs/                         # Technical documentation
│   ├── ssb-architecture.md       # Original game architecture
│   ├── reverse-engineering.md    # RE-XXX log
│   ├── rendering.md              # N64→PSP rendering
│   ├── memory.md                 # Memory layout
│   ├── porting-status.md         # Per-subsystem progress
│   ├── rendering-fidelity.md     # Fidelity checklist
│   └── rendering-fidelity-baseline.json
├── crates/                       # Host + PSP shared crates (no_std)
│   ├── ssb-rom/                  # ROM formats, archive, conversion, pack
│   ├── ssb-engine/               # Layer B: traits, math, coord, timing
│   └── ssb-game/                 # Layer A: fighters, physics, collision, animation
├── tools/
│   └── romtool/                  # Host-only CLI (extraction, verification)
└── psp/                          # PSP executable (separate Cargo project)
    ├── Cargo.toml
    ├── rust-toolchain.toml       # nightly-2026-08-01
    └── src/                      # Layer C: sceGu, sceCtrl, main, meshdraw
```

---

## Crate Dependency Graph

```
                    ┌─────────────────┐
                    │   romtool       │  (host only, std)
                    └────────┬────────┘
                             │ uses
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
        ┌───────────┐  ┌───────────┐  ┌───────────┐
        │ ssb-rom   │  │ssb-engine │  │ ssb-game  │
        │ (no_std)  │  │ (no_std)  │  │ (no_std)  │
        └─────┬─────┘  └─────┬─────┘  └─────┬─────┘
              │              │              │
              └──────────────┼──────────────┘
                             │ linked by
              ┌──────────────┴──────────────┐
              ▼                             ▼
        ┌─────────────┐               ┌─────────────┐
        │  Host test  │               │  PSP binary │
        │  (stable)   │               │ (nightly)   │
        └─────────────┘               └─────────────┘
```

**Key invariant:** `ssb-rom`, `ssb-engine`, `ssb-game` are `no_std` with `default = ["std"]`. Workspace dependencies declare `default-features = false` (root `Cargo.toml:31-33`). Crates wanting `std` opt in via their own `std` feature.

---

## Layer A — Game (`crates/ssb-game`)

### `fighter.rs`
Fighter entity and roster. `Fighter` struct holds:
- `kind: FTKind` (0-26, must not renumber)
- `facing: i8` (+1/-1)
- `state: FighterState` (status machine)
- `physics: FighterPhysics` (vel_ground, vel_air, vel_knockback, gravity, etc.)
- `collision: FighterCollision` (floor height, surface flags, ledge state)
- `animation: Skeleton` (joint transforms, current animation)
- `attributes: &'static FTAttributes` (real constants from ROM)

Methods: `tick(&mut self, input, stage_collision, dt)`, `spawn(kind, pos)`, `change_status()`

### `status.rs`
Movement status machine — **complete for movement, empty for combat**.
States: `Wait`, `WalkSlow/Medium/Fast`, `Dash`, `Run`, `RunBrake`, `Turn`, `KneeBend`, `JumpF/JumpB`, `JumpAerialF/B`, `Fall`, `FallAerial`, `Squat`, `LandingLight/Heavy`, `Pass`.

Each status has:
- `animation: FigatreeAnimation` (from pack)
- `duration: u16` (from `FTAttributes` or figatree length)
- `interrupts: StatusInterrupts` (original chain ordering)
- `tap_counter: u8` (for smash-turn, dash, etc.)

`StatusMachine::tick()` drives transitions per original interrupt logic.

### `physics.rs`
16 functions ported function-for-function from `ftphysics.c` with original addresses cited:
- `apply_gravity_clamp_tvel`
- `set_ground_vel_transfer_air`
- `set_ground_vel_friction`
- `apply_air_vel_x_friction`
- `apply_air_drift`
- `apply_fall_speed`
- `ground_correction` (swept floor query integration)
- `vel_knockback` handling

All 27 fighters' `FTAttributes` extracted and verified field-by-field.

### `collision.rs`
Stage collision query ported from `mpcollision.c`:
- `swept_floor_query(old_pos, new_pos) -> FloorResult`
- `vertical_projection(pos) -> f32`
- `per_line_surface_height(line, x, z) -> f32`
- `mpprocess_floor_path` (substepping, landing snap at 0.001, ledge corner, follow-surface)

Surface flags: `MAP_VERTEX_COLL_PASS` (drop-through), `MAP_VERTEX_COLL_CLIFF` (ledge-grabbable). Material friction via `dMPCollisionMaterialFrictions[material] * attr->traction`.

### `ground.rs`
Stage collision geometry wrapper. Holds packed polylines (1531 lines, 3331 vertices across 41 stages). Provides iterator for collision queries.

### `lib.rs`
Re-exports: `fighter`, `physics`, `collision`, `ground`, `status`.

---

## Layer B — Engine (`crates/ssb-engine`)

### `renderer.rs`
`Renderer` trait — **minimal, not fully implemented**:
```rust
trait Renderer {
    fn begin_frame(&mut self);
    fn end_frame(&mut self);
    fn draw_mesh(&mut self, mesh: &Mesh, transform: Mat4, material: &Material);
    fn draw_debug_text(&mut self, text: &str);
}
```

### `input.rs`
`Input` trait + `N64Mapping`:
- `ControllerState { stick_x, stick_y, buttons }` (N64 range: -80..=80)
- `map_psp_to_n64(psp_state) -> ControllerState`
- Deadzone (20 units), linear rescale to ±80
- C-button mapping placeholder (Triangle/Square, C-L/C-R unmapped)

### `timing.rs`
`GameClock` — fixed 60 Hz simulation:
```rust
struct GameClock {
    accumulator: u64,
    tick_hz: u32,           // 60
    max_catchup_ticks: u32, // 3
}
fn advance(&mut self, frame_dt: u64) -> u32; // returns ticks to run
```
Catch-up cap, backwards-clock protection, 1 tick/frame steady state.

### `coord.rs`
Coordinate conversion (N64 ↔ PSP):
- `n64_to_psp_matrix(m: N64Matrix) -> Mat4` — no transpose, s15.16 → f32
- `n64_to_psp_position(v: Vec3) -> Vec3` — identity
- `pillarboxed_viewport() -> (x, y, w, h)` — 362×272 centered in 480×272
- UV: S10.5 (÷32) → normalized → `sceGuTexScale(1024/w, 1024/h)`

### `math.rs`
Scalar math (no VFPU yet):
- `Vec3`, `Vec4`, `Mat4`, `Quat`
- 36 unit tests
- `Mat4::from_n64_mtx()`, `Mat4::mul()`, `transform_vec3()`

### `audio.rs`
`AudioBackend` trait — **stub only**:
```rust
trait AudioBackend {
    fn play_sfx(&mut self, id: u32);
    fn play_music(&mut self, id: u32);
    fn set_volume(&mut self, vol: f32);
}
```

### `lib.rs`
Re-exports all traits and math.

---

## Layer C — PSP Backend (`psp/src/`)

### `main.rs` (42 KB)
PSP entry point. Sets up:
- Thread: main game thread + audio thread (2 threads, not 5 like N64)
- `sceGu` initialization (double-buffered 480×272, depth, viewport, scissor)
- `sceCtrl` analog sampling mode
- `sceDisplay` vblank wait
- Game loop: fixed-tick simulation + render frame
- Debug overlay via `sceGuDebugFlush` (requires software rasteriser in PPSSPP)

### `gu.rs`
`Gpu` struct — `sceGu` wrapper:
- `init()` — GU setup, matrices, depth range (65535, 0), viewport+scissor pillarbox
- `begin_frame()` / `end_frame()` — `sceGuStart`/`sceGuFinish`/`sceGuSync`/`sceDisplayWaitVblankStart`
- `draw_mesh()` — `sceGumDrawArray` with `GU_VERTEX_16BIT` + `GU_TEXTURE_16BIT`
- `debug_text()` — `sceGuDebugPrint` + `sceGuDebugFlush` (CPU VRAM write)

### `meshdraw.rs`
Mesh drawing implementation:
- `Mesh` — vertex buffer (Aligned16), index buffer, material ID
- `Material` — texture ID, CLUT ID, blend mode, flags
- `draw_stage_animated()` — draws stage layers with baked node matrices
- `draw_fighter()` — draws fighter skeleton posed by `Skeleton::compose()`
- `MODEL_SCALE = 32768.0` (undoes `GU_VERTEX_16BIT` normalization)

### `input.rs`
`PspInput` — `sceCtrlReadBufferPositive` → `ControllerState` via shared mapping.

### `timing.rs`
`PspClock` — `sceKernelLibcClock` / `sceRtcGetCurrentTick` for frame timing.

### `play.rs`
Viewer/debug mode: stage browse, fighter spawn, animation playback, collision viz.

### `assets.rs`
Runtime pack loader — loads `ssb64.pak`, verifies header, maps sections.

---

## Shared — ROM & Asset Pipeline (`crates/ssb-rom`)

### Archive Layer
| File | Purpose |
|------|---------|
| `rom.rs` | ROM validation (SHA-1/MD5), segment layout |
| `vpk0.rs` | VPK0 decompression (LZ77 + postfix Huffman) |
| `archive.rs` | relocData: 2132 files, intern/extern relocation chains |
| `scan.rs` | Display list discovery (1,864 lists, heuristic + relocation targets) |

### Format Decoding
| File | Purpose |
|------|---------|
| `dl.rs` | F3DEX2 command decode (all opcodes Smash emits) |
| `texture.rs` | N64 texture decode (RGBA16/32, IA4/8/16, I4/8, CI4/8) |
| `psp_texture.rs` | PSP packing: swizzle, PSM selection, CLUT, mip generation |
| `mesh.rs` | DL → indexed mesh: cache threading, vertex dedup, material merge |
| `mobj.rs` | MObjSub decode (palette, prim/env/blend color, flags, unknowns) |

### Animation
| File | Purpose |
|------|---------|
| `anim.rs` | `AObjEvent32` decode (stage joint + material animation) |
| `figatree.rs` | `AObjEvent16` decode (fighter joint animation) |
| `anim_table.rs` | Animation table packing for runtime |
| `skeleton.rs` | `Skeleton` + `Joint` — per-joint clocks, compose() at 60 FPS |
| `objanim.rs` | DObjDesc array recovery (363 arrays, depth-tagged tree) |

### Scene Graph
| File | Purpose |
|------|---------|
| `scene.rs` | SceneGraph, DObjNode, NodeDesc (world matrix, material chain, flags) |
| `stage.rs` | MPGroundData/MPGroundDesc recovery (41 stages, 4 layers each) |
| `collision.rs` | MPGeometryData decode (polylines, surface flags, map objects) |
| `fighter.rs` | FTAttributes, FTCommonPart, commonparts_container recovery |
| `matanim.rs` | Material animation script decode (12 layers, not yet played) |

### Pack Format
| File | Purpose |
|------|---------|
| `pack.rs` | PackWriter/PackReader — zero-copy, 16-byte aligned, v6 |
| | Sections: header, textures, meshes, nodes, animations, stages, fighters, collision |

**Pack v6 contents:** 2450 meshes, 3137 scene-graph nodes, 617 textures, 189 fighter animations, 4709 joint entries, 41 stage collisions, node local rest transforms.

---

## Host Tool — `tools/romtool`

### `main.rs` (170 KB)
CLI commands:
- `verify <rom>` — SHA-1/MD5 check
- `info <rom>` — Archive summary
- `extract <rom>` — Byte-exact file payloads (16.29 MiB) + manifest
- `pack <rom>` — Build runtime `ssb64.pak`
- `check <rom>` — VPK0 cross-verification (chain vs ROM gap)
- `fighters <rom> --verify` — FTAttributes vs decomp (27/27 agree)
- `anims <rom> --verify` — Animation lengths two ways (189/189 agree)
- `figatree <rom> --frames N --pack <pak>` — Replay from pack vs ROM
- `stages <rom> --pack <pak>` — Stage poses vs archive (444,960 values)
- `textures <rom>` — Conversion report (647 bound, 617 packed, 30 failed)
- `mesh <rom>` — Mesh conversion stats
- `scene <rom>` — DObjDesc array validation
- `mobj <rom>` — Material table search scoring

---

## Data Flow (Build Time)

```
User ROM (Super Smash Bros. (USA).z64)
         │
         ▼
romtool verify  ──► SHA-1/MD5 match?
         │
         ▼
romtool pack
         │
         ├─► romtool scan          → discovers 1,864 display lists
         ├─► romtool mesh          → converts to 2450 indexed meshes
         ├─► romtool textures      → converts 617 textures (swizzle, PSM, mips)
         ├─► romtool scene         → recovers 363 DObjDesc arrays
         ├─► romtool fighter       → extracts 27 FTAttributes + figatree
         ├─► romtool stage         → extracts 41 stages + collision + animation
         └─► PackWriter            → writes ssb64.pak (3.6 MB, zero-copy)
         │
         ▼
assets/generated/ssb64.pak  ──► staged by run-ppsspp.sh  ──► PSP loads at runtime
```

---

## Data Flow (Runtime on PSP)

```
EBOOT.PBP boots
         │
         ▼
psp::main::main()
         │
         ├─► Gpu::init()                    (sceGu setup)
         ├─► PspInput::init()               (sceCtrl setup)
         ├─► assets::load_pack()            (mmap ssb64.pak, cache flush)
         ├─► GameClock::new()               (60 Hz fixed tick)
         └─► Viewer::new()                  (stage browser, fighter spawn)
         │
         ▼
Game Loop (per frame):
         │
         ├─► frame_dt = PspClock::frame_time()
         ├─► ticks = GameClock::advance(frame_dt)
         ├─► for _ in 0..ticks:
         │     ├─► input = PspInput::poll()
         │     ├─► fighter.tick(input, stage_collision, 1/60)
         │     ├─► Skeleton::tick()         (joint clocks advance)
         │     └─► StageAnimator::tick()    (AObjEvent32 joints)
         │
         ├─► Gpu::begin_frame()
         ├─► meshdraw::draw_stage_animated()  (baked node matrices)
         ├─► meshdraw::draw_fighter()         (posed skeleton)
         ├─► Gpu::debug_text()                (diagnostics overlay)
         └─► Gpu::end_frame()                 (sceGuSync + vblank wait)
```

---

## Key Types (Cross-Crate)

| Type | Defined In | Used By |
|------|------------|---------|
| `FTKind` | `ssb-rom::fighter` | `ssb-game::fighter`, `psp::play` |
| `FTAttributes` | `ssb-rom::fighter` | `ssb-game::fighter`, `ssb-game::physics` |
| `FigatreeAnimation` | `ssb-rom::figatree` | `ssb-game::status`, `psp::play` |
| `Skeleton` | `ssb-rom::skeleton` | `ssb-game::fighter`, `psp::meshdraw` |
| `SceneGraph` / `NodeDesc` | `ssb-rom::scene` | `psp::meshdraw` |
| `StageCollision` | `ssb-rom::collision` | `ssb-game::collision`, `ssb-game::ground` |
| `Material` / `Mesh` | `ssb-rom::pack` | `psp::meshdraw` |
| `ControllerState` | `ssb-engine::input` | `ssb-game::fighter`, `psp::input` |
| `GameClock` | `ssb-engine::timing` | `psp::main`, `psp::play` |

---

## Build Profiles

| Target | Command | Toolchain | Output |
|--------|---------|-----------|--------|
| Host tests | `cargo test --workspace` | stable 1.98.0 | 317 tests pass |
| romtool | `cargo build --release -p romtool` | stable 1.98.0 | `target/release/romtool` |
| PSP EBOOT | `cd psp && cargo psp --release` | nightly-2026-08-01 | `target/mipsel-sony-psp/release/EBOOT.PBP` |

**PSP crate is NOT a workspace member** — excluded in root `Cargo.toml` to allow stable host tests.

---

## Memory Layout (Planned, from `docs/memory.md`)

```
Main RAM (32/64 MiB)
├── Game Arena        ~2-4 MiB    (game state, fighters, scene graph)
├── Asset Arena       ~8-16 MiB   (per-scene relocData closure, contiguous)
├── Render Arena      ~2 MiB      (CPU-side mesh/material records)
├── Frame Arena       ~256 KiB    (bump alloc, reset per tick)
└── Audio Arena       ~1-2 MiB    (decoded samples, sequencer)

VRAM (2 MiB)
├── Framebuffer 0     522 KiB     (480×272×4)
├── Framebuffer 1     522 KiB     (480×272×4)
├── Depth Buffer      261 KiB     (480×272×2)
└── Texture Pool      ~700 KiB    (CI4/CI8 paletted — 617 textures = 717 KiB packed)

Scratchpad (16 KiB)
└── VFPU staging (after profiling)
```

**Current:** Pack loads as single contiguous block. Arenas not yet implemented.

---

## Threading Model

| N64 (5 threads) | PSP (2 threads) |
|-----------------|-----------------|
| Idle (pri APPMAX→IDLE) | — |
| Main game (pri 50) | **Game thread** (simulation + render) |
| Scheduler (pri 120) | **Dropped** — no RCP to schedule |
| Audio (pri 110) | **Audio thread** (software mixer, dedicated) |
| Controller (pri 115) | Polled in game thread via `sceCtrl` |

Priority order preserved: audio thread > game thread. Scheduler thread has no equivalent.

---

## Validation Commands (All Reproducible)

```bash
# Full test suite
cargo test --workspace --all-targets

# ROM verification (requires ROM)
cargo run --release -p romtool -- verify "rom/Super Smash Bros. (USA).z64"
cargo run --release -p romtool -- check "rom/Super Smash Bros. (USA).z64"
cargo run --release -p romtool -- fighters "rom/..." --verify
cargo run --release -p romtool -- anims "rom/..." --verify
cargo run --release -p romtool -- figatree "rom/..." --frames 40 --pack assets/generated/ssb64.pak
cargo run --release -p romtool -- stages "rom/..." --pack assets/generated/ssb64.pak
cargo run --release -p romtool -- textures "rom/..."

# Build + run on PPSSPP
cd psp && cargo psp --release
tools/run-ppsspp.sh
```

---

## Known Code Gaps (from STATUS.md)

1. **PSP GU backend** — No textured mesh path, no material state (40%)
2. **Extern relocation runtime loader** — Pack records them zeroed
3. **Arenas/pools** — GameArena, AssetArena, FrameArena, ObjectPool not implemented
4. **VFPU math** — Scalar only, correctly deferred
5. **Audio backend** — Trait only
6. **Combat pipeline** — Empty (hitbox, damage, knockback, opponent, match loop)
7. **Stage loader** — Viewer browses, match doesn't select