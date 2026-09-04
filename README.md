# SSB64PSP

A native Rust port of **Super Smash Bros. (N64)** to the **Sony PSP**.

This is **not an emulator**. It is a reimplementation of the game for PSP hardware, using the [Super Smash Bros. decompilation][decomp] as the primary reference for original behaviour and [`rust-psp`][rustpsp] for the platform layer.

> **Status: rendering-focused engine prototype.** The ROM/resource pipeline, scene graphs, textures, materials, fighter models, fighter animations, stage animations, collision data and core movement systems have been recovered and implemented. Fighters and stages render and animate at a locked 60 FPS under PPSSPP, which is the project's primary validated environment today. The project has been smoke-tested on physical PSP hardware earlier in development, but the formal physical-PSP rendering validation milestone (`PLAN.md` R2) has not yet been completed — see [`STATUS.md`](STATUS.md) §8. The current development priority is completing rendering fidelity and coverage before combat implementation.

![Dream Land rendering on PSP](docs/images/m4-stage-textured.png)

*Dream Land with geometry, textures and palettes extracted from the ROM, placed through the recovered scene graph and rendered through the PSP's Graphics Engine.*

---

## Project Goals

The project follows this order:

```text
Original SSB64 behavior
        ↓
Rendering correctness
        ↓
Rendering completeness
        ↓
Physical PSP validation
        ↓
Rendering performance
        ↓
Combat
        ↓
Full game systems
```

The renderer is a **hard gate** for gameplay development.

The goal is not to produce a game that merely looks similar to SSB64. The implementation should reproduce the original game's behavior wherever the original decompilation and ROM provide sufficient evidence.

---

## Current Status

See [`PLAN.md`](PLAN.md) for the authoritative development roadmap and [`STATUS.md`](STATUS.md) for the current execution state.

### Working and verified

* ROM validation
* VPK0 decompression
* `relocData` archive processing
* Asset extraction and conversion
* Runtime asset-pack generation
* N64 display-list parsing
* F3DEX2-related rendering infrastructure
* N64 texture decoding
* Scene graph conversion
* Fighter model conversion
* Fighter animation extraction and playback
* Stage animation extraction and playback
* Stage collision extraction
* Fighter movement infrastructure
* Fixed timestep
* PSP runtime asset loading
* PSP mesh rendering
* Textured and shaded fighters
* Textured and shaded stages
* Fighter costume colours currently represented by the runtime pack
* Camera-facing/billboard rendering
* Stage scenery animation
* Fighter animation on hardware
* Stage collision queries
* Fighter movement and landing on extracted stage collision

### Physical PSP hardware

The project has been booted and smoke-tested on physical PSP hardware earlier
in development. That testing predates the current rendering work above and
was not captured with the evidence (hardware model, build, asset-pack
version, observed behavior) the project now requires — see `AGENTS.md` §16.
Treat physical-hardware behavior as **unverified for the current renderer**
until PLAN.md's R2 milestone is completed and recorded in `STATUS.md`. PPSSPP
is the environment all claims above were validated against.

### Current rendering work

The remaining work is focused on reproducing the original N64 renderer more completely and accurately, including:

* N64 rendering command coverage
* texture conversion completeness
* CI4/CI8 and TLUT behavior
* texture filtering
* texture addressing
* LOD and mipmapping behavior
* material tables
* material/combiner state
* lighting behavior
* unresolved `MObj` fields
* transform kind `0x8000`
* stage material animation
* additional fighter palettes/costumes
* framebuffer rendering
* screen wipes
* camera/projection correctness
* render-state isolation
* N64 render-state model fidelity (the intermediate representation must not collapse to `mesh + texture + basic colour` before correctness is established)
* deterministic visual-regression methodology (reference vs. PPSSPP software vs. PPSSPP hardware vs. physical PSP)
* comparative audit against `sf64-psp` and `oot-PSP`
* rendering regression coverage
* PSP VRAM usage
* rendering performance

These are tracked individually in `PLAN.md` (see R0.1–R0.18).

### Not yet implemented

Combat and higher-level game systems are intentionally blocked until rendering has passed its acceptance gate.

Not yet implemented include:

* attacks
* hitboxes and hurtboxes
* damage
* knockback
* hitstun
* opponents
* CPU combat AI
* stocks and KO handling
* complete match loop
* complete stage-selection/loading flow
* items
* menus
* save data
* audio

---

## Legal

You must supply your own legally obtained ROM dump.

This repository contains **no Nintendo code, ROM, copyrighted game assets, textures, models or audio**.

Assets are extracted from the user's own ROM on their own machine during the build process. Generated assets are stored under:

```text
assets/generated/
```

and are gitignored.

The `rom/` directory is also gitignored.

---

## Requirements

* Rust stable for host tools and tests
* Rust nightly for the PSP target, pinned by the repository toolchain
* [`cargo-psp`][rustpsp]
* Your own `Super Smash Bros. (USA).z64` ROM

### Supported ROM

|           |                                            |
| --------- | ------------------------------------------ |
| Game code | `NALE` (US)                                |
| SHA-1     | `e2929e10fccc0aa84e5776227e798abc07cedabf` |
| MD5       | `f7c52568a31aadf26e14dc2b6416b2ed`         |

---

## Quick Start

### 1. Add your ROM

```bash
mkdir -p rom
cp "/path/to/Super Smash Bros. (USA).z64" rom/
```

### 2. Verify the ROM

```bash
cargo run -p romtool -- verify "rom/Super Smash Bros. (USA).z64"
```

### 3. Inspect the ROM archive

```bash
cargo run -p romtool -- info "rom/Super Smash Bros. (USA).z64"
```

### 4. Build the runtime asset pack

```bash
cargo run --release -p romtool -- pack "rom/Super Smash Bros. (USA).z64"
```

This generates:

```text
assets/generated/ssb64.pak
```

The PSP executable loads this runtime asset pack.

### 5. Run the host test suite

```bash
cargo test
```

### 6. Build the PSP executable

```bash
cd psp
cargo psp --release
```

The resulting executable is:

```text
psp/target/mipsel-sony-psp/release/EBOOT.PBP
```

### 7. Run under PPSSPP

```bash
tools/run-ppsspp.sh
```

The script stages the generated asset pack next to the executable before launching.

> `tools/run-ppsspp.sh` currently uses PPSSPP's software rasteriser because the debug overlay relies on CPU writes to emulated VRAM. This does not represent the physical PSP rendering path.

---

## Architecture

The project is divided into three primary layers.

```text
              Layer A — Game
              crates/ssb-game
       fighters, physics, collision,
       animation, stages, items,
       AI, menus, match state
                    │
                    ▼
              Layer B — Engine
              crates/ssb-engine
       Renderer, AudioBackend,
       Input, Clock, math,
       coordinate conversion,
       fixed timestep
                    │
                    ▼
              Layer C — PSP
                   psp/
       sceGu, sceCtrl, sceAudio,
       VFPU, timing, PSP runtime
```

Game logic should not directly depend on PSP APIs.

The PSP backend should not contain fighter-specific game logic.

`crates/ssb-rom` sits beside these layers because it provides ROM parsing, extraction and runtime resource handling for both host tooling and the PSP executable.

### Crates

| Crate               | Purpose                                                                        |     `no_std` | Target            |
| ------------------- | ------------------------------------------------------------------------------ | -----------: | ----------------- |
| `crates/ssb-rom`    | ROM validation, archive handling, N64 formats, animation data and runtime pack | Yes (+alloc) | Host + PSP        |
| `crates/ssb-engine` | Engine traits, math and coordinate conversion                                  |          Yes | Host + PSP        |
| `crates/ssb-game`   | Game logic, fighters, stages, physics and animation                            |          Yes | Host + PSP        |
| `tools/romtool`     | ROM verification, extraction, conversion and asset-pack generation             |           No | Host              |
| `psp/`              | PSP backend and executable                                                     |          Yes | `mipsel-sony-psp` |

`psp/` is intentionally outside the root Cargo workspace because the PSP target uses a pinned nightly toolchain and `-Z build-std`.

---

## Verification

The project emphasizes evidence from the original ROM and decompilation rather than visual guesswork.

### ROM integrity

```bash
cargo run --release -p romtool -- check "rom/Super Smash Bros. (USA).z64"
```

### Fighter constants

```bash
cargo run --release -p romtool -- fighters "rom/Super Smash Bros. (USA).z64" --verify
```

### Fighter animations

```bash
cargo run --release -p romtool -- anims "rom/Super Smash Bros. (USA).z64" --verify
```

### Animation/skeleton validation

```bash
cargo run --release -p romtool -- figatree "rom/Super Smash Bros. (USA).z64" --frames 40 \
    --pack assets/generated/ssb64.pak
```

### Stage animation validation

```bash
cargo run --release -p romtool -- stages "rom/Super Smash Bros. (USA).z64" \
    --pack assets/generated/ssb64.pak
```

### Texture conversion report

```bash
cargo run --release -p romtool -- textures "rom/Super Smash Bros. (USA).z64"
```

These checks are intended to establish correctness against the recovered N64 data, rather than simply proving that the code compiles.

---

## Development Roadmap

The development roadmap is maintained in [`PLAN.md`](PLAN.md).

The major phases are:

### Foundation

* research
* PSP bootstrap
* resource pipeline
* core game/scene infrastructure

### Rendering

* rendering correctness
* rendering completeness
* physical PSP validation
* rendering performance

### Gameplay

Combat is unlocked only after the rendering gate has passed.

The first gameplay milestone will be a complete combat vertical slice:

```text
Input
  ↓
Attack
  ↓
Hitbox
  ↓
Collision
  ↓
Damage
  ↓
Knockback
  ↓
Hitstun
  ↓
KO
  ↓
Stock / Match loop
```

After that, the project will progress toward complete combat, match systems, menus, save data, audio and final optimization.

---

## Documentation

| Document                                                     | Contents                                                  |
| ------------------------------------------------------------ | --------------------------------------------------------- |
| [`AGENTS.md`](AGENTS.md)                                     | Agent operating rules and autonomous development protocol |
| [`PLAN.md`](PLAN.md)                                         | Authoritative development roadmap                         |
| [`STATUS.md`](STATUS.md)                                     | Current execution state and session continuity            |
| [`docs/ssb-architecture.md`](docs/ssb-architecture.md)       | Recovered architecture of the original game               |
| [`docs/reverse-engineering.md`](docs/reverse-engineering.md) | Reverse-engineering investigations and evidence           |
| [`docs/rendering.md`](docs/rendering.md)                     | N64 → PSP rendering implementation                        |
| [`docs/memory.md`](docs/memory.md)                           | Memory layout and allocation                              |
| [`docs/porting-status.md`](docs/porting-status.md)           | Per-subsystem implementation status                       |
| [`DECISIONS.md`](DECISIONS.md)                               | Permanent architectural decisions                         |
| [`TODO.md`](TODO.md)                                         | Discovered future work not yet folded into `PLAN.md`      |

---

## References

These projects are used as technical and architectural references, not as sources for Nintendo assets or blindly copied implementations.

1. [ssb-decomp-re][decomp] — SSB64 decompilation
2. [BattleShip](https://github.com/JRickey/BattleShip) — PC/Mac/Linux/Android SSB64 port based on the decompilation
3. [sf64-psp](https://github.com/TheMrIron2/sf64-psp) — Star Fox 64 PSP port
4. [oot-PSP](https://github.com/z2442/oot-PSP) — Ocarina of Time PSP port
5. [n64psp](https://github.com/TheMrIron2/n64psp) — reusable N64 → PSP runtime
6. [rust-psp][rustpsp] — Rust support for PSP

References 2–5 are technical references, not authorities — see `DECISIONS.md` D-037.

### Local reference setup

The reference repositories can be cloned into the gitignored `refs/` directory:

```bash
mkdir -p refs
cd refs

git clone https://github.com/VetriTheRetri/ssb-decomp-re
git clone https://github.com/JRickey/BattleShip
git clone https://github.com/TheMrIron2/sf64-psp
git clone https://github.com/z2442/oot-PSP
git clone https://github.com/TheMrIron2/n64psp
git clone https://github.com/overdrivenpotato/rust-psp
```

---

## Contributing / Agent Development

This repository is designed to support autonomous AI-assisted development.

The intended workflow is:

> **Continue with the plan.**

The agent reads `AGENTS.md`, `PLAN.md` and `STATUS.md`, resumes the current task, verifies its work, updates documentation and continues through the ordered roadmap.

The repository should always contain enough state for a fresh agent session to continue without relying on previous conversation history.

---

## License

MIT OR Apache-2.0, for the code in this repository only.

This license does not grant rights to Nintendo's intellectual property.

[decomp]: https://github.com/VetriTheRetri/ssb-decomp-re
[rustpsp]: https://github.com/overdrivenpotato/rust-psp
