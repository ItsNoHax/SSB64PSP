---
name: n64-emulator
description: Need to observe or navigate the original N64 game live (not just read decomp source) — menu navigation, settling a scene, live RDRAM values, or a reference screenshot. Activates for "what does the original do when I press X", "navigate to Y in the real game", or any claim about original-game behavior that static code reading can't settle.
---

# N64 emulator (headless, scripted)

Use this skill (headless Mupen64Plus) whenever you need the real N64 game
live — not PSP output. For PSP visual/rendering testing use
[visual-regression](../visual-regression/SKILL.md) (PPSSPPHeadless); for real
PSP hardware testing use [psp-hardware](../psp-hardware/SKILL.md) (PSPLink).

Use this when the decompilation's *static* behavior is ambiguous or
insufficient — e.g. runtime-only state (camera settle after N ticks,
animation timing, live struct values) or reaching a specific menu/gameplay
state to screenshot. For static behavior, [reverse-engineering](../reverse-engineering/SKILL.md)
(reading the decomp/ROM) is faster and should be tried first.

## Tool

`tools/run-n64-headless.sh` drives the real Super Smash Bros. 64 ROM
(`rom/Super Smash Bros. (USA).z64`) through Mupen64Plus's public Core API
inside the M64Py flatpak sandbox — frame-exact scripted button input, no
GUI automation, no real controller. It is a promoted/generalized version of
the harness built in RE-151/RE-216/RE-274 (`docs/evidence/re/`).

By default no window appears anywhere (`--env=SDL_VIDEODRIVER=offscreen`,
see the Gotchas entry below for why this is the only setting that actually
achieves that). Pass `--visible` to render on the real desktop instead
(only useful for interactively watching a route play out).

```
tools/run-n64-headless.sh --frames 120                      # idle smoke test
tools/run-n64-headless.sh --route "60:start;30:;10:a" \
    --screenshot-every 30 --final-screenshot                # navigate + capture
```

`--route` is a `;`-separated list of `hold:button+button` steps applied to
P1 (buttons: `a,b,start,z,l,r,dpad_u,dpad_d,dpad_l,dpad_r,c_u,c_d,c_l,c_r`;
empty button list = idle for `hold` frames). Screenshots land under
`~/.var/app/net.sourceforge.m64py.M64Py/data/mupen64plus/screenshot/`
(override with `--data-dir`). Driver logs (`[driver] ...`) go to stderr and
show plugin attach and every screenshot path — read them, don't guess
whether a step landed.

For scripts needing RDRAM reads/writes or custom step sequences beyond what
`--route` expresses, import `Mupen64PlusHarness` from
`tools/n64-headless/n64_driver.py` directly (see `read_rdram`/`read_u32`/
`read_f32`/`write_rdram` and `run_steps`) and run it the same way:
`flatpak run --command=python3 net.sourceforge.m64py.M64Py <your_script.py>`.

## Known-good route landmarks (from RE-151/RE-216)

- `Start` at the title/mode-select screens is a hardcoded shortcut straight
  into 1P Mode regardless of cursor position — use `A` to confirm a
  highlighted option instead.
- A VS Mode match with both fighters idle settles into a stable pose by
  frame ~1150–1800 (RE-151's Mario/Pikachu/Dream Land reference route).

## Gotchas

- **Flatpak sandbox required.** The core/plugins only exist inside
  `net.sourceforge.m64py.M64Py`'s `/app/lib`; there is no host-installed
  Mupen64Plus. Always go through `flatpak run --command=python3
  net.sourceforge.m64py.M64Py ...`, not a bare `python3`.
- **A `DISPLAY`/Xvfb override does NOT make this headless — do not
  reintroduce that approach.** `mupen64plus-video-rice.so` opens its render
  surface via SDL2, not Qt (M64Py's GUI is never launched here). SDL2
  auto-prefers Wayland over X11 when both are available, and the flatpak
  sandbox always has the real compositor's Wayland socket mounted — so
  overriding `$DISPLAY` (even pointed at a private Xvfb server) has no
  effect: SDL ignores it and opens a real, focusable window on the actual
  desktop anyway (confirmed by testing). The only setting that suppresses
  the window is forcing SDL's `offscreen` driver via
  `flatpak run --env=SDL_VIDEODRIVER=offscreen ...`, which is what
  `run-n64-headless.sh` does by default. Screenshot capture and RDRAM access
  both work unchanged under `offscreen`.
- **`ctypes` `argtypes` are mandatory**, not optional cleanup: every Core
  API call passing a dynlib handle or pointer needs explicit `c_void_p`
  argtypes. Without them `ctypes` truncates the 64-bit handle to 32 bits and
  `PluginStartup` segfaults (RE-216 root-caused this once — see
  `n64_driver.py`'s `start()` for the reference-correct declarations).
  `mupen64plus-video-angrylion-plus.so` under the *other* installed flatpak
  (Rosalie's Mupen GUI) still segfaults in `PluginStartup` even with this
  fix applied — that is a distinct, unresolved plugin-specific issue
  (RE-275); this tool uses M64Py's `mupen64plus-video-rice.so`, which works.
- **Sandbox filesystem access.** The flatpak needs read access to this repo
  (for the ROM/scripts) and write access to a scratch dir for
  screenshots/config. Check with `flatpak info --show-permissions
  net.sourceforge.m64py.M64Py`; grant a missing path with `flatpak override
  --user --filesystem=<path> net.sourceforge.m64py.M64Py` (reversible via
  `flatpak override --user --reset net.sourceforge.m64py.M64Py`).
- **PPSSPP is not physical PSP proof** (see [psp-hardware](../psp-hardware/SKILL.md)),
  and symmetrically, **this emulator capture is not a physical N64/real-hardware
  reference** — it is a convenient, high-fidelity software approximation
  (Rice plugin). Treat divergences at the level of exact RDP/RSP filtering
  quirks with the same caution PPSSPP software-vs-hardware captures get.
- ROM identity: SHA-1 `e2929e10fccc0aa84e5776227e798abc07cedabf`, MD5
  `F7C52568A31AADF26E14DC2B6416B2ED` — verify before trusting a route if the
  ROM file may have changed.
- **Occasional single-frame stall.** `ADVANCE_FRAME` can rarely (roughly
  1-in-5 to 1-in-8 runs, observed on a 300+ idle-frame hold) exceed the
  driver's 5s per-frame timeout and raise `TimeoutError`, even fully
  headless with no user interaction — not yet root-caused (Python-thread
  scheduling jitter is the leading suspect). It does not leak the flatpak
  process; a bare retry of the same command has always succeeded in
  testing. Treat it as a retryable flake, not a route bug, unless it
  reproduces on the same route every time. **The rate is much worse under
  host CPU contention** — one RE-276 session saw the failure rate spike to
  roughly 85-90% (12-20+ retries needed per run) with a background Chrome
  process pinning a CPU core, on an otherwise-idle 24-core machine (load
  average ~1, plenty of spare cores). A tight retry loop still eventually
  succeeds; it just costs more attempts. This is also the strongest
  practical reason to prefer the decomp-rebuild warp technique below over
  a long scripted `--route` when the host is under load: fewer total frames
  means fewer chances to hit the stall.

## Reaching a specific game state: menu navigation vs. decomp rebuild

For a state reachable by a short, known button sequence, script it with
`--route` directly. For a state that's expensive to reach by menu (deep
menu chains, or this session's host was flaky enough that even a ~900-frame
idle-to-title route needed 10-20 retries), **don't try to RDRAM-warp
`gSCManagerSceneData.scene_curr`** (`refs/ssb-decomp-re/src/sc/scmanager.c`,
US address `0x800a4ad0`) **from outside** — `scManagerRunLoop`'s
`while(TRUE) { switch(scene_curr) { ... } }` only re-reads `scene_curr` once
per *scene lifetime* (each case blocks inside its own per-frame loop,
potentially thousands of frames, and only returns when it decides on its
own that the user finished it), not once per video frame. An external write
lands in memory (reads back correctly) but sits inert until the current
scene organically exits and overwrites it with its own hardcoded target —
confirmed by direct measurement, RE-276.

**What works:** patch `refs/ssb-decomp-re` locally (temporary, revert with
`git checkout --` after use — never commit), rebuild
(`PATH=~/ppsspp-test/mips-bin-shim:$PATH make -j$(nproc)`, see Toolchain
setup below), and run the patched `build/smashbrothers.us.z64` through this
skill's harness instead of the real `rom/`. Writing the target state inside
`scManagerRunLoop` right after its own
`gSCManagerSceneData = dSCManagerDefaultSceneData;` reset (so the write is
part of the game's own boot path, not a race against it) reliably lands on
the *first* attempt — RE-276's scene-warp and camera-warp runs both
succeeded first-try, against 12-20+ retries for menu navigation to the same
depth. `SCCommonData`'s field layout (`src/sc/sctypes.h`) has to be
hand-offset (u8 fields pack tightly, u16/u32 naturally aligned, no debug
symbols carry struct offsets) — RE-276 has a worked example including
`training_man_fkind`/`training_com_fkind` (fighter/CPU select) and forcing
Training Mode's edit-triggered "Close-Up" camera call
(`gmCameraSetStatusPlayerZoom`) directly, since that field is edge-triggered
on live controller input inside the pause menu and doesn't respond to a
static poke at all.

### Toolchain setup (once per machine)

`refs/ssb-decomp-re/installDependencies.sh` assumes Debian package names
(`apt`). On Fedora/Nobara: `sudo dnf install clang glibc-devel.i686
binutils-mips64-linux-gnu`, then `git submodule update --init --recursive`
(asm-processor/asm-differ), `bash tools/ido-irix4/provision.sh` (IRIX4
frontend `ovl8_8.c` needs), and manually fetch IDO 5.3 the same way
`installDependencies.sh` fetches 7.1 if `tools/ido-recomp/5.3/cc` is
missing. Fedora's cross-binutils package uses the `mips64-linux-gnu-*`
prefix, not the `mips-elf`/`mips-linux-gnu` prefix the Makefile probes for
(`LD_MIPS := $(shell command -v mips-linux-gnu-ld ...)`) — shim it with
symlinks under a dir on `$PATH` (e.g. `~/ppsspp-test/mips-bin-shim/`), but
make `mips-linux-gnu-ld` a wrapper script forcing
`exec mips64-linux-gnu-ld -m elf32btsmip "$@"` rather than a bare symlink:
the real toolchain's default link emulation isn't the plain 32-bit target
the Makefile's un-flagged `$(LD) -Map ... -o $@ $(LDFLAGS)` call expects,
and objects fail to link with "ABI is incompatible with that of the
selected emulation" otherwise. Verify the toolchain before trusting any
patched build: `make init -j$(nproc)` clean, then confirm
`build/smashbrothers.us.z64` is byte-identical to `baserom.us.z64`
(`sha1sum`, expect `e2929e10fccc0aa84e5776227e798abc07cedabf`) *before*
applying any patch.

## Recording findings

If a session establishes new original-game behavior this way, write it up
as a `docs/evidence/re/RE-XXX.md` record per [reverse-engineering](../reverse-engineering/SKILL.md)
step 6 — the harness run itself is not evidence until the observed
behavior is written down with the route/frame numbers that produced it.
