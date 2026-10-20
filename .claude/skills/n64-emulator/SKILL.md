---
name: n64-emulator
description: Need to observe or navigate the original N64 game live (not just read decomp source) — menu navigation, settling a scene, live RDRAM values, or a reference screenshot. Activates for "what does the original do when I press X", "navigate to Y in the real game", or any claim about original-game behavior that static code reading can't settle.
---

# N64 emulator (headless, scripted)

- Headless Mupen64Plus running the real game
- Use for: settle timing, live struct values, screenshots of a menu/gameplay state
- Static reading first: [reverse-engineering](../reverse-engineering/SKILL.md)
- PSP rendering: [visual-regression](../visual-regression/SKILL.md)
- Physical PSP: [psp-hardware](../psp-hardware/SKILL.md)

## Tool

- `tools/run-n64-headless.sh`: drives `rom/Super Smash Bros. (USA).z64` via Core API in the M64Py flatpak sandbox
- Input: frame-exact scripted P1, no GUI automation
- Harness records: RE-151, RE-216, RE-274
- Window: none by default (`--env=SDL_VIDEODRIVER=offscreen`); `--visible` only to watch a route

```
tools/run-n64-headless.sh --frames 120                      # idle smoke test
tools/run-n64-headless.sh --route "60:start;30:;10:a" \
    --screenshot-every 30 --final-screenshot                # navigate + capture
```

| Item | Detail |
|---|---|
| `--route` | `;`-separated `hold:button+button` steps for P1; empty list = idle `hold` frames |
| Buttons | `a,b,start,z,l,r,dpad_u,dpad_d,dpad_l,dpad_r,c_u,c_d,c_l,c_r` |
| Stick | `x=N`, `y=N` tokens in a step: raw signed N64 units (-128..127), same units as `ssb_capture::script` pads; e.g. `30:b+x=-45+y=70` |
| Screenshots | `~/.var/app/net.sourceforge.m64py.M64Py/data/mupen64plus/screenshot/`; `--data-dir` overrides |
| Driver log | `[driver] ...` on stderr: plugin attach, screenshot paths; read to check a step landed |

### Per-frame fighter trace

- `tools/n64-headless/pitch_trace.py ROM OUT WORKDIR WINDOWS LAST`: P1 pad windows `start:len:btn+x=N+y=N`, one row per frame of `--scene` (default 54, Training)
- Row per player: status id, `lr`, `ga`, `vel_air` x/y, joint `--joint` (default 4) rotate and scale, f32 bits then decimals
- Golden tick = N64 row + 4 on the Training warp ROMs: the golden's A at tick 8 jabs on its first fighter frame; the N64 script presses A at 4 (RE-503)
- Warp ROMs (Training on Peach's Castle): `~/ppsspp-test/re473/warp/`, `~/ppsspp-test/re474/warp/`; spawn `lr` differs from Dream Land

### Audio dump

- `--audio-dump PATH`: swaps `mupen64plus-audio-sdl` for `n64_audio_dump.so`
- PATH: raw s16le stereo L,R, 32006.451 Hz, no resampling or volume, HLE audio; must be writable in sandbox
- Sidecar `PATH.txt`: DAC rate; one `buf` record per AI buffer (callback frame, sample offset, byte length)
- Core delivers each game buffer (552 or 368 samples) in two pieces; consecutive `dram` ranges join
- Mono/Stereo comes from the SRAM save
  - core always loads `data/mupen64plus/save/Super Smash Bros. (U) [!]-F7C52568.sra`
  - ignores `SaveSRAMPath` and `XDG_DATA_HOME`
  - save set to Mono → plain dumps have L == R
- `tools/n64-headless/record_audio_ref.py --force-stereo`:
  - writes fresh-cartridge default (Stereo) into RDRAM after frame 1
  - logs route steps, scenes, How to Play hits beside the dump

### RDRAM access and custom steps

- Import `Mupen64PlusHarness` from `tools/n64-headless/n64_driver.py`
- Methods: `read_rdram`, `read_u32`, `read_f32`, `read_be`, `write_rdram`, `write_u8`, `run_steps`
- RDRAM holds host-order (little-endian) 32-bit words: `read_rdram` bytes are word-swapped; byte `i` lives at `addr ^ 3`; `read_u32`/`read_f32` (aligned words) and `read_be` undo it
- Screenshots take a frame: never call one inside a per-frame trace loop
- Run: `flatpak run --command=python3 net.sourceforge.m64py.M64Py <script.py>`

## Route landmarks (RE-151, RE-216)

- `Start` on title/mode-select: hardcoded jump into 1P Mode, any cursor position
- `A` confirms an option
- VS match, both fighters idle: settles by frame ~1150-1800 (Mario/Pikachu/Dream Land)

## Gotchas

| Gotcha | Rule |
|---|---|
| Flatpak required | core and plugins only in `/app/lib` of `net.sourceforge.m64py.M64Py`; never bare `python3` |
| `DISPLAY`/Xvfb | does not hide window: `mupen64plus-video-rice.so` opens via SDL2 (prefers Wayland; socket always mounted); only `--env=SDL_VIDEODRIVER=offscreen` (script default) hides it; screenshots and RDRAM still work |
| `ctypes` `argtypes` | mandatory: Core API calls taking a dynlib handle or pointer need explicit `c_void_p`, else the 64-bit handle truncates and `PluginStartup` segfaults (RE-216; see `n64_driver.py` `start()`) |
| angrylion-plus | `mupen64plus-video-angrylion-plus.so` (Rosalie's Mupen GUI flatpak) segfaults in `PluginStartup`; abandoned, use Rice (RE-276) |
| Sandbox filesystem | needs read on repo, write on scratch dir |
| Software approximation | Rice is not a physical N64; treat RDP/RSP filtering differences as PPSSPP-vs-hardware |
| ROM identity | SHA-1 `e2929e10fccc0aa84e5776227e798abc07cedabf`, MD5 `F7C52568A31AADF26E14DC2B6416B2ED` |

Sandbox permissions:

```
flatpak info --show-permissions net.sourceforge.m64py.M64Py
flatpak override --user --filesystem=<path> net.sourceforge.m64py.M64Py
flatpak override --user --reset net.sourceforge.m64py.M64Py
```

Single-frame stall:

- Symptom: `ADVANCE_FRAME` exceeds the driver's 5 s per-frame timeout → `TimeoutError`
- Rate: ~1 in 5-8 runs on a 300+ frame idle hold; ~85-90% under host CPU contention (RE-276)
- Cause: unknown (Python thread jitter suspected); no flatpak process leaks
- Action: bare retry; investigate only if it reproduces on the same route every time
- Under load: prefer the decomp-rebuild warp below to a long `--route`

## Reaching a specific state

| Need | Method |
|---|---|
| Short known button sequence | `--route` |
| State expensive by menu | decomp rebuild warp (below) |

External writes to `gSCManagerSceneData.scene_curr` (`refs/ssb-decomp-re/src/sc/scmanager.c`, US `0x800a4ad0`) do not work:

- `scManagerRunLoop`'s `while(TRUE) { switch(scene_curr) }` re-reads it once per scene lifetime
- write reads back, stays inert until the scene exits and overwrites it (RE-276)

Decomp rebuild warp:

1. Patch `refs/ssb-decomp-re` temporarily; revert with `git checkout --`, never commit
2. Write the target state in `scManagerRunLoop`, right after `gSCManagerSceneData = dSCManagerDefaultSceneData;`
3. Rebuild: `PATH=~/ppsspp-test/mips-bin-shim:$PATH make -j$(nproc)`
4. Run patched `build/smashbrothers.us.z64` through the harness, not `rom/`

- Result: RE-276 scene-warp and camera-warp runs landed first try (menu navigation: 12-20+ retries)
- `SCCommonData` offsets (`src/sc/sctypes.h`): compute by hand (u8 packed, u16/u32 naturally aligned, no debug symbols)
- Worked example (RE-276):
  - `training_man_fkind`/`training_com_fkind` (fighter/CPU select)
  - Training "Close-Up" camera (`gmCameraSetStatusPlayerZoom`): edge-triggered by pause-menu input, ignores a static poke

### Toolchain setup (once per machine, Fedora/Nobara)

Decomp `installDependencies.sh` assumes Debian.

1. `sudo dnf install clang glibc-devel.i686 binutils-mips64-linux-gnu`
2. `git submodule update --init --recursive` (asm-processor, asm-differ)
3. `bash tools/ido-irix4/provision.sh`, run from the decomp (IRIX4 frontend for `ovl8_8.c`)
4. IDO 5.3 (`ido-recomp/5.3/cc`) missing → fetch as `installDependencies.sh` fetches 7.1
5. Makefile probes `mips-linux-gnu-*`; Fedora ships `mips64-linux-gnu-*`
   - symlink tools into a `$PATH` dir (e.g. `~/ppsspp-test/mips-bin-shim/`)
   - `mips-linux-gnu-ld` must be a wrapper: `exec mips64-linux-gnu-ld -m elf32btsmip "$@"`
   - why: else link fails "ABI is incompatible with that of the selected emulation"
6. Before any patch: `make init -j$(nproc)` clean; `build/smashbrothers.us.z64` SHA-1 must equal `baserom.us.z64` (`e2929e10fccc0aa84e5776227e798abc07cedabf`)

## Recording findings

- Harness run is not evidence until written down with route and frame numbers
- Record as `RE-XXX.md`: [reverse-engineering](../reverse-engineering/SKILL.md) step 6
