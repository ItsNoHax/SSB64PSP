# SSB64PSP — Agent Operating Protocol

## Project Overview
Native Rust port of Super Smash Bros. (N64) to Sony PSP. Not an emulator — a reimplementation using the SSB64 decompilation as reference and `rust-psp` for the platform layer.

**Current status:** Engine prototype. Asset pipeline complete and verified. Fighters and stages render at 60 FPS on PSP. Physics, collision, and movement status machine ported and working on device. Attacks, damage, opponents, match loop not implemented. Never run on real hardware.

---

## Architecture (Three Layers)

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

`crates/ssb-rom` sits beside all three: reads ROM formats at build time, runtime pack on device. Linked by both host tools and PSP binary.

| Crate | Purpose | `no_std` | Target |
|-------|---------|----------|--------|
| `crates/ssb-rom` | ROM validation, VPK0, relocData archive, N64 formats, animation, runtime pack | yes (+alloc) | host + PSP |
| `crates/ssb-engine` | Layer B traits, math, coordinate conversion | yes | host + PSP |
| `crates/ssb-game` | Layer A game logic | yes | host + PSP |
| `tools/romtool` | Build-time extraction/verification CLI | no | host |
| `psp/` | Layer C backend + executable | yes | `mipsel-sony-psp` |

`psp/` is **outside** the root cargo workspace (needs pinned nightly + `-Z build-std`). This lets `cargo test` at root run on stable.

---

## Required Toolchain

| Tool | Version/Notes |
|------|---------------|
| Rust stable | Pinned to `1.98.0` (see `.github/workflows/ci.yml:RUST_STABLE`) |
| Rust nightly | Pinned to `nightly-2026-08-01` (see `psp/rust-toolchain.toml`) |
| `cargo-psp` | `cargo install cargo-psp --locked` |
| ROM | `Super Smash Bros. (USA).z64` — SHA-1 `e2929e10fccc0aa84e5776227e798abc07cedabf` |

---

## Common Commands

### Setup & Verification
```bash
# 1. Place ROM (gitignored)
mkdir -p rom && cp "/path/to/Super Smash Bros. (USA).z64" rom/

# 2. Verify ROM is supported revision
cargo run -p romtool -- verify "rom/Super Smash Bros. (USA).z64"

# 3. Inspect archive
cargo run -p romtool -- info "rom/Super Smash Bros. (USA).z64"

# 4. Build runtime asset pack (REQUIRED — PSP build has nothing to draw without it)
cargo run --release -p romtool -- pack "rom/Super Smash Bros. (USA).z64"
# -> assets/generated/ssb64.pak
```

### Testing (Host)
```bash
# Run all host tests (317+ passing)
cargo test --workspace --all-targets

# Run tests for a specific crate
cargo test -p ssb-rom
cargo test -p ssb-engine
cargo test -p ssb-game
```

### Building
```bash
# Build host tools (romtool)
cargo build --release -p romtool

# Build PSP executable (from psp/ directory)
cd psp && cargo psp --release
# -> psp/target/mipsel-sony-psp/release/EBOOT.PBP
```

### Running on PPSSPP
```bash
# Build + run + screenshot (forces software rasteriser for debug overlay)
tools/run-ppsspp.sh

# Options: --no-build, --backend software|opengl, --seconds N
tools/run-ppsspp.sh --no-build --seconds 5
```

### Verification Commands
```bash
# VPK0 decompression cross-verification
cargo run --release -p romtool -- check "rom/Super Smash Bros. (USA).z64"

# Fighter physics constants vs decompilation
cargo run --release -p romtool -- fighters "rom/…z64" --verify

# Animation lengths decoded two ways
cargo run --release -p romtool -- anims "rom/…z64" --verify

# Figatree animations replayed from pack
cargo run --release -p romtool -- figatree "rom/…z64" --frames 40 --pack assets/generated/ssb64.pak

# Stage animations
cargo run --release -p romtool -- stages "rom/…z64" --pack assets/generated/ssb64.pak

# Texture conversion report
cargo run --release -p romtool -- textures "rom/…z64"
```

---

## CI Pipeline (`.github/workflows/ci.yml`)

**Order matters** — mirrors local verification:
1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets`
3. `cargo test --workspace --all-targets`
4. `cargo build --release -p romtool`
5. `no_std` build for `thumbv7em-none-eabi` (catches `std` leakage via default features)
6. PSP build (`cargo psp --release` in `psp/`)
7. ROM commit guard

**Key CI details:**
- `RUSTFLAGS: -D warnings` on host jobs
- `RUST_STABLE=1.98.0` pinned deliberately — bump on purpose, not by surprise
- PSP job uses `psp/rust-toolchain.toml` (nightly-2026-08-01), clears `RUSTFLAGS` (linker warnings)
- `future_lints` job runs on latest stable (non-blocking) to warn of new lints

---

## Critical Conventions & Gotchas

### `no_std` Discipline
- All three core crates (`ssb-rom`, `ssb-engine`, `ssb-game`) are `no_std` with `default = ["std"]` feature
- **Workspace dependencies declare `default-features = false`** (root `Cargo.toml:31-33`)
- A crate **cannot** turn off a workspace dependency's default features — if `std` leaks, PSP build fails with confusing "can't find crate for `std`"
- Crates that want `std` opt in through their own `std` feature
- CI `no_std` job builds all three for `thumbv7em-none-eabi` to catch this

### Asset Pack Is Mandatory
- `cargo psp` builds the executable but **does not** build the asset pack
- `tools/run-ppsspp.sh` stages `assets/generated/ssb64.pak` alongside `EBOOT.PBP`
- Without the pack, the viewer falls back to a built-in tetrahedron (looks like "no assets")

### Debug Overlay Requires Software Rasteriser
- `sceGuDebugFlush` paints VRAM with CPU
- PPSSPP hardware backends don't reflect CPU VRAM writes → overlay invisible
- `tools/run-ppsspp.sh` forces software rasteriser via `--appendconfig`
- This is an emulator limitation, not a port bug (see `docs/reverse-engineering.md` RE-014)

### PSP Toolchain Pinning
- `rust-psp` reaches into unstable `core` internals (`core::panic::PanicPayload`)
- Only builds on narrow band of nightlies
- **Before bumping `psp/rust-toolchain.toml`**: build with new nightly and boot in PPSSPP — successful compile is not sufficient evidence
- Known broken: nightly-2026-08-26+

### ROM Handling
- `rom/` directory is gitignored
- `assets/generated/` is gitignored
- CI rejects any committed ROM files (`.z64`, `.n64`, `.v64`, `.rom`)
- You must supply your own legally obtained ROM dump

---

## Testing Quirks

- Tests that touch the real ROM are gated behind `SSB64_ROM` env var (CI passes without ROM)
- Host tests use synthetic data or assert on constants — no copyrighted material needed in CI
- 317 host tests passing: `ssb-rom` (174), `ssb-engine` (36), `ssb-game` (107)

---

## Key Documentation References

| File | Purpose |
|------|---------|
| `docs/ssb-architecture.md` | Original game architecture from decompilation |
| `docs/reverse-engineering.md` | Open questions with evidence/confidence (RE-XXX IDs) |
| `docs/rendering.md` | N64 → PSP rendering translation |
| `docs/memory.md` | Memory layout and allocator plan |
| `docs/porting-status.md` | Per-subsystem progress (source of truth for status) |
| `PLAN.md` | Milestone plan (Rules 11-12: % = intended scope, COMPLETE = validated) |

---

## Common Failure Modes to Avoid

1. **Running `cargo test` from `psp/`** — runs on nightly, not stable; use root workspace
2. **Forgetting to rebuild asset pack** after ROM/tool changes — stale pack silently used
3. **Running `cargo psp` from repo root** — exits 0 without rebuilding; must run from `psp/`
4. **Using OpenGL backend in PPSSPP** for debug overlay work — overlay invisible
5. **Enabling `std` feature transitively** — breaks PSP build; check `no_std` CI job
6. **Assuming PPSSPP = real hardware** — biggest open risk, never validated on device
7. **Leaving `SoftwareRenderer = True` in PPSSPP config** — script now snapshots/restores but SIGKILL leaves it set

---

## Current Milestone
**Combat vertical slice** — one grounded attack driven end-to-end (input → hitbox → knockback). Everything underneath is in place: fighter with real model, colours, physics, collision, animations, standing on real stage.

---

## Agent Operating Rules

### Rule 1 — Decompilation First
Always inspect the existing SSB decompilation before implementing functionality that already exists there. The decompilation is 100% complete — every question about original behavior has an answer in the source.

### Rule 2 — Reference Ports First
Always inspect `sf64-psp` and `n64psp` before designing an N64→PSP subsystem.

### Rule 3 — No Invention
Never invent N64 behavior when the decompilation can answer the question.

### Rule 4 — Subsystem by Subsystem
Do not perform giant speculative rewrites. Work subsystem-by-subsystem.

### Rule 5 — Functional Validation
Every major subsystem must compile and have a test/debug path before moving on.

### Rule 6 — Layer Separation
Keep platform code separate from game logic. Game logic never mentions PSP; PSP backend never mentions fighters.

### Rule 7 — Rust Abstractions
Prefer Rust abstractions over C-style global state where practical.

### Rule 8 — Unsafe Discipline
Do not introduce `unsafe` without a concrete reason. When unsafe is required for PSP APIs/VFPU/GPU memory, isolate it behind small safe abstractions.

### Rule 9 — Profile First
Profile before optimizing. Especially VFPU code.

### Rule 10 — Document Uncertainty
When uncertain about original behavior, document the uncertainty rather than guessing. Create `docs/reverse-engineering.md` entries with: Question, Evidence, Hypothesis, Implementation, Confidence.

### Rule 11 — Porting Status
Maintain a porting-status document (`docs/porting-status.md`). Percentages are of *intended scope for that subsystem*, not of the original's line count. A subsystem is only `COMPLETE` when it has been functionally validated, not merely compiled.

### Rule 12 — Milestone Validation
Do not move to the next milestone merely because the code compiles. Each milestone requires functional validation.

---

## License
MIT OR Apache-2.0 for code in this repository only. No rights to Nintendo's IP.