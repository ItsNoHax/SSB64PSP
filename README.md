# SSB64PSP

Native Rust source port of **Super Smash Bros.** (N64, US) to the **Sony PSP**. Not an emulator.

- Gameplay: Rust translated from the community [decompilation][decomp]
- Build time: game data and N64 display lists read from your own ROM, converted to PSP-native form

## No copyrighted assets are included in this repository

- No Nintendo asset checked in or distributed: code, textures, audio, models, text, ROM data
- Only published build: CI asset-viewer EBOOT, compiled without a ROM, no game data
- ROM: supplied by you, legally obtained
- Build extracts from the ROM into gitignored output ([D-048](docs/decisions/D-048.md)):
  - asset pack: `assets/generated/`
  - data tables the game reads: Cargo `target/`
- A build you make contains ROM data: own use only
- No legal copy of the N64 game: cannot build or run
- Repository holds source, tools, documentation only
- Gameplay code: own translation of decompiled logic, constants (offsets, ids, timings) as source
- Golden captures: pixel hashes, not images
- XMB icons, backgrounds, music: original work ([XMB assets](docs/xmb-assets.md))

## Requirements

- ROM: your own `Super Smash Bros. (USA).z64` (big-endian)

| Game code | SHA-1 | MD5 |
|---|---|---|
| `NALE` | `e2929e10fccc0aa84e5776227e798abc07cedabf` | `f7c52568a31aadf26e14dc2b6416b2ed` |

| Tool | Version source |
|---|---|
| Rust stable (host crates, tools) | `rust-toolchain.toml` |
| Rust nightly (PSP crates) | `rust-toolchain.toml` of `psp-game/` and `psp-asset-viewer/`; `psp-runtime` builds under them |
| `cargo-psp` | project `rust-psp` fork, revision CI pins |
| Hardware | PSP-2000 or later (64 MiB mode), or [PPSSPP](https://www.ppsspp.org/) |

```bash
cargo +nightly-2026-08-26 install --git https://github.com/ItsNoHax/rust-psp --rev a89142b237f3014fc15425d3549e44d3aa07d1c6 cargo-psp --locked
```

## Build

- ROM location: `rom/`, or `SSB64_ROM`
- Every build reads the ROM: build scripts of `ssb-game` and `ssb-rom` verify SHA-1, generate tables (`crates/ssb-tablegen`)
- No ROM → build stops, prints how to supply one

```bash
# 1. The ROM
mkdir -p rom && cp "/path/to/Super Smash Bros. (USA).z64" rom/
cargo run -p romtool -- verify "rom/Super Smash Bros. (USA).z64"

# 2. The asset pack: models, textures, animations, sprites, stages
cargo run --release -p romtool -- pack "rom/Super Smash Bros. (USA).z64"
#    -> assets/generated/ssb64.pak

# 3. Host tests
cargo test --workspace

# 4. The PSP applications (each is its own cargo-psp crate)
(cd psp-game && cargo psp --release)          # the game
(cd psp-asset-viewer && cargo psp --release)  # debug and render-validation viewer
#    -> <crate>/target/mipsel-sony-psp/release/EBOOT.PBP
```

- `cargo run -p ssb-tablegen -- DIR`: writes generated tables into `DIR` for inspection
- `SSB64_STUB_TABLES=1`: builds empty stub tables; CI type-checks and tests without a ROM
- Stub build cannot run the game

## Run

- PSP: copy `EBOOT.PBP` and `ssb64.pak` into one `PSP/GAME/<folder>/`
- Save: `ssb64.sav` beside them
- Pack needs 64 MiB mode (PSP-2000 or later)
- PPSSPP:

```bash
tools/run-ppsspp.sh --crate psp-game                        # interactive
tools/run-ppsspp-headless.sh --crate psp-game --feature F   # deterministic capture
```

- Both scripts default to `psp-asset-viewer`
- PPSSPPHeadless, golden captures: [visual regression](docs/visual-regression/README.md)

### Controls

| PSP | N64 |
|---|---|
| Analog nub | Control Stick |
| D-pad Up/Down/Left/Right | C-Up/C-Down/C-Left/C-Right |
| Cross / Square | A / B |
| L / R | Z / R |
| Circle | L |
| Start | Start |

- Triangle, Select: unbound
- Battle: any C-button jumps; N64 R acts as A + Z
- Menu Training entry: D-pad (C-button) tap picks the costume, as character select does

## Layout

| Path | Holds |
|---|---|
| `crates/` | Portable code: gameplay, engine, ROM and pack formats, capture specs, build-time table generation |
| `tools/romtool` | Pack builder |
| `psp-runtime/`, `psp-game/`, `psp-asset-viewer/` | PSP code; outside the Cargo workspace ([D-026](docs/decisions/D-026.md)) |

Crate roles and boundaries: [architecture](docs/architecture.md), [AGENTS.md](AGENTS.md).

## Documentation

| Document | Contents |
|---|---|
| [`PLAN.md`](PLAN.md) | Project plan: completed work, remaining work, target state |
| [`STATUS.md`](STATUS.md) | Current project snapshot |
| [`DECISIONS.md`](DECISIONS.md) | Permanent architectural decisions |
| [`docs/architecture.md`](docs/architecture.md) | Current and target architecture; original-game reference |
| [`docs/rendering.md`](docs/rendering.md) | Rendering architecture and per-domain detail |
| [`docs/memory.md`](docs/memory.md) | Memory layout and constraints |
| [`docs/xmb-assets.md`](docs/xmb-assets.md) | XMB icon, background and music assets |
| [`docs/visual-regression/`](docs/visual-regression/README.md) | Golden and visual regression testing |
| [`docs/evidence/`](docs/evidence/INDEX.md) | Durable technical findings |
| [`AGENTS.md`](AGENTS.md) | Development and agent rules |

## References

Technical references only; none outranks decompilation and ROM ([D-037](docs/decisions/D-037.md)).

- [ssb-decomp-re][decomp] — SSB64 decompilation (primary source)
- [BattleShip](https://github.com/JRickey/BattleShip) — native PC SSB64 port
- [sf64-psp](https://github.com/TheMrIron2/sf64-psp), [oot-PSP](https://github.com/z2442/oot-PSP), [n64psp](https://github.com/TheMrIron2/n64psp) — N64 → PSP ports
- [rust-psp][rustpsp] — Rust PSP support (project fork)

Local use: clone into gitignored `refs/`.

## License

- MIT OR Apache-2.0, code in this repository only
- No rights to Nintendo intellectual property granted
- Super Smash Bros.: trademark of Nintendo

[decomp]: https://github.com/VetriTheRetri/ssb-decomp-re
[rustpsp]: https://github.com/ItsNoHax/rust-psp
