# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v121 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 225 of 225 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy) | Room 13.39 ms, Run 14.98, Yoster 15.58, Clash 13.17 | RE-530, RE-532, RE-539 |
| PSP-2000 `vs4@2400` | 16.92 ms busy, BFPS 59.1 | RE-539 |
| PSP-2000 1P intro | ~34 ms (~21 FPS) | RE-537, RE-539 |

## Current work

- Landed: Kirby roster-A2 entry camera (RE-543).
  - Entry zoom reads the live fighter position each camera tick.
  - 8 entry-window goldens rebaselined.
  - Open: N64 camera eye/at trace, rec 320-340.

## Next

1. N64 camera eye/at trace for the A2 entry, rec 320-340 (RE-543).
2. Thunder Jolt burst spikes (RE-541).
3. Effects process order (RE-542).
4. MS8 list in `PLAN.md`, 1P intro first.
5. `effect.rs` `QUAKE`/`THUNDER_SHOCK` → `ssb-tablegen` AnimJoint player.

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: RE-543 entry camera.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | 225/225, 8 entry-window rows rebaselined (`r2-mario-entry-pipe` nondeterministic, RE-506) |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
