---
name: psp-hardware
description: Physical PSP, PSPLink, PSP-1000/2000/3000, hardware-only faults, memory constraints, cache coherency, or EBOOT/hardware validation work. Activates for real-hardware debugging distinct from PPSSPP emulation.
---

# PSP hardware

**PPSSPP is not physical PSP proof.** Treat every "on device" claim
elsewhere in the docs as PPSSPP unless a hardware model, firmware and build
are explicitly cited. For deterministic visual/rendering testing, use the
[visual-regression](../visual-regression/SKILL.md) Skill (PPSSPPHeadless)
instead; for live original-N64 behavior use
[n64-emulator](../n64-emulator/SKILL.md) (headless Mupen64Plus) instead —
this skill is for real PSP hardware validation and crash capture only.

Routing:

1. This file (below) — PSPLink install, build/launch debug PRX, capture and
   map exceptions, shutdown, fast crash-loop procedure.
2. [docs/memory.md](../../docs/memory.md) — PSP memory layout, allocator
   plan, `MEMSIZE`/RAM constraints (PSP-1000's 32 MiB vs Slim/2000/3000).
3. [plans/rendering/R2.md](../../plans/rendering/R2.md) — the physical
   hardware validation task: current matrix, what's confirmed, what remains
   (PSP-1000 coverage, exhaustive/long-duration runs).
4. `docs/evidence/INDEX.md` filtered to the `hardware` topic tag for prior
   hardware-only bugs found (GE races, PSPLink module-manager state, etc.).
5. For code, prefer Serena symbol search over reading whole files
   (`psp-asset-viewer/src/gu.rs`, `psp-asset-viewer/src/depth_diag.rs`,
   PSPLink-related PRX code).

After a `kill` in a PSPLink session, `reset` before the next `ldstart` — a
stale module-manager state can silently break the next pack load while
`exlist`/`thlist` still look clean.

## PSPLink hardware-crash debugging

PSPLink gives a host shell direct access to a PSP running custom firmware. Use
it for physical-hardware validation and crash capture; PPSSPP cannot prove PSP
behavior.

This workflow was verified with PSP Slim, 6.61, ARK/Infinity, and PSPLink
v3.2.1. It assumes this repository has built its PSP image and locally rebuilt
ROM-derived asset pack. Never commit ROM data, generated pack, or captures.

### Install PSPLink and host tools

Download PSPLink USB release from
[pspdev/psplinkusb](https://github.com/pspdev/psplinkusb/releases). Copy its
`psplink` directory to Memory Stick:

```text
PSP/GAME/psplink/
```

Exit PSP USB mass-storage mode. Launch **PSPLink** from XMB, then connect its
USB session. Device changes from mass storage to PSPLink (`054c:01c9`).

Build matching host programs. Host needs libusb and readline development
packages:

```bash
git clone https://github.com/pspdev/psplinkusb.git
make -C psplinkusb/usbhostfs_pc
make -C psplinkusb/pspsh
```

Install supplied udev rule, then reconnect cable or restart udev:

```bash
sudo install -m 644 psplinkusb/usbhostfs_pc/50-psplink.rules \
  /etc/udev/rules.d/50-psplink.rules
sudo udevadm control --reload-rules
```

Check connection:

```bash
pspsh -e ver
```

If no device appears, confirm PSPLink—not mass storage—is open on PSP,
reconnect cable, and check udev rule.

### Serve checkout as `host0:`

Run in dedicated terminal from repository root:

```bash
usbhostfs_pc -v "$PWD"
```

It maps current host directory to `host0:/`. Keep process running while game
uses host files.

```bash
pspsh -e ver
pspsh -e modlist
pspsh -e usbstat
```

### Build and launch debug PRX

```bash
cargo psp --release --features regression_capture
```

PRX searches beside itself for `ssb64.pak`. For `host0:` launch, create ignored
build-output symlink after pack rebuild:

```bash
ln -s ../../../../assets/generated/ssb64.pak \
  psp-asset-viewer/target/mipsel-sony-psp/release/ssb64.pak
```

Load game:

```bash
pspsh -e 'ldstart host0:/psp-asset-viewer/target/mipsel-sony-psp/release/ssb64-psp-asset-viewer.prx'
```

For Memory Stick runs use `tools/stage-psp-regression.sh`; it does not provide
live host-file loading.

Before reload inspect `modlist`. Kill exact game UID if it remains, then
`reset` before the next `ldstart` — not only after an observed fault.
RE-212 found a case with no visible symptom at all (`exlist` empty,
`thlist` showed a live `main_thread`, a plausible-looking rendered frame)
where a bare `kill` still left PSPLink's own module-manager state stale
enough that the next module's relative `sceIoOpen("ssb64.pak")` silently
failed and fell back to the built-in placeholder mesh. `exlist`/`thlist`
did not catch it; only comparing the actual rendered content against the
expected model did. Treat `reset` as the default step after any `kill`,
not a conditional one:

```bash
pspsh -e modlist
pspsh -e 'kill 0x08800000' # Replace with game module UID from modlist.
pspsh -e reset
```

Never load second game PRX before prior instance has stopped.

### Capture and map exception

Collect state before reset:

```bash
pspsh -e exlist
pspsh -e exprint
pspsh -e thlist
pspsh -e 'thinfo <thread-uid>'
pspsh -e 'modinfo <game-module-uid>'
```

`modinfo` reports `TextAddr`. Map exception `EPC` to PRX offset:

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

Record exception code, EPC, TextAddr, module/thread IDs, symbol mapping, PRX
hash, and pack hash in `docs/reverse-engineering.md`. PSPLink FPU traps may
expose invalid or speculative floating-point code. Treat trap as correctness
bug; map and fix cause, never disable trapping. `RE-201` is working example.

### Native frame evidence

```bash
pspsh -e 'scrshot host0:/psp-hw-capture.bmp'
sha256sum psp-hw-capture.bmp
```

This yields native 480x272 BMP. Record capture hash, PSP model,
firmware/CFW, PSPLink version, commit, PRX/pack hashes, scene, runtime window,
and `exlist` result. PSPLink may add diagnostic overlay; exclude it from pixel
comparison. Keep capture out of Git unless repository policy permits it.

### Shutdown

Close shell, then stop USBHostFS after game no longer needs `host0:`:

```bash
pspsh -e close
```

Stop `usbhostfs_pc` with Ctrl-C. Return PSP to USB mass-storage mode only after
PSPLink session closes.

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
