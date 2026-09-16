# Scene 1 — Dream Land default (deterministic test scene)

Part of [docs/visual-regression/README.md](../README.md).

## The deterministic test scene

**Stage:** Dream Land, `stage_index` 0 (confirmed against the decomp
repeatedly throughout `docs/reverse-engineering.md`, e.g. RE-031/RE-034:
"stage 0/41 file 255 @0x14").

**Fighter:** Mario, placed at the stage's own spawn 0
(`psp/src/play.rs::Play::at_spawn` hardcodes `FighterKind::Mario` and
`pack.spawn(stage, 0)` — no debug-viewer state affects this choice).

**Camera:** the debug viewer's own boot defaults — `cam_distance =
CAM_FIT`, `spin = 0.0`, no button input. Dream Land's stage-view camera is
"face-on, always" or `sim_fighter`-follow only past a zoom threshold this
scene never crosses at default zoom (`psp/src/main.rs`, stage-view camera
comment), so no additional pinning was needed.

**Animation/frame:** pinned by a new `regression_capture` Cargo feature on
the `ssb64-psp` crate (`psp/Cargo.toml`), off by default. When enabled, once
240 simulation ticks have run (`DETERMINISTIC_CAPTURE_TICKS`, 4 real seconds at
the sim's fixed 60 Hz — comfortably past Mario's fall from Dream Land's
spawn height), every per-frame mutation freezes: the fighter physics tick
(`Play::tick`), the object/animation-viewer skeleton tick, the stage
scenery animator (`StageAnimator::tick`), and the material animator
(`MaterialAnimator::tick`). Nothing in the simulation is randomised, so a
frozen state never changes again regardless of how long the capture script
keeps the emulator running afterward — a screenshot at tick 240 and one at
tick 900 are the same PNG (measured, see Evidence below).

**Game state:** none of the interactive toggles are touched (`show_collision`
stays at its default `true`, `sim_fighter` stays `true`, no button is
pressed) — the debug viewer boots directly into this state with a pack
loaded and no input.

**The on-screen debug HUD is never drawn at all under `regression_capture`,
from frame 0.** `gpu.debug_text` normally prints live perf counters
(`cpu`, `frame`, `tick`) that are meaningless once the sim is frozen and
would otherwise be the only non-deterministic content left in the frame.
Three narrower fixes were tried before this one and each failed for a
different reason specific to `sceGuDebugPrint` (a PPSSPP-only debug
overlay hook, not real GE drawing, that does not fully clear between
calls): skipping the call once frozen left a stuck, partial-width redraw
behind; pinning the three fields to `0` once frozen left old, wider digits
ghosted behind a shorter string; pinning them to their real last-seen
values fixed the width but not the content, since `cpu`/`frame` are
genuine wall-clock timing measurements that legitimately differ between
runs; and even a hardcoded, safely-wide sentinel value still ghosted,
meaning the corruption was never simply about string width. Never calling
`sceGuDebugPrint` in a `regression_capture` build sidesteps whatever
PPSSPP-internal state causes this rather than trying to out-guess it — and
a developer diagnostic overlay was never actually part of the golden scene
this task wants captured. See `docs/reverse-engineering.md` RE-123 and
RE-125 for the full account.
