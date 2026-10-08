# SSB64PSP

A native Rust source port of **Super Smash Bros. (N64)** to the **Sony PSP**.
Not an emulator: gameplay is translated from the
[SSB64 decompilation][decomp], and N64 display lists are converted to PSP GE
geometry at build time.

> **Status:** in development. All 12 fighters, Master Hand and the special
> fighters, VS and Training, the 1P campaign through the ending, save data and
> the front end (title, opening movie, attract demos, menus) run; audio and
> rumble are not started. See [`PLAN.md`](PLAN.md) for what is done and what
> remains and [`STATUS.md`](STATUS.md) for current work.

## No ROM or extracted assets are included in this repository

No Nintendo ROM, extracted asset files (textures, models, animations, audio,
text) or asset pack are checked into this repo, and no build contains them:
the EBOOT holds compiled code only, and the asset pack is a separate file you
generate yourself.

You supply your own legally obtained ROM. `romtool pack` extracts and
converts its assets locally into `assets/generated/ssb64.pak`, which is
gitignored along with `rom/`.

If you do not own a legal copy of Super Smash Bros. for the Nintendo 64, you
cannot build the asset pack or run this project.

The repository contains the source code and tools required to perform the
conversion locally. Be aware of what that code is:

- The gameplay code is a Rust translation of the community decompilation
  ([`ssb-decomp-re`][decomp]).
- Tables the original keeps in its code and data (fighter motion scripts,
  colour-animation scripts, CPU input scripts, status flags, animation file
  tables) are generated from the decompilation's sources by `tools/gen-*.py`
  and committed as Rust (`crates/ssb-game/src/motion/scripts.rs`,
  `colanim_scripts.rs`, `computer/scripts.rs`, `spgame/stat_flags.rs`,
  `crates/ssb-rom/src/anim_table.rs`); they are compiled into the EBOOT.
- `tests/golden/` holds PNG screenshots of the port's own output for
  regression testing; they show the game's characters and stages.
- The XMB icons, backgrounds and music in `psp-game/xmb/` and
  `psp-asset-viewer/xmb/` are original work, not taken from the game
  ([XMB assets](docs/xmb-assets.md)).

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
The XMB icon, animated icon and backgrounds come from each crate's `xmb/`
([XMB assets](docs/xmb-assets.md)).

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
| `crates/ssb-game` | Portable gameplay: fighters, items, stages, CPU, menus, game modes | host + PSP |
| `crates/ssb-capture` | Golden-capture scene specs | host + PSP |
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
Nintendo's intellectual property are granted.

[decomp]: https://github.com/VetriTheRetri/ssb-decomp-re
[rustpsp]: https://github.com/overdrivenpotato/rust-psp
