# Porting Status

Per Rule 11. Percentages are of *intended scope for that subsystem*, not of the
original's line count. A subsystem is only `COMPLETE` when it has been
functionally validated, not merely compiled (Rule 12).

Last updated: 2026-08-27.

## Milestones

| Milestone | Status | Notes |
|---|---|---|
| **M0 — Research** | ✅ COMPLETE | `docs/ssb-architecture.md`, this file, `docs/reverse-engineering.md` |
| **M1 — Rust PSP bootstrap** | 🟡 BUILDS | EBOOT.PBP produced; **PPSSPP boot not yet verified** — see below |
| **M2 — Resource pipeline** | 🟢 60% | Archive + VPK0 done and verified; texture/model conversion pending |
| **M3 — Rendering** | 🔴 5% | DL parser written; no geometry converted yet |
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
| F3DEX2 DL parser | 🟢 70% | Opcodes decoded and unit-tested; segmented-address resolution and traversal not done |
| N64 texture decode | 🟢 70% | RGBA16/32, IA4/8/16, I4/8, CI4/8 decoded and unit-tested; not yet run on real ROM textures |
| Texture → PSP conversion | 🔴 0% | Swizzling and PSM packing not started |
| Model conversion | 🔴 0% | |
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

## Known gaps and honest caveats

1. **The EBOOT has not been observed running.** `cargo psp` produces a valid
   9.6 MB `EBOOT.PBP` with correct PBP magic, in both debug and release. But
   PPSSPP could not initialise a graphics backend in this development
   environment (Vulkan unavailable, no usable window), so the binary has
   never actually been seen to boot. **M1 is not complete until it has.**
   Next step: run it on a desktop session or on real hardware.

2. **Nothing has run on real PSP hardware.** Per §37 of the plan, PPSSPP is
   not proof of hardware behaviour, and neither is a successful link.

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
