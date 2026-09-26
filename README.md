# SSB64PSP

A native Rust source port of **Super Smash Bros. (N64)** to the **Sony PSP**.
Not an emulator: gameplay is translated from the
[SSB64 decompilation][decomp], and N64 display lists are converted to PSP GE
geometry at build time.

![Dream Land on PSP](docs/images/m4-stage-textured.png)

> **Status:** early development. Stages and fighters render at 60 FPS. Mario's
> and Fox's movesets are ported and run in a Training sandbox against a dummy.
> No full match, CPU AI, items, real menus or audio yet. See [`docs/porting-status.md`](docs/porting-status.md)
> for per-subsystem detail and [`STATUS.md`](STATUS.md) for current work.

## Legal

This repository contains no Nintendo code, ROM or game assets. You must supply
your own legally obtained ROM. Assets are extracted locally into
`assets/generated/`, which is gitignored along with `rom/`.

## Requirements

- Rust stable for host tools and tests (pinned by `rust-toolchain.toml`)
- Rust nightly for the PSP crates (pinned by their own `rust-toolchain.toml`)
- `cargo-psp` from the project's `rust-psp` fork, at the revision CI pins:
  `cargo +nightly-2026-08-26 install --git https://github.com/ItsNoHax/rust-psp --rev a89142b237f3014fc15425d3549e44d3aa07d1c6 cargo-psp --locked`
- `Super Smash Bros. (USA).z64`:

| Game code | SHA-1 | MD5 |
|---|---|---|
| `NALE` | `e2929e10fccc0aa84e5776227e798abc07cedabf` | `f7c52568a31aadf26e14dc2b6416b2ed` |

## Build

```bash
# 1. Verify the ROM and build the runtime asset pack
mkdir -p rom && cp "/path/to/Super Smash Bros. (USA).z64" rom/
cargo run -p romtool -- verify "rom/Super Smash Bros. (USA).z64"
cargo run --release -p romtool -- pack "rom/Super Smash Bros. (USA).z64"
#    -> assets/generated/ssb64.pak

# 2. Host tests
cargo test --workspace

# 3. PSP applications (each is its own cargo-psp crate)
(cd psp-game && cargo psp --release)          # the game
(cd psp-asset-viewer && cargo psp --release)  # debug / render-validation viewer
#    -> <crate>/target/mipsel-sony-psp/release/EBOOT.PBP
```

Copy `EBOOT.PBP` and `ssb64.pak` into the same `PSP/GAME/<folder>/`. The pack
needs the 64 MiB mode of a PSP-2000 or later; it does not fit on a PSP-1000.

## Run

```bash
tools/run-ppsspp.sh [--crate psp-game]                     # interactive PPSSPP
tools/run-ppsspp-headless.sh [--crate psp-game] --feature F  # deterministic capture
```

Both default to `psp-asset-viewer`. See
[`docs/visual-regression/README.md`](docs/visual-regression/README.md) for
PPSSPPHeadless setup and golden comparisons.

### `psp-game` controls

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
| `crates/ssb-game` | Portable gameplay: fighters, physics, collision, match logic | host + PSP |
| `tools/romtool` | ROM verification, extraction, conversion, pack generation | host |
| `psp-runtime/` | Shared PSP backend: GE rendering, input, timing, asset loading | PSP |
| `psp-game/` | The game application (front end, Training) | PSP |
| `psp-asset-viewer/` | Asset browser and deterministic render-regression scenes | PSP |

The portable crates never depend on `psp-runtime`. The three PSP crates sit
outside the Cargo workspace because they build with a pinned nightly and
`-Z build-std` ([D-026](docs/decisions/D-026.md)).

## Documentation

| Document | Contents |
|---|---|
| [`STATUS.md`](STATUS.md) | Current batch and blockers |
| [`PLAN.md`](PLAN.md) | Roadmap (milestones `P0`–`P5`) |
| [`docs/porting-status.md`](docs/porting-status.md) | Per-subsystem status |
| [`TODO.md`](TODO.md) | Deferred work |
| [`DECISIONS.md`](DECISIONS.md) | Architectural decisions |
| [`docs/ssb-architecture.md`](docs/ssb-architecture.md) | How the original game is structured |
| [`docs/rendering.md`](docs/rendering.md) | N64 → PSP rendering |
| [`docs/memory.md`](docs/memory.md) | Memory budget and layout |
| [`docs/visual-regression/README.md`](docs/visual-regression/README.md) | Golden-capture methodology |
| [`docs/evidence/INDEX.md`](docs/evidence/INDEX.md) | Reverse-engineering and investigation records |
| [`AGENTS.md`](AGENTS.md) | Contributor and agent workflow |

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
Nintendo's intellectual property are granted.

[decomp]: https://github.com/VetriTheRetri/ssb-decomp-re
[rustpsp]: https://github.com/overdrivenpotato/rust-psp
