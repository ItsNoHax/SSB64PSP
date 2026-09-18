# Current State

Milestone: `F1 — Front End & Training Mode`
Primary task: not yet started
Task state: `TODO`

`F1` (`plans/gameplay/F1.md`) is the current primary task, selected this
session per user instruction: build the intro screen, main menu and Training
Mode next, as a new separate PSP application from the existing debug asset
viewer (`psp/`). It runs parallel to `R3` (rendering performance, still
`NOT_STARTED`, not currently active) — `R3` is not blocked by this choice,
just not the task in progress. `F1`'s training-mode combat sandbox (single
stationary dummy target; real hitbox/hurtbox/damage/knockback/hitstun; no
stocks/KO/match loop/CPU AI/items) is a scoped, explicit exception to the
rendering-gate-before-combat rule — see `AGENTS.md`'s non-negotiable
constraints. Full match combat (`G0`–`G2`) remains `BLOCKED_BY_R3`.

Current objective: **RE-288 (this session): PSP-1000 physically tested,
closing R2.** The attached PSP-1000 (firmware 6.61, ARK/Infinity, PSPLink
v3.2.1) cleanly fails to load the ~25.6 MiB pack: `assets::load_pack`'s
`AlignedBuf::new` allocation fails, returning `LoadError::OutOfMemory`
(`psp/src/assets.rs:122-125`), and the game falls back to its built-in
placeholder tetrahedron — zero exceptions, not a crash. Root cause is
`MEMSIZE=1` being ignored on this hardware class (32 MiB total RAM, vs the
64 MiB PSP-3000/Slim already tested), documented but not previously
hardware-confirmed. Confirmed via a new one-shot, GU-free diagnostic Cargo
feature added this session, `pack_status_overlay` (`psp/src/main.rs`,
`psp/Cargo.toml`) — it prints `pack_status` with `psp::dprintln!` (a raw
framebuffer text path) rather than the existing `debug_overlay` HUD, whose
`sceGuDebugFlush`-based rendering was suspected of a hardware-only crash on
this unit. That suspicion was a false alarm: the observed exception was
stale PSPLink module-manager state left over from a `kill` that wasn't
followed by `reset` (this project's own documented trap, `psp-hardware`
Skill), not a real fault — resetting and reloading cleanly reproduced no
exception at all. This closes the "is PSP-1000 pack compatibility
unresolved" question definitively: it fails to fit, cleanly, with a known
cause — not an open question, and not a rendering bug.

A 30-minute sustained-run attempt on this same PSP-1000 (parity with
RE-284's single-unit sample) was started on the fallback-tetrahedron path
(the only thing that runs, since the pack doesn't load) and reached the
5-minute checkpoint clean (zero exceptions, `main_thread` alive, USB
stable) before being stopped on explicit user instruction: testing a
placeholder path for 30 minutes doesn't validate anything a real game
session would exercise. Per that instruction, PSP-1000 real-content
confirmation and a second unit at the 30-minute duration are both moved out
of R2 and deferred to a future hardware-acceptance pass once real game
UI/scene loading replaces the current debug asset viewer — see `TODO.md`.
**R2 is now `COMPLETE`**; all of its own acceptance criteria
(`plans/rendering/R2.md`) are met, and neither deferred item blocks R3.

**Operational note:** the PSP-1000 dropped off USB mid-session (`lsusb`
stopped listing `054c:01c9`) while a `kill`/`reset` was in flight to shut
the game down cleanly; `usbhostfs_pc` was left spinning "waiting for
device" and was killed. The unit's last known state is running the
fallback tetrahedron (harmless) or powered off — **replug and check it**
before assuming a clean shutdown. This is the same class of USB flakiness
RE-287 already noted (worth a replug before assuming the udev rule itself
is broken), not a new failure mode.

Prior objective: `RE-287` confirmed all 41 stage goldens on physical PSP
hardware, closing stage-coverage. `RE-283` (the reopened fighter-texture
quality gate) is fully traced — all nine user-reported items accounted for.

Current blocker(s): none for R2 (closed this session). R3 has not started;
no blockers recorded yet. Kirby's per-copy-ability hat graphs remain
untested (separate small scene graphs, unreachable until copy-ability
gameplay exists) — not an R2 or R3 blocker, tracked in `TODO.md`.

If `pspsh -e ver` returns "connection refused" despite the PSP showing
`054c:01c9` on USB and PSPLink visibly launched, start
`usbhostfs_pc -v "$PWD"` first — it bridges the USB link `pspsh` actually
connects to. `scrshot host0:<path>` only resolves under the repo root
`usbhostfs_pc` was launched from — an absolute host path after `host0:`
silently fails to write while the PSP still prints a success-looking
`frame_addr ... output host0:/...` line. **After any `kill`, always
`reset` before the next `ldstart`** — RE-288 hit a case where a stale
post-`kill` module-manager state produced a convincing but fake exception
report on the *next* load, wasting a full diagnostic detour before RE-288
traced it back to the missing `reset`.

Required next action: begin F1 (`plans/gameplay/F1.md`) — scaffold the new
`psp-game` crate (separate Cargo.toml/EBOOT from `psp/`), then intro screen,
main menu, and the training-mode combat sandbox. R3 (`plans/rendering/R3.md`)
remains `NOT_STARTED` and eligible to resume any time — F1 does not block it.
PSP-1000 real-content confirmation and a second-unit 30-minute sustained run
are deferred to a future hardware-acceptance pass, now gated on F1 (real
game UI/scene loading) existing (see `TODO.md`); do not resume them before
then.

Relevant PLAN task: [plans/gameplay/F1.md](plans/gameplay/F1.md) (active),
[plans/rendering/R2.md](plans/rendering/R2.md) (closed),
[plans/rendering/R3.md](plans/rendering/R3.md) (parallel, not started)
Relevant evidence: RE-282, RE-283, RE-284, RE-285, RE-286, RE-287, RE-288
(see [docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full
R2.2/physical chain, RE-240–288). Toolchain note: the global `cargo-psp`
install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-288 (this session) adds one new Cargo feature
(`pack_status_overlay`, off by default) and its `#[cfg]`-gated call site in
`psp/src/main.rs`; the plain feature-free build is otherwise unchanged.
Plain feature-free EBOOT hash (PRX):
`81abb86233772aed57bc61c0d3ccefd3c2f09d24746993b41f7fd54de630b62e`. Pack
unchanged this session (no asset-pipeline code touched):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`.
