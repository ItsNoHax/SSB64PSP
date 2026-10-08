# SSB64PSP

A native Rust source port of **Super Smash Bros.** (Nintendo 64, US) to the
**Sony PSP**. It is not an emulator. The gameplay code is Rust translated from
the community [decompilation][decomp] of the game's program. At build time,
the game's data and its N64 display lists are read from your own ROM and
converted into PSP-native form.

## No copyrighted assets are included in this repository

None of Nintendo's assets (code, textures, audio, models, text, ROM data) are
checked into this repo or distributed with builds. The only build the
project publishes, CI's asset-viewer EBOOT, is compiled without a ROM and
holds no game data.

You supply your own legally obtained ROM. Assets are extracted locally from
the ROM during the build process and generated into gitignored output. That
output is the asset pack in `assets/generated/` and the data tables the game
code reads, which go to Cargo's `target/` directory
([D-048](docs/decisions/D-048.md)). A build you make therefore contains data
from your ROM and is for your own use only.

If you do not own a legal copy of Super Smash Bros. for the Nintendo 64, you
cannot build or run this project.

The repository contains only the source code, tools and documentation
required to perform the conversion locally. No Nintendo-owned assets or
data extracted from the ROM are included or distributed by this repository.
The gameplay code is the project's own Rust translation of the decompiled
game logic, not Nintendo's code; like the functions it translates, it
carries their constants (offsets, ids, timings) as source. Golden test
captures are stored as pixel hashes, not images. The XMB icons, backgrounds
and music are original work ([XMB assets](docs/xmb-assets.md)).

## Requirements

- Your own `Super Smash Bros. (USA).z64` (big-endian):

| Game code | SHA-1 | MD5 |
|---|---|---|
| `NALE` | `e2929e10fccc0aa84e5776227e798abc07cedabf` | `f7c52568a31aadf26e14dc2b6416b2ed` |

- Rust stable for the host crates and tools (pinned by `rust-toolchain.toml`)
- Rust nightly for the PSP crates (pinned by their own `rust-toolchain.toml`)
- `cargo-psp` from the project's `rust-psp` fork, at the revision CI pins:
  `cargo +nightly-2026-08-26 install --git https://github.com/ItsNoHax/rust-psp --rev a89142b237f3014fc15425d3549e44d3aa07d1c6 cargo-psp --locked`
- A PSP-2000 or later (the game needs its 64 MiB mode), or
  [PPSSPP](https://www.ppsspp.org/)

## Build

Put the ROM in `rom/`, or point `SSB64_ROM` at it. Every build reads it:
the build scripts of `ssb-game` and `ssb-rom` verify its SHA-1 and generate
the tables they compile from it (`crates/ssb-tablegen`). Without a ROM the
build stops and says how to supply one.

```bash
# 1. The ROM
mkdir -p rom && cp "/path/to/Super Smash Bros. (USA).z64" rom/
cargo run -p romtool -- verify "rom/Super Smash Bros. (USA).z64"

# 2. The asset pack: models, textures, animations, sprites, stages
cargo run --release -p romtool -- pack "rom/Super Smash Bros. (USA).z64"
#    -> assets/generated/ssb64.pak

# 3. Host tests (one thread: the game's RNG is a single global seed)
cargo test --workspace -- --test-threads=1

# 4. The PSP applications (each is its own cargo-psp crate)
(cd psp-game && cargo psp --release)          # the game
(cd psp-asset-viewer && cargo psp --release)  # debug and render-validation viewer
#    -> <crate>/target/mipsel-sony-psp/release/EBOOT.PBP
```

`cargo run -p ssb-tablegen -- DIR` writes the generated tables into `DIR` for
inspection. `SSB64_STUB_TABLES=1` builds empty stub tables instead; CI uses
it to type-check and test without a ROM. A stub build cannot run the game.

## Run

Copy `EBOOT.PBP` and `ssb64.pak` into the same `PSP/GAME/<folder>/`; the
game saves to `ssb64.sav` beside them. The pack needs the 64 MiB mode of a
PSP-2000 or later. Or run it under PPSSPP:

```bash
tools/run-ppsspp.sh --crate psp-game                        # interactive
tools/run-ppsspp-headless.sh --crate psp-game --feature F   # deterministic capture
```

Both scripts default to `psp-asset-viewer`.
[Visual regression](docs/visual-regression/README.md) covers PPSSPPHeadless
and the golden captures.

### Controls

| PSP | N64 |
|---|---|
| Analog nub | Control Stick |
| D-pad Up/Down/Left/Right | C-Up/C-Down/C-Left/C-Right |
| Cross / Square | A / B |
| L / R | Z / R |
| Circle | L |
| Start | Start |

Triangle and Select are unbound. In battle any C-button jumps, and N64 R
acts as A + Z. On the menu's Training entry, a D-pad (C-button) tap picks
the costume, as the character select does.

## Layout

| Path | Role | Target |
|---|---|---|
| `crates/ssb-rom` | ROM validation, relocData archive, VPK0, N64 formats, asset pack | host + PSP |
| `crates/ssb-engine` | Engine traits, math, coordinate conversion, timing | host + PSP |
| `crates/ssb-game` | Portable gameplay: fighters, items, stages, CPU, menus, game modes | host + PSP |
| `crates/ssb-capture` | Golden-capture scene specs | host + PSP |
| `crates/ssb-tablegen` | Build-time generation of the tables read from the ROM | host (build scripts) |
| `tools/romtool` | ROM verification, extraction, conversion, pack generation | host |
| `psp-runtime/` | Shared PSP backend: GE rendering, input, timing, asset loading | PSP |
| `psp-game/` | The game application | PSP |
| `psp-asset-viewer/` | Asset browser and deterministic render-regression scenes | PSP |

The portable crates never depend on `psp-runtime`. The three PSP crates sit
outside the Cargo workspace because they build with a pinned nightly and
`-Z build-std` ([D-026](docs/decisions/D-026.md)).

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

Technical references only; none is an authority over the decompilation and
ROM ([D-037](docs/decisions/D-037.md)).

- [ssb-decomp-re][decomp] — SSB64 decompilation (primary source)
- [BattleShip](https://github.com/JRickey/BattleShip) — native PC SSB64 port
- [sf64-psp](https://github.com/TheMrIron2/sf64-psp), [oot-PSP](https://github.com/z2442/oot-PSP), [n64psp](https://github.com/TheMrIron2/n64psp) — N64 → PSP ports
- [rust-psp][rustpsp] — Rust PSP support

Clone them into the gitignored `refs/` directory for local use.

## License

MIT OR Apache-2.0, for the code in this repository only. No rights to
Nintendo's intellectual property are granted. Super Smash Bros. is a
trademark of Nintendo.

[decomp]: https://github.com/VetriTheRetri/ssb-decomp-re
[rustpsp]: https://github.com/overdrivenpotato/rust-psp
