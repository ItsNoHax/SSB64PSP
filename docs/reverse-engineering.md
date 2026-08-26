# Reverse-Engineering Log

Per Rule 10: when the original's behaviour is uncertain, record the
uncertainty rather than guessing. Each entry is Question / Evidence /
Hypothesis / Implementation / Confidence.

Because the decompilation is **100% complete**, most questions here are about
*porting* decisions rather than about what the original does. Anything
answerable from the decomp should be answered from the decomp, not guessed.

---

## RE-001 — relocData table geometry

**Question.** Where is the asset archive, how many files, and how are entries
laid out?

**Evidence.**
- `symbols/linker_constants.txt`: `lLBRelocTableFilesNum = 0x000854` (2132),
  `lLBRelocTableAddr = relocData_ROM_START`.
- `smashbrothers.us.yaml:1870`: `- [0x1AC870, bin, relocData]`.
- `lbRelocInitSetup`: `rom_table_hi = table_addr + ((files_num + 1) * sizeof(LBTableEntry))`.
- `struct LBTableEntry` in `src/lb/lbtypes.h` — 12 bytes, sizes in *words*.

**Hypothesis.** Table at `0x1AC870`, 2133 entries (2132 + sentinel), data base
at `0x1B2C6C`.

**Implementation.** `crates/ssb-rom/src/archive.rs`.

**Verification.** Parsed against the real ROM: offsets are monotonic across all
2133 entries, all 499 `is_compressed` entries carry `vpk0` magic at their
computed address, 0 do not, and the sentinel's offset lands exactly at the end
of the archive region.

**Confidence: certain.**

---

## RE-002 — VPK0 stream format

**Question.** How is the compressed payload encoded?

**Evidence.** `syDmaDecodeVpk0`, `src/sys/dma.c:160-388`. Read in full.

**Hypothesis.** `vpk0` magic, 32-bit decompressed length, 8-bit sample method,
then two postfix-encoded Huffman trees (offsets, lengths), then an LZ stream
where a `0` bit is a literal byte and a `1` bit is a back-reference. Huffman
leaves hold *bit widths*, not values.

**Implementation.** `crates/ssb-rom/src/vpk0.rs`.

**Verification.** This one deserves spelling out, because "it didn't crash" is
not evidence of a correct decompressor.

The number of extern relocations in a file can be derived two *independent*
ways:

1. By walking the linked chain embedded in the **decompressed payload** —
   which requires every byte to be correct, since the chain is threaded
   through the data itself and a single wrong byte derails it.
2. By measuring the ROM gap between the end of this file's data and the start
   of the next file's, which is exactly the `u16` target-ID array
   (`lbRelocGetExternBytesNum` bounds its scan this way). This does not depend
   on decompression at all.

`romtool check` compares them for every file:

```
files                 2132
load failures         0
intern reloc slots    61343
extern reloc slots    3092
chain/ROM mismatches  0
compressed files cross-verified against ROM geometry: 499
```

All 499 compressed files agree. Additionally, every VPK0 stream's
self-declared decompressed length matches the table's `decompressed_size`
independently.

**Confidence: certain.**

---

## RE-003 — Microcode variant

**Question.** F3D, F3DEX, or F3DEX2? This changes every display list opcode.

**Evidence.**
- `src/sys/taskman.c:61`: `NewUcodeInfo(gspF3DEX2_fifo)`.
- `symbols/linker_constants.txt:54`: `gspF3DEX2_fifoTextStart = 0x8003A320`,
  commented "F3DEX2 fifo 2.04H".

**Implementation.** `crates/ssb-rom/src/dl.rs` uses F3DEX2 opcode numbering
(`G_VTX = 0x01`, `G_TRI1 = 0x05`, `G_MTX = 0xDA`) taken from the decomp's own
`include/PR/gbi.h` rather than from memory.

**Confidence: certain.**

---

## RE-004 — Coordinate system and matrix conversion

**Question.** Does converting an N64 matrix to a PSP matrix require a
transpose?

**Evidence.** N64 `Mtx` is row-major and libultra uses the row-vector
convention (`v' = v·M`, translation at `m[3][0..2]`). PSP `sceGuSetMatrix`
takes column-major with the column-vector convention (`v' = M·v`, translation
in the last column).

**Hypothesis.** Two transposes that cancel:
`result[i] = Σⱼ v[j]·M64[j][i]` and `result[i] = Σⱼ Mpsp[i][j]·v[j]` give
`Mpsp[i][j] = M64[j][i]`; column-major storage means `cols[j][i] = Mpsp[i][j]`,
hence `cols[j][i] = M64[j][i]` — **identical linear element order**. The only
real work is widening s15.16 fixed point to `f32`.

**Implementation.** `crates/ssb-engine/src/coord.rs::n64_to_psp_matrix`.

**Note.** The first implementation *did* transpose, and the unit test
`row_vector_translation_lands_in_the_translation_column` caught it — the
translation ended up in the bottom row instead of the translation column.
Worth keeping as a cautionary tale: this is exactly the kind of error that
produces a subtly broken renderer rather than an obviously broken one.

**Confidence: high.** Verified by unit test against the algebra; not yet
verified against on-hardware output, which requires real geometry (M3).

---

## RE-005 — Handedness

**Question.** Do world-space positions need a flip?

**Evidence.** N64 `guPerspective`/`guLookAt` and PSP
`sceGumPerspective`/`sceGumLookAt` both produce right-handed view space
looking down `-Z` with `+Y` up. `ftPhysicsApplyGravityClampTVel` does
`vel_air.y -= gravity`, confirming `+Y` up in world space.

**Hypothesis.** No flip needed.

**Implementation.** `n64_to_psp_position` is an identity function, kept as a
named function so the renderer never hardcodes the assumption and any future
correction lands in one place.

**Confidence: high.** Needs on-hardware confirmation with real geometry —
specifically, that characters face the direction they should. Flag for M3.

---

## RE-006 — Simulation rate

**Question.** Is the simulation a fixed 60 Hz?

**Evidence.** Every timing constant in the decomp is expressed in frames
(`kneebend_anim_length`, `attack1_followup_frames`, hitlag/hitstun counters,
`FTINPUT_STICKBUFFER_TICS_MAX`). `scheduler.c:1249` registers
`osViSetEvent(..., INTR_VRETRACE, 1)` — an event every single retrace, i.e.
60 Hz NTSC.

**Hypothesis.** Fixed 60 Hz simulation.

**Implementation.** `crates/ssb-engine/src/timing.rs`. Simulation is decoupled
from rendering with a fixed-step accumulator; the PSP display runs at
~59.94 Hz so the steady state is one tick per vblank, with capped catch-up for
stalls.

**Confidence: high.**

---

## RE-007 — Physics zero-crossing asymmetry

**Question.** Ground friction clamps with `> 0.0` / `< 0.0`, air friction with
`>= 0.0` / `<= 0.0`. Deliberate, or decompilation noise?

**Evidence.** `ftPhysicsSetGroundVelFriction` @ 0x800D8978 uses strict
comparisons; `ftPhysicsApplyAirVelXFriction` @ 0x800D9034 uses non-strict. The
decomp is a byte-matching build, so both reflect the original instructions
exactly — this is not a transcription artifact.

**Hypothesis.** An original inconsistency, probably unintentional, but
*observable*: an air speed landing exactly on the friction value stops, where
the ground equivalent leaves it at exactly zero by a different path.

**Implementation.** Preserved verbatim in `crates/ssb-game/src/physics.rs`,
with a comment warning against "tidying" it into a shared helper.

**Confidence: certain** that it is in the original; **low** on whether it ever
changes observable gameplay. Preserved regardless — matching behaviour is
cheaper than proving it does not matter.

---

## RE-008 — C-button mapping *(OPEN)*

**Question.** Which C-button functions matter in Smash 64, and what should
they map to on a PSP with no C-stick?

**Evidence so far.** Unlike Melee, Smash 64's C-buttons are not attack inputs.
They appear to be used for taunt and for camera control in some single-player
modes. **Not yet confirmed against the decomp's input handling** (`ft/ftkey.c`,
`sys/controller.c`).

**Current implementation.** C-Up → Triangle, C-Down → Square, per
`DEFAULT_MAPPING` in `crates/ssb-engine/src/input.rs`. C-Left and C-Right are
currently **unmapped**.

**Confidence: low.** This is a placeholder. Resolve by reading `ftkey.c` and
the menu input paths before M4. If C-Left/C-Right turn out to matter, the PSP
has no free buttons and a modifier scheme will be needed.

---

## RE-009 — PSP nub deadzone *(OPEN)*

**Question.** How large should the analog deadzone be, and does the N64's
`-80..=80` range map linearly?

**Evidence.** Smash reads raw stick magnitudes for thresholds (tilt vs. smash
attacks, smash-turn detection, fast-fall via
`FTCOMMON_FASTFALL_STICK_RANGE_MIN`), so the *scale* matters, not just the
direction. The PSP nub reports 0..255 and drifts noticeably.

**Current implementation.** Deadzone of 20 nub units, then a linear rescale so
full deflection still reaches ±80. Tested for monotonicity and for reaching
both extremes.

**Confidence: low.** The deadzone value is a guess and the mapping is linear
where the N64's stick response may not be. Needs measurement against a real
PSP nub and comparison of resulting stick ranges against the decomp's
thresholds. Flag for M4.

---

## RE-010 — `MObjSub` unknown fields *(OPEN)*

**Question.** `MObjSub` has ~15 fields still named `unkNN` (`unk08`, `unk0A`,
`unk10`, `unk24`, `unk28`, `unk36`…`unk74`). Do any affect rendering?

**Evidence.** The named fields cover what a material needs: format/size,
sprite and palette pointers, UV translate/scale/scroll, prim/env/blend colours,
two light colours, and a flags word. The unknowns are interleaved with these.

**Hypothesis.** Mostly padding or animation scratch, given the struct is also
written by the material-animation system.

**Implementation.** Not yet consumed. The converter reads only the named
fields.

**Confidence: low.** Revisit if converted materials look wrong in M3. The
decomp can answer this definitively by finding the readers — do that rather
than experimenting.

---

## RE-011 — Level of detail selection *(OPEN)*

**Question.** `DObjDistDL` picks a display list by camera distance and
`sGCDetailLevel` picks a global tier. How is the tier chosen, and should the
PSP force one?

**Evidence.** `objdisplay.c:1776`: `gSPDisplayList(..., dls[sGCDetailLevel])`.
The variable is set elsewhere; the setter has not been traced yet.

**Hypothesis.** Likely tied to player count or an options setting, both of
which affect N64 fill rate.

**Implementation.** None yet.

**Confidence: low.** Worth resolving before M8 — forcing a lower tier is one of
the cheapest performance levers available, but only if it does not change
gameplay-visible geometry (e.g. platform collision derived from the same data).

---

## RE-012 — Nightly toolchain pin

**Question.** Why is the PSP crate pinned to a specific nightly?

**Evidence.** `rust-psp` 0.3.13 imports `core::panic::PanicPayload` in its
panic handler (`psp/src/panic.rs:15`). That path no longer resolves on
`nightly-2026-08-26`. Upstream `rust-psp` master has the same import, so there
is no newer release to move to.

**Implementation.** `psp/rust-toolchain.toml` pins `nightly-2026-08-01`, which
was verified to still export it.

**Confidence: certain.** Documented in the toolchain file itself, with a note
that a successful compile is not sufficient evidence for a bump — the result
must boot.

---

## RE-013 — `psp::dprintln!` is a 30x performance trap

**Question.** Why did a 4-triangle scene run at 2 FPS under PPSSPP?

**Evidence.** PPSSPP's debug log showed `sceDisplaySetMode(0, 480, 272)` being
issued *every frame*, which our code never calls. Tracing it to rust-psp's
debug-print path: `psp::dprintln!` writes into the framebuffer directly and
re-establishes display mode on each call. We were making eight such calls per
frame.

**Measurement.** Removing the per-frame `dprintln!` calls took the frame rate
from **2.0 FPS to a locked 60.0 FPS**, and shrank the EBOOT from 9.6 MB to
3.3 MB. Emulator debug logging (`-d`) was ruled out as the cause by measuring
both with and without it.

**Implementation.** `Gpu::debug_text` in `psp/src/gu.rs`, with the constraint
documented at the call site.

**Confidence: certain.** Directly measured, before and after.

**Lesson worth keeping:** `dprintln!` is fine for one-shot boot diagnostics and
must never appear in a frame loop.

---

## RE-014 — GU debug text is invisible under PPSSPP's hardware backends

**Question.** Why does `sceGuDebugPrint` + `sceGuDebugFlush` render nothing?

**Evidence.** Reading rust-psp's implementation
(`psp/src/sys/gu.rs:3523-3661`):

* `sceGuDebugPrint` copies characters into a static buffer, so passing a
  short-lived stack string is safe.
* It has a bug: `char_struct_ptr` always starts at the beginning of
  `CHAR_BUFFER` while `CHAR_BUFFER_USED` keeps accumulating, so successive
  calls in one frame overwrite each other. Worked around by emitting a single
  newline-separated string.
* `sceGuDebugFlush` does **not** queue a GE command. It computes pixel
  addresses and writes glyphs straight into VRAM with the CPU.

That last point explains the first failure mode — flushing before
`sceGuSync` meant the still-queued `sceGuClear` erased the text. Moving the
flush to after the sync (in `Gpu::end_frame`) fixed the ordering, but the text
is *still* invisible.

**Hypothesis.** PPSSPP's *hardware* backends (OpenGL, Vulkan) render into a
GPU-side framebuffer and do not reflect CPU writes to emulated VRAM. Its
*software* rasteriser emulates VRAM directly and should show them.

**Verification.** Confirmed. Forcing `SoftwareRenderer = True` (via
`--appendconfig`, so the user's own config is untouched) renders the overlay
perfectly — see `docs/images/m1-ppsspp-diagnostics.png`. The identical binary
shows no text under OpenGL.

Note an earlier `--graphics=software` attempt appeared to fail; that flag did
not take effect, and the config-file route is the reliable one. Do not trust a
negative result from the command-line flag alone.

**Conclusion.** The code is correct. This is an emulator-backend limitation,
not a port bug.

**Implementation.** Kept as-is. `tools/run-ppsspp.sh` passes the software-render
config so diagnostics are always visible during development.

**Confidence: certain.**

**Caveat for later:** relying on CPU framebuffer writes still means the overlay
is invisible under the fast backends. The real HUD must render as GE geometry
(Renderer 3), at which point this mechanism should be retired for anything a
developer needs to watch at full speed.

---

## RE-015 — Unexplained horizontal drift *(RESOLVED — earlier hypothesis was wrong)*

**Question.** In one run the test object drifted steadily left with no input.
Why?

**Original hypothesis (WRONG).** That PPSSPP reports the analog nub as 0
rather than centred 128 when no gamepad is attached, which `nub_axis_to_n64`
would legitimately map to −80 (full left) and feed to `apply_air_drift`.

**Evidence that refutes it.** Once the on-screen diagnostics became visible
(RE-014), the same build under the same conditions reports:

```
pos  x0 y-300 z0   (x100)
vel  x0 y0         (x100)
stick 0  buttons 0000
```

`stick 0` is dead centre and horizontal velocity is exactly zero. The nub is
being read correctly and the deadzone is doing its job.

**Actual cause: unknown.** The drift was most likely stray input — PPSSPP's
default keyboard mapping with the window focused, or its on-screen touch
controls — rather than anything in the input path.

**Lesson.** The original entry reasoned from a *plausible* mechanism to a
confident conclusion without measuring the value. The instrumentation existed
but was not visible, and the hypothesis was written anyway. Measure the
variable before explaining it.

**Confidence: certain** that the nub reads centred; **low** on what caused the
one-off drift, and it is not worth chasing further unless it recurs.

---

## RE-016 — Measured frame budget at M1

**Question.** How much CPU headroom does the M1 baseline actually have?

**Evidence.** On-screen diagnostics, PPSSPP software rasteriser, steady state:

```
frame 701  tick 701
ticks/frame 1  dropped 0
cpu 13us / budget 16667us
frame 16682us  view 362x272
```

**Readings.**

* `frame == tick` — exactly one simulation tick per displayed frame. The fixed
  60 Hz clock (RE-006) holds in lockstep with no accumulated error over 700
  frames, and no catch-up ticks or drops.
* `frame 16682us` — 59.94 Hz, the expected PSP vblank cadence.
* `cpu 13us` against a 16667us budget — **0.08%** consumed by simulation plus
  render submission.
* `view 362x272` — `coord::pillarboxed_viewport()` confirmed on-device.

**Caveat.** The scene is four triangles and one fighter's worth of physics, so
13us says nothing about how a real match will perform. Its value is as a
**baseline**: the platform layer, clock and submission path cost essentially
nothing, so future frame time can be attributed to the game rather than to
scaffolding.

**Confidence: high** for the measurement; explicitly **not** a performance
prediction. Real PSP hardware is ~333 MHz against an emulator on a desktop CPU
— these numbers do not transfer (plan §37).
