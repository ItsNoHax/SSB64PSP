---
name: psp-hardware
description: Physical PSP, PSPLink, PSP-1000/2000/3000, hardware-only faults, memory constraints, cache coherency, or EBOOT/hardware validation work. Activates for real-hardware debugging distinct from PPSSPP emulation.
---

# PSP hardware

**PPSSPP is not physical PSP proof.** Treat every "on device" claim
elsewhere in the docs as PPSSPP unless a hardware model, firmware and build
are explicitly cited.

Routing:

1. [docs/psplink.md](../../docs/psplink.md) — install, build/launch debug
   PRX, capture and map exceptions, shutdown, fast crash-loop procedure.
2. [docs/memory.md](../../docs/memory.md) — PSP memory layout, allocator
   plan, `MEMSIZE`/RAM constraints (PSP-1000's 32 MiB vs Slim/2000/3000).
3. [plans/rendering/R2.md](../../plans/rendering/R2.md) — the physical
   hardware validation task: current matrix, what's confirmed, what remains
   (PSP-1000 coverage, exhaustive/long-duration runs).
4. `docs/evidence/INDEX.md` filtered to the `hardware` topic tag for prior
   hardware-only bugs found (GE races, PSPLink module-manager state, etc.).
5. For code, prefer Serena symbol search over reading whole files
   (`psp/src/gu.rs`, `psp/src/depth_diag.rs`, PSPLink-related PRX code).

After a `kill` in a PSPLink session, `reset` before the next `ldstart` — a
stale module-manager state can silently break the next pack load while
`exlist`/`thlist` still look clean.
