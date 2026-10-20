---
name: psp-hardware
description: Physical PSP, PSPLink, PSP-1000/2000/3000, hardware-only faults, memory constraints, cache coherency, or EBOOT/hardware validation work. Activates for real-hardware debugging distinct from PPSSPP emulation.
---

# PSP hardware

Rules: AGENTS.md § Constraints (PPSSPP is not hardware proof; no ROM data in Git).

- Scope: real-hardware validation and crash capture only
- "On device" claim = PPSSPP unless PSP model, firmware and build are cited
- PPSSPP goldens: [visual-regression](../visual-regression/SKILL.md)
- Original N64: [n64-emulator](../n64-emulator/SKILL.md)

Routing:

| Need | Owner |
|---|---|
| PSPLink install, debug PRX, exception mapping, shutdown, crash loop | this file |
| Memory layout, allocator, `MEMSIZE` (PSP-1000 32 MiB vs Slim/2000/3000) | [docs/memory.md](../../../docs/memory.md) |
| Open hardware work | [PLAN.md](../../../PLAN.md) "PSP performance and hardware" |
| Staging a capture build | [docs/visual-regression/README.md](../../../docs/visual-regression/README.md) "Physical PSP" |
| Prior hardware-only bugs | `docs/evidence/INDEX.md`, `rg` topic tag `hardware` |

Code: `psp-runtime/src/{gu,meshdraw}.rs`, `psp-asset-viewer/src/depth_diag.rs`.

## PSPLink hardware-crash debugging

- PSPLink: host shell on a PSP running custom firmware
- Verified: PSP Slim, 6.61, ARK/Infinity, PSPLink v3.2.1
- Needs: repository's PSP image, locally rebuilt pack

### Install PSPLink and host tools

1. Download the USB release from [pspdev/psplinkusb](https://github.com/pspdev/psplinkusb/releases)
2. Copy its `psplink` directory to `PSP/GAME/psplink/` on the Memory Stick
3. Exit USB mass-storage mode, launch PSPLink from the XMB
4. Connect USB (device becomes `054c:01c9`)

Host (needs libusb and readline dev packages):

```bash
git clone https://github.com/pspdev/psplinkusb.git
make -C psplinkusb/usbhostfs_pc
make -C psplinkusb/pspsh
sudo install -m 644 psplinkusb/usbhostfs_pc/50-psplink.rules /etc/udev/rules.d/50-psplink.rules
sudo udevadm control --reload-rules
pspsh -e ver     # connection check
```

No device → confirm PSPLink (not mass storage) is open, replug, check the udev rule.

### Serve checkout as `host0:`

Dedicated terminal at repository root; keep running while the game uses host files.

```bash
usbhostfs_pc -v "$PWD"     # maps $PWD to host0:/
pspsh -e modlist           # also: ver, usbstat
```

### Build and launch debug PRX

```bash
cargo psp --release --features regression_capture
```

- PRX loads `ssb64.pak` from beside itself
- `host0:` launch: symlink the pack after each pack rebuild (build output, Git-ignored)

```bash
ln -s ../../../../assets/generated/ssb64.pak \
  psp-asset-viewer/target/mipsel-sony-psp/release/ssb64.pak
pspsh -e 'ldstart host0:/psp-asset-viewer/target/mipsel-sony-psp/release/ssb64-psp-asset-viewer.prx'
```

Memory Stick runs: `tools/stage-psp-regression.sh` (no live host-file loading).

Reload procedure:

1. `modlist`
2. Kill the exact game UID if it remains
3. `reset` before every next `ldstart`, not only after a fault

```bash
pspsh -e modlist
pspsh -e 'kill 0x08800000'   # game module UID from modlist
pspsh -e reset
```

- why: bare `kill` can leave PSPLink's module manager stale, no visible symptom
  - `exlist` empty, `thlist` shows live `main_thread`, plausible frame
  - next module's relative `sceIoOpen("ssb64.pak")` silently fails → placeholder mesh renders
  - only comparing rendered content to the expected model catches it
- `kill` of a running `profile` build hung PSPLink until restarted on the PSP (RE-479)
- Scripted measurement runs: skip `kill`, bare `reset` (unloads the game), wrap every `pspsh` in `timeout`
- Never load a second game PRX before the first has stopped

### Capture and map exception

Collect state before reset.

```bash
pspsh -e exlist
pspsh -e exprint
pspsh -e thlist
pspsh -e 'thinfo <thread-uid>'
pspsh -e 'modinfo <game-module-uid>'
```

`modinfo` reports `TextAddr`; map exception `EPC` to a PRX offset.

```text
offset = EPC - TextAddr
```

```bash
psp-addr2line -e psp-asset-viewer/target/mipsel-sony-psp/release/ssb64-psp-asset-viewer.prx \
  -f -C -i 0x<offset>
psp-objdump -d --start-address=0x<nearby-offset> \
  --stop-address=0x<end-offset> \
  psp-asset-viewer/target/mipsel-sony-psp/release/ssb64-psp-asset-viewer.prx
```

Record in an RE record: exception code, EPC, TextAddr, module/thread IDs, symbol mapping, PRX hash, pack hash.

- FPU trap = correctness bug (invalid or speculative float code); map and fix
- Never disable trapping (example: RE-201)

### Shipped EBOOTs and symbols

- `psp-game` release profile has no debug info (4 MB EBOOT, not 31 MB; RE-475)
- Map an exception from a shipped EBOOT: build the same commit with `CARGO_PROFILE_RELEASE_DEBUG=true` (code unchanged), run `psp-addr2line` on that PRX

### Feature builds

| Feature | Effect |
|---|---|
| (XMB launch) | ~4 MB less user memory than under PSPLink (RE-475) |
| `memory_ballast` | reads `ballast.txt` (free bytes, decimal) beside the EBOOT, holds the rest at boot; PSPLink/PPSSPP runs see the XMB budget |
| `boot_log` | writes boot and scene loads to `ms0:/PSP/GAME/ssb64/boot.log`, no PSPLink needed |
| `profile_hw` | hardware counters per `profile` span (see below) |
| `capture_scene_file,frame_timer` | no profile spans; ticks power; prints `prof n= fps= frame= busy=` lines |

Start a PRX in its own directory:

```bash
printf 'cd ms0:/PSP/GAME/ssb64/\nldstart host0:/<prx>\n' | pspsh
```

- `capture_scene.txt`, `ballast.txt`, `probe` files, `profile.log`: read/written beside the PRX on `host0:`
- Pack absent there → loads from `ms0:/PSP/GAME/ssb64/` (RE-476)
- Capture builds never write the save
- Measure frame times on the PSP: PPSSPP opening CPU time ≈ 1/3 of PSP-2000 (RE-476)

Scene file lines:

- `stage=N` goes after the scene line
- `profile` and `frame_timer` builds also read:
  - `items=N`: VS item appearance 0..5
  - `level=N`: CPU level 1..9
  - `fighters=a,b,c,d`: fighter ordinals per port; first is also the 1P fighter
- release and `golden_capture` builds ignore them (RE-531)

### Profiling counters (`profile_hw`)

- Counters: cycles, memory/internal/FPU stalls, I/D-cache misses, uncached stores
- Source: ARK `KUBridge` (`kuKernelPeekw`); never `kuKernelCall` (flushes caches per call)
- Needs ARK; does not run under PPSSPP
- `profile_hw.txt` beside the PRX containing `frame`: one read per frame
- Unattended PSP auto-sleeps: `profile` builds call `scePowerTick` every frame, others do not (RE-481)

### I-cache layout search

- Counters give miss counts, not locations
- Per-function I-cache misses:
  1. cache model in a local PPSSPP build (`tools/icache/ppsspp-icache.patch`, run with `--ir`; 86% of PSP-2000 count in VS)
  2. `tools/icache/*.py`
  3. confirm with `profile_hw` (RE-482)
- `psp-game` links via `tools/psp-link.py`: functions in `psp-game/link-order.txt` placed first
- An order fits only the build it was searched on
  - hot code changes size → search again on the shipped configuration
  - `tools/icache/layout.c`: replays a model trace per candidate
- `tools/icache/relink.py`: relinks a saved build with a new order in seconds, for hardware A/B (procedure: RE-494)
- Model ranks orders weakly (RE-500) → pick on the PSP
- `tools/hw-link-order.sh` (`prep`/`run`/`rank`, `DRY_RUN=1`):
  - relinks candidates, runs each via PSPLink
  - ranks with `tools/icache/hwrank.py` by BFPS (1e6/busy); why: release code is vsync-bound
- `tools/icache/cmpaddr.py --elf`: checks a measurement build keeps searched functions at release addresses
- `tools/icache/summ.sh`: summarises runs

### Native frame evidence

```bash
pspsh -e 'scrshot host0:/psp-hw-capture.bmp'
sha256sum psp-hw-capture.bmp
```

- Native 480x272 BMP
- Record: capture hash, PSP model, firmware/CFW, PSPLink version, commit, PRX and pack hashes, scene, runtime window, `exlist` result
- PSPLink may overlay diagnostics; exclude from pixel comparison
- Captures stay out of Git

### Shutdown

```bash
pspsh -e close
```

1. Ctrl-C `usbhostfs_pc`
2. Return PSP to USB mass-storage mode only after the PSPLink session closes

### Fast crash loop

```text
build PRX + current pack
  -> start usbhostfs_pc in repository root
  -> ldstart host0:/... PRX
  -> reproduce once
  -> exlist/exprint + modinfo + thread state
  -> EPC - TextAddr -> psp-addr2line
  -> smallest fix + targeted test
  -> rebuild, reset/reload, stability window
  -> native capture + hardware evidence
```
