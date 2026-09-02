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

## RE-017 — `G_VTX` destination index encoding

**Question.** How does F3DEX2 encode the destination slot of a vertex load?

**Evidence.** `gbi.h`:

```c
#define gSPVertex(pkt, v, n, v0) \
    gDma0p(pkt, G_VTX, v, ((n) << 12) | (((v0) + (n)) * 2))
```

The low byte holds `(v0 + n) * 2` — the **end** of the destination range, not
its start. Therefore `v0 = (w0 & 0xFF) / 2 - n`.

**The bug this caused.** The first implementation computed
`dest_index = ((w0 >> 1) & 0x7F) / 2`, which evaluates to `(v0 + n) / 2`. That
is correct **only when `v0 == n`** — and the unit test happened to use
`v0 = n = 8`, so it passed.

Consequence: nearly every real display list decoded with vertices loaded into
the wrong cache slots, so triangles indexed slots that were never filled.
`romtool mesh` reported **666 of 762 lists failing** with `EmptyCacheSlot`,
which was misread as "these are continuation lists" and sent the design down a
blind alley (root-detection, call-inlining as a *fix* rather than a feature).

**How it was actually found.** By decoding a display list the decomp names
explicitly — file 105 at offset `0xCDA0`, documented in `relocData.md` — and
reading the commands:

```
+0A8 01 VTX   w0=01004008 w1=0000CD20     n=4, v0=0  (not v0=2)
+0B0 06 TRI2  w0=06060402 w1=00020006     slots 3,2,1 and 1,0,3
```

The triangle references slots 0 and 1, which the broken decode never filled.

**After the fix:** 1,768 root lists convert with **zero failures**, yielding
25,562 triangles.

**Confidence: certain.** Verified against known-good data, with a regression
test using `v0 != n`.

**Lesson.** A unit test with `v0 == n` cannot distinguish the correct formula
from a wrong one. When testing an encoding, choose values where the candidate
formulas *disagree*. And when a decoder reports mass failures, suspect the
decoder before inventing a theory that explains the failures away.

---

## RE-018 — Segmented addresses survive relocation

**Question.** `G_DL` targets in relocated files sometimes hold values like
`0x0E000000`, far past the end of the file. Corrupt?

**Evidence.** File 105 at `0xCDA0+0x80`: `DE000000 0E000000`. Segment `0x0E` is
the graphics heap — `objdisplay.c` does
`gSPSegment(dl_head[0]++, 0xE, gSYTaskmanGraphicsHeap.ptr)`.

**Conclusion.** Not all display-list pointers are relocated file offsets. Some
are **segmented addresses** the RSP resolves at draw time against a segment
table the game sets per frame. They point at runtime-generated lists that do
not exist in the ROM at all.

**Implementation.** `scan::is_plausible` bounds-checks only segment-0
addresses, and `mesh::convert` skips non-zero segments when inlining. Treating
them as file offsets previously rejected valid display lists outright.

**Confidence: certain.**

---

## RE-019 — `G_SETTIMG` format describes the load, not the render

**Question.** Why did 294 texture conversions fail with the impossible
combination `(Ci, Bits16)`? CI is only ever 4- or 8-bit.

**Evidence.** From the real display list in file 105 at `0xCDA0`:

```
F5 SETTILE  w0=F5400400 w1=00098250   tile 0: fmt=CI, siz=4b   <- render format
F5 SETTILE  w0=F5500000 w1=07018050   tile 7: fmt=CI, siz=16b  <- TLUT staging
F5 SETTILE  w0=F5000100 w1=05000000   tile 5:                  <- TLUT staging
FD SETTIMG  w0=FD100000 w1=0000C030   fmt=RGBA, siz=16b        <- the *load*
```

Two independent mistakes were feeding each other:

1. **Every `G_SETTILE` was being applied**, not just tile 0. Tiles 5 and 7 stage
   TLUT loads and carry descriptors that make no sense as render formats.
2. **`G_SETTIMG`'s own format/size were being trusted.** They describe how the
   RDP *reads* the image during `G_LOADBLOCK`, not how it samples it. A CI4
   texture is routinely loaded as RGBA16 because that is the efficient DMA
   width.

**Conclusion.** The address comes from `G_SETTIMG`; the render format comes
from `G_SETTILE` on **tile 0** (`G_TX_RENDERTILE`). They are separate pieces of
state and must be tracked separately.

**Result.** Texture conversion went from **41 of 469** to **336 of 469**, and
the format distribution flipped to match the measured inventory — `PsmT4`
became dominant (274 textures) instead of everything falling back to
`Psm8888`.

**Confidence: certain.** Verified against a decomp-named display list, with
regression tests covering both halves (`tile 0 wins over SETTIMG`, and
`non_render_tiles_are_ignored`).

**Lesson, and it is the same one as RE-017:** the failure counts were pointing
at the bug the whole time. 294 identical `UnsupportedCombination(Ci, Bits16)`
errors is not noise, it is a decoder reporting one specific wrong assumption.
Reading the failure histogram before theorising would have found this faster.

---

## RE-020 — PSP integer vertex formats are normalised, not raw

**Question.** Geometry converted correctly, the draw was issued (`draws 1`),
culling was off, the bounding box was sane and the camera was inside the far
plane — yet nothing appeared on screen.

**Evidence.** On-screen instrumentation, added rather than guessed at:

```
mesh 0/1768  file 22  @0x4D0
tris 2  verts 4  prims 1
draws 1  state changes 2
bb 0 -253 -253 .. 253 1024 1024
cam 2939 r 1277
```

Everything was consistent with geometry that should be visible.

**Cause.** `GU_VERTEX_16BIT` does not pass integer coordinates through — the GE
interprets them as **normalised** fixed point, dividing by 32768. N64
coordinates in the hundreds therefore became hundredths of a unit, collapsing
the model to an invisible speck at the origin.

The same applies to `GU_TEXTURE_16BIT`.

**Implementation.** A uniform model-matrix scale of 32768 undoes it
(`meshdraw::MODEL_SCALE`). Precision is unaffected: the coordinates are
integers well inside the `i16` range, so they are exactly representable.

Texture coordinates need the normalisation undone *and* the N64's S10.5 fixed
point (32 units per texel) converted, giving
`sceGuTexScale(1024 / width, 1024 / height)` — `32768 / 32 = 1024`.

**Result.** Real ROM geometry renders. See
`docs/images/m3-rom-geometry.png`: 396 triangles, 2 draws, 60 FPS, 168us CPU.

**Confidence: certain.** Directly observed before and after.

**Note on the claim this invalidates:** the 16-bit vertex format was described
as "free, because the N64 data is already `i16`". It is still a bandwidth win
(16 bytes versus 24), but it is *not* free — it requires a compensating model
scale, and any code that mixes 16-bit meshes with float meshes must apply the
right scale to each.

---

## RE-021 — Lighting state is inherited, not carried by the display list

**Question.** After textures worked, meshes still rendered as saturated
red/green/cyan polygons. Why were so many primitives drawing packed normals as
vertex colours?

**Evidence.** N64 normals are `i8` components of a unit vector, so
`x² + y² + z²` sits near `127² = 16129`; arbitrary colours have no reason to.
Measured over the whole archive:

```
vertices in lit prims    555     100.0% look like unit normals
vertices in unlit prims  36356    69.4% look like unit normals
```

**Conclusion.** Two things at once:

1. The `G_LIGHTING` check has **no false positives** — every vertex in a list
   that set the mode really does carry a normal.
2. It has an enormous false-negative rate, because `G_LIGHTING` is set
   **per-object by `objdisplay.c` before the list runs**. A list relying on
   inherited state carries no geometry-mode command of its own, so per-list
   conversion cannot see it. Only 555 of ~37,000 vertices are covered.

This is a structural limit of converting display lists in isolation, not a
parsing bug.

**Implementation.** `pack::looks_like_unit_normal` plus an 80% per-primitive
majority vote, in `PackWriter::add_mesh`. The geometry mode is still trusted
when it fires; the data is only consulted when it does not. Deciding
per-primitive rather than per-vertex avoids splitting one surface into shaded
and unshaded halves.

**Result.** Coherent, correctly shaded models
(`docs/images/m3-textured-model.png`).

**Confidence: high** for the discriminator (100% precision on the known-lit
set, and genuine vertex colours like opaque white fall far outside the band).
**Medium** on the 80% threshold, which is a judgement call.

**Proper fix, later:** extract the `DObj`/`MObj` scene-graph setup so the real
per-object render state is known, rather than inferring it. That also supplies
the light colours (`MObj::light1color`/`light2color`) this currently replaces
with a single neutral key light.

**Update.** All three parts are now settled. The scene-graph half is RE-023,
which recovers all 363 `DObjDesc` arrays. The light-colour half turned out not
to matter: RE-024 measured them and they are white. And RE-027 extracts
`MObjSub` itself, so per-object render state — palette, prim/env/blend colour —
is read rather than inferred wherever a table is named. The 80% lit-primitive
threshold remains a judgement call, and still applies to the geometry the
`MObj` path does not cover.

---

## RE-022 — Mesh UVs run far outside 0..1 and need REPEAT

**Question.** Textures uploaded correctly (proved with a known-UV quad) yet
meshes rendered as fine coloured speckle.

**Evidence.** Measured UV range on one mesh: `u -55..119 texels`,
`v -2.7..65.7 texels`, against 64x32 and 64x48 textures. So coordinates
legitimately span several tiles and go negative.

**Cause.** Without `sceGuTexWrap(Repeat, Repeat)` those samples land outside
the texture and produce noise.

**Also fixed alongside:** V was being normalised against the texture's
*logical* height while `sceGuTexImage` had been handed the *padded*
power-of-two height, stretching V on any non-power-of-two texture.

**Confidence: certain.** The speckle disappeared immediately.

**Method note.** This was found by rendering a single texture on a quad with
known 0..1 UVs and float vertices. That isolates the upload path (format,
CLUT, swizzle, buffer width) from the mesh data, and turned "textures are
broken" into "textures are fine, the UVs are not" in one build. Kept as a
toggle in the viewer.

---

## RE-023 — `DObjDesc` arrays are a depth-tagged flattened tree

**Question.** A display list says *what* to draw, never *where*. Every mesh so
far rendered at the origin. Where does placement live?

**Answer.** In `DObjDesc` arrays, which `gcSetupCommonDObjs`
(`src/sys/objanim.c`) walks at load time:

```c
DObj *array_dobjs[DOBJ_ARRAY_MAX];          // 18
while (dobjdesc->id != ARRAY_COUNT(array_dobjs))
{
    id = dobjdesc->id & 0xFFF;
    if (id != 0) dobj = array_dobjs[id] = gcAddChildForDObj(array_dobjs[id - 1], dobjdesc->dl);
    else         dobj = array_dobjs[0]  = gcAddDObjForGObj(gobj, dobjdesc->dl);
    ...
}
```

So `id & 0xFFF` is the node's **depth**, and its parent is whichever node most
recently occupied `depth - 1`. The array is a pre-order flattening of a tree,
44 bytes per entry. Depth 18 terminates — the terminator is not a magic number
so much as an out-of-range depth, which is also why nothing can nest deeper
than 18.

The high nibble selects matrix composition (`0x8000` recalc-rot-rpy-sca,
`0x4000` kind46, `0x2000` kind48, `0x1000` kind50), and any high bit also
pushes a leading translate-only matrix.

**Finding them.** Nothing indexes these arrays. Five constraints recover them:

1. Terminator is `id == 18` followed by 40 zero bytes.
2. The first entry is always depth 0.
3. Depth never jumps by more than one — `array_dobjs[depth - 1]` must already
   be populated or the runtime would pass `NULL` as a parent.
4. Non-zero float components fall in a narrow band. Measured over the corpus:
   translate peaks at 2.34e4, rotate at 31.7 rad, scale at 123, and the
   smallest non-zero magnitude anywhere is 1e-6.
5. **`dl` is either NULL or the target of an intern relocation.** This is the
   decisive one: the archive loader already knows exactly which four-byte slots
   are pointers, so a plausible-looking offset that was never relocated is not
   a `DObjDesc`.

**Validation.** The decomp has typed 363 of these by hand, byte-compared
against the original ROM on every build. Scanning all 2132 archive files:

```
scanner: 363 arrays across 134 files
decomp:  363 arrays across 134 files
per-file counts: identical
the 180 arrays carrying an @offset annotation: 180 exact, 0 missing
```

Zero false positives, zero misses. Reproduce with
`tools/dobjdesc-ground-truth.py` piped into `romtool scene --expect`.

**Confidence: certain.**

**Method note.** The single mismatch on first run (file 296, Mario's joint
tree: 25 nodes found, 33 expected) was **my ground-truth extractor being
wrong**, not the scanner — that array carries `#if defined(REGION_JP)/#else`
blocks and I counted both branches. Worth stating plainly because the instinct
was to go fix the scanner. When a check disagrees with a reference, the
reference is a suspect too.

---

## RE-025 — A `DObj`'s display-list field is an undiscriminated union

**Question.** With scene graphs recovered (RE-023), only **3 of 22** of Sector
Z's node display lists converted. The rest failed with `EmptyCacheSlot(76)` —
and slot 76 is beyond the 32-entry vertex cache, so those bytes were not a
display list at all.

**Cause.** `DObj`'s display-list field is a union:

```c
union { void *dv; Gfx *dl; Gfx **dls; DObjMultiList *multi_list;
        DObjDLLink *dl_link; DObjDistDL *dist_dl; ... };
```

**Nothing in the data says which member is live.** The discriminator is the
`proc_display` callback the `GObj` was registered with — `gcDrawDObjTree` reads
it as `Gfx*`, `gcDrawDObjDLLinks` reads it as `DObjDLLink*` — and that lives in
game code, not in the archive.

Sector Z's nodes point at `DObjDLLink` arrays, confirmed against the decomp:

```c
DObjDLLink dStageSectorFile2_gap_0x3EE0_sub_0x558[2] = {
    { 0, dStageSectorFile2_DL_0x3EE0 },
    { 4, NULL },
};
```

The terminator is `list_id == ARRAY_COUNT(gSYTaskmanDLHeads)`, i.e. 4 — the
same "out-of-range index terminates" idiom as `DObjDesc`:

```c
while ((++dl_link)->list_id != ARRAY_COUNT(gSYTaskmanDLHeads))
```

**Resolution.** Disambiguate structurally. `DObjDLLink` is much the more
constrained shape — a `list_id` under 4, then a **relocated** pointer — so try
it first and fall back to "the field is a `Gfx*`". A real display list cannot
pass as a link array: `G_VTX`'s command word is `0x01xxxxxx`, far above 4.

Measured over all 2132 files: 1661 node fields resolve to 1661 lists, of which
**1417 convert with triangles**, 32 convert empty, and 212 fail (mostly
segmented or cross-file vertex pointers).

**Also found here:** feeding those authoritative offsets back through
`find_display_lists_at` — which re-applies the *discovery* heuristics — placed
only 742 of 1661. Decoding them directly places 1417. Once the game itself has
told you where a list starts, a heuristic can only lose information.

**Confidence: certain** for `DObjDLLink`. The other union members
(`Gfx**`, `DObjMultiList`, `DObjDistDL`) are not handled and account for part
of the remaining 264.

**Update — the `Gfx**` member is a pre/post-matrix pair, and it is what
fighters use.** See RE-026; the remaining 264 are now 0 conversion failures.

---

## RE-026 — Fighters share a vertex cache across joints

**Question.** After RE-025, 244 node display lists still failed to convert.
152 of them failed with `EmptyCacheSlot(N)` for small `N` — a *valid* display
list drawing triangles from cache slots nothing in that list had filled. They
clustered almost entirely in the fighter models (Yoshi, Samus, Donkey, Kirby,
Pikachu, Master Hand).

**Two separate causes, both in `ftDisplayMainDrawDefault`.**

**(1) The `Gfx**` member is a two-slot pre/post-matrix pair.**

```c
case 1:
    dls = dobj->dls;
    if (dls != NULL && dls[0] != NULL) gSPDisplayList(..., dls[0]);
    sp58 = gcPrepDObjMatrix(gSYTaskmanDLHeads, dobj);   // push this node
    if (dls != NULL && dls[1] != NULL) gSPDisplayList(..., dls[1]);
```

`dls[0]` draws in the **parent's** space, `dls[1]` in the node's own. The decomp
labels the arrays exactly that way (`338_YoshiModel.c`: *"DObj.dls pre/post-
matrix DL pairs @ 0x3308 (19 pairs, 152 bytes)"*). Reading such a pair as a
`Gfx*` decodes the two pointer words as commands and walks off into whatever
follows.

The shape is only two words, so the relocation test carries all the weight:
`dls[1]` must be a relocated pointer, `dls[0]` NULL or one too. `G_VTX`'s
command word is `0x01xxxxxx` — non-zero and never a relocation target — so a
display list cannot pass. `{ NULL, NULL }` pairs exist (a joint that draws
nothing) and carry no evidence of their own; they are accepted only when a
neighbouring pair vouches, which is sound because pairs only occur in arrays.

Cross-checked against every array the decomp annotates:

| file | decomp | recovered |
|---|---|---|
| 304 `NYoshiModel` | 18 pairs | 18 |
| 307 `NPikachuModel` | 14 pairs | 14 |
| 338 `YoshiModel` | 19 + 18 pairs | 37 |
| 344 `BossModel` | 23 pairs | 23 |

**(2) The RSP vertex cache survives across display lists — and that is how
Smash skins its joints.**

`gcDrawDObjTree*` walks the node tree emitting each node's list into one command
stream, so the 32-entry cache carries over. A joint's list routinely draws
triangles whose *other* vertices a previous joint loaded. Converting a list in
isolation therefore cannot work, no matter how correctly it was located.

Worse — and this is the interesting part — `G_VTX` transforms vertices by the
modelview **as it stands at load time**. A triangle referencing slots loaded
under two different joints spans two joint spaces. That is the N64's version of
skinning, done with no per-vertex weights at all.

**Resolution.** Convert a graph's lists as a *sequence* in draw order, threading
one cache through them. Each cached vertex records which node loaded it; a
triangle that borrows one carries it across by `inv(world_here) * world_there`.
Draw order needs no sorting: `gcAddChildForDObj` appends to the tail of the
sibling list and the draw walk is node-then-child-then-siblings, so the
pre-order flattening the `DObjDesc` array already is round-trips exactly.

RDP *material* state is threaded too, since the hardware keeps it: 394 textures
resolve against 378 with a per-list reset.

Inheritance has one hard limit, and it bites exactly here. `gcDrawMObjForDObj`
injects a material display list at segment `0x0E`, the runtime graphics heap,
and **that is where a fighter's texture binding comes from**. It is not in the
archive (RE-021). Letting the previous node's binding survive such a call
smeared another joint's texels across Samus's torso and inflated the texture
count by 117; a segmented call now invalidates the binding instead, which is
the honest answer — untextured, pending `MObjSub` extraction.

**Result.** Node lists that convert with triangles: **1417 → 1613**. Conversion
failures across all 2132 files: **244 → 0**. The 23 lists that still place
nothing are 16 that decode to pure state and 7 pair halves that are genuinely
NULL.

**Limits.** The rebase is exact for the rest pose only. Under animation a
stitched seam would tear, because the two halves of such a triangle move with
different joints — reproducing that needs the runtime to keep the cache, which
is a decision for when animation lands. `gcDrawMObjForDObj`'s material list is
resolved in RE-027.

**Confidence: certain** for the pair member (byte-exact against four annotated
files) and for the shared cache (the failures are gone and the geometry lands
where the decomp's `translate` values say it should).

![Samus assembled from her joint hierarchy](images/m4-fighter-joints.png)

Samus, 33 joints, 326 triangles, 25 draws, 649 µs, 60 FPS — white, because at
this point the segment-`0x0E` material was still unrecoverable. RE-027 fixes
that. The arm cannon is textured even here, because that binding is in the
display list itself.

---

## RE-027 — A fighter's palette lives in a table another file names

**Question.** RE-026 left fighters white. Their display lists set up a CI4
render tile, call segment `0x0E`, then run `G_LOADTLUT` — with no `G_SETTIMG`
in between to say *what* to load. The palette address is missing from the file
that draws with it. Where is it?

**Evidence.** `gcDrawMObjForDObj` builds the missing commands at run time from
the node's `MObj` chain. For Samus's joints the chain contributes exactly one
thing, because `MObjSub::flags` is `0x0004` = `MOBJ_FLAG_PALETTE` alone:

```c
gDPSetTextureImage(branch_dl++, G_IM_FMT_RGBA, G_IM_SIZ_16b, 1,
                   mobj->sub.palettes[(s32) mobj->palette_id]);
```

The `flags & (MOBJ_FLAG_SPLIT | MOBJ_FLAG_ALPHA)` block that would load the
TLUT is skipped, so the display list's own `G_LOADTLUT` does it — which is
precisely the gap in the file. `gcAddMObjForDObj` zeroes `palette_id`, so index
0 is the neutral costume.

Three facts make the rest recoverable:

1. **`MObjSub` is in the archive.** 789 of them are typed in the decomp,
   including every fighter `*Model` file.
2. **The table is parallel to the `DObjDesc` array.**
   `gcSetupCustomDObjsWithMObj(gobj, dobjdesc, p_mobjsubs, ...)` advances both
   in lockstep, so slot `i` holds node `i`'s NULL-terminated `MObjSub *` chain.
3. **The display list names the index.** `gcDrawMObjForDObj` writes one 8-byte
   `gSPBranchList` per `MObj` at the head of the heap, so a call to
   `0x0E000000 + 8i` selects `MObj` *i*. Samus's node 2 calls `0x10`, `0x08`,
   `0x00` — three materials, switching mid-list.

**The part I got wrong first.** Point 3 looks like a fingerprint strong enough
to *find* the table: decode the lists, get a required chain length per node,
search for a table matching that vector. It is not. Samus has two 33-node
graphs with identical demands and two equally well-formed tables, and across
the archive a fits-the-graph search agreed with the truth **26 times out of
50** — a coin flip. Shipping it would have put the wrong costume on half the
models it "recovered".

The pairing is stated outright in the data instead:

```c
struct FTCommonPart {
    DObjDesc *dobjdesc;
    MObjSub ***p_mobjsubs;
    AObjEvent32 ***p_costume_matanim_joints;
    u8 flags;
};
```

These live in the fighter's `*Main` file and point into its `*Model` file, so
both pointers are **extern relocations** the archive loader records exactly.
Samus's is `dSamusMain_commonparts_container` at file 217, naming graph
`0x3520` with table `0x0000` and graph `0x69D0` with table `0x4000`.

Two adjacent pointers into one file is a common shape, though: `FTAttributes`
stores `dobj_lookup` immediately before `shield_anim_joints`, both pointing
into the same `*ShieldPose` file, and **51** of those matched the record shape.
Requiring the named table to actually parse for that graph's node count removes
all 51 and nothing else.

**Result.** 44 graphs paired. Then the check that matters, since it uses data
the table does not contain: for **310 of 310** nodes, the recovered chain
length equals what the display lists' segment-`0x0E` calls ask for — no
mismatches. And all **459** resolved `MObjSub` offsets are ones the decomp
typed by hand (`tools/mobjsub-ground-truth.py`), 0 unaccounted.

Archive-wide textures **394 → 455**. Every fighter's two main model graphs are
covered.

**Limits.** 83 graphs want a material and no record names their table — mostly
stages and effect files, which use a different setup path. RE-028 covers the
stages. Stage tables also
reach chains in a *different* archive file through extern relocations; those
slots parse but read back empty. Only costume 0 and animation frame 0 are
taken, since `palette_id` and `texture_id` are runtime counters.

**Confidence: certain.** The pairing is read from a struct that names both
sides, and two independent checks — chain length against display-list demand,
and every offset against the decomp — agree completely.

![Samus with her Varia suit palettes](images/m4-fighter-materials.png)

The same frame as RE-026, same 326 triangles and 25 draws. 707 µs, 60 FPS.

---

## RE-028 — A stage is one struct, and its material table is one word further along

**Question.** RE-027 paired 44 graphs with a material table and left 83
unpaired, most of them stages. Fighters name their table through
`FTCommonPart`; what do stages use? And more basically — a stage's geometry is
spread over `StagePupupuFile2`, `File3`, a wallpaper file and a `GR*Map` file,
with nothing so far saying which files make up *Dream Land*. What ties them
together?

**Evidence.** The same struct answers both:

```c
struct MPGroundDesc {
    DObjDesc *dobjdesc;
    AObjEvent32 **anim_joints;
    MObjSub ***p_mobjsubs;
    AObjEvent32 ***p_matanim_joints;
};

struct MPGroundData {
    MPGroundDesc gr_desc[4];        // four render layers
    MPGeometryData *map_geometry;   // collision lines
    ...                             // bounds, fog, light angle, BGM, items
};
```

`MPGroundDesc` pairs a graph with its material table exactly as
`FTCommonPart` does — except `anim_joints` sits between them, so the table is
at `dobjdesc + 8` rather than `+ 4`. **One word.** That is the entire reason
every stage layer went unmatched while every fighter matched, and it is a good
argument for reading the struct rather than pattern-matching adjacency.

`sizeof(MPGroundData)` is **0xA8**, confirmed rather than assumed: three
`GR*Map` files place the header at `0x14`, and the decomp names the next
symbol in each at `0xBC`.

**Finding them.** A header is four 16-byte descriptors followed by
`map_geometry`. Every word in those 64 bytes is a pointer, so a non-zero
unrelocated word anywhere rules a candidate out; at least one `dobjdesc` must
land on a `DObjDesc` array we already recovered; and the camera and map bounds
at `0x6C`/`0x74` must enclose a positive area, since a stage the camera cannot
move in does not exist.

**Result.** **41 stage headers.** Dream Land's, checked field by field against
the decomp:

```
file 255 @ 0x14  bgm 0x0
  camera  top   4000 bottom  -2000 right   3900 left  -3900
  map     top   8300 bottom  -3500 right   9000 left  -9000
  layer 0  graph file 104 @ 0x1008 (21 nodes)  no materials
  layer 1  graph file 104 @ 0x1CE0 (2 nodes)   no materials
  layer 2  graph file 104 @ 0x2450 (4 nodes)   materials file 104 @ 0x1F50
  layer 3  graph file 104 @ 0x2BF8 (4 nodes)   no materials
  collision  file 104 @ 0x1F34  (not decoded)
  map nodes  file 152 @ 0x10F0
```

Every one matches `dGRPupupuMap_header`. `bgm 0x0` is `nSYAudioBGMPupupu`,
which is 0; Planet Zebes reads 1 and Yoshi's Island 8, both correct, so the
field is genuinely at `0x7C` and not accidentally right once.

Material pairings **44 → 56**, nodes whose chain length matches display-list
demand **310 → 342** with still **0** mismatches. Archive-wide textures
**455 → 485**.

**A gap in the ground truth, not in the extractor.** Four resolved `MObjSub`
offsets are at places `tools/mobjsub-ground-truth.py` cannot locate a decomp
symbol. They are real: e.g. file 117's `0x1E18` is
`dStageMetalFile2_Layer1MObj_MObjSub_real`, hand-named with neither an `@`
comment nor an offset in its name, and the other two members of its chain
(`0x1ED0`, `0x1F48`) both verify. The generator's message now says
"no decomp symbol is placed here" rather than implying a contradiction.

**Limits.** `map_geometry` — the collision lines a match actually needs — is
located but not decoded. Layers whose material table lives in another archive
file are skipped, as in RE-027. 71 graphs still want a table and have no record
naming one.

**Confidence: certain.** Every Dream Land field matches the decomp, three BGM
ids check out, and the demand check stays at zero mismatches over 32 more
nodes. The discovery filter lands on files **255–295 with no gaps and no
extras** — precisely the archive's `GR*Map` range, which is the full stage
list including the bonus and 1P maps. One header per map file, none missed,
nothing else mistaken for one.

---

## RE-029 — Stage collision is 2D polylines, and `vertex2` is a count

**Question.** RE-028 located every stage's `MPGeometryData` but did not read
it. Collision is the last thing M4's vertical slice is missing. What shape is
it in?

**Evidence.** Not triangles — 2D polylines, reached through two indirections:

```c
line_info[yakumono].line_data[kind] = { group_id, line_count };
// line ids group_id .. group_id + line_count, kind in {floor, ceil, rwall, lwall}
vertex_links[line_id] = { vertex1, vertex2 };
pos = vertex_data[ vertex_id[vertex1 + k] ].pos;
```

**The trap.** `MPVertexLinks { u16 vertex1, vertex2; }` reads like a segment
between two vertices. It is not. `mpCollisionCheckFloor` walks

```c
for (v = links[id].vertex1; v < links[id].vertex1 + links[id].vertex2 - 1; v++)
```

joining consecutive points — so `vertex2` is a **count** and a line is a
polyline. The field name says otherwise and the struct gives no hint.

Dream Land settles it. Line 3 is `{9, 2}`. Under "count" that is
`vertex_id[9..11]` → `(-2318, 0) .. (2318, 0)`: a platform symmetric about the
origin. Under "second vertex index" it would be `vertex_id[9]` to
`vertex_id[2]`, which is not.

**Lengths without adjacency.** No array stores its own length, and guessing
from the next symbol's offset would not survive a file the decomp has not
typed. It is not needed: the line count is the largest `group_id + line_count`
over every kind, the `vertex_id` length the largest `vertex1 + vertex2` over
every line, and the vertex count one past the largest index those name. Each
level bounds the next, out of the data itself.

**Result.** All **41** stages decode, 0 failures. Dream Land:

```
7 lines (floor 4, ceiling 1, walls 2), 42 map objects
  line 0 Floor      (570,1542) (0,1542) (-570,1542)
  line 1 Floor      (1892,907) (1421,907) (951,907)
  line 2 Floor      (-951,904) (-1396,904) (-1841,904)
  line 3 Floor      (-2318,0) (2318,0)
  line 4 Ceiling    (1972,-1072) (-1972,-1072)
  line 5 RightWall  (2318,0) (2307,-124) (2290,-331) (2075,-834) (1972,-1072)
  line 6 LeftWall   (-1972,-1072) ... (-2318,0)
  object kind 0 at (0,6)       kind 1 at (-1397,906)
  object kind 2 at (1,1545)    kind 3 at (1421,909)
```

Three floating platforms — two low and one high and centred — over a main
platform, with the rounded underside walls sloping from `±2318, 0` down to
`±1972, -1072`. That is Dream Land. `MPMapObjKind` 0–3 are the player starts,
and they land one per platform, which is how Dream Land opens a match.

Final Destination cross-checks it from the other direction: **one** floor
line, `(-2508, 0) .. (2508, 0)`, all four spawns on it. Exactly one flat
platform and nothing else, which is the whole stage.

**Limits.** `MPVertexInfo` is deliberately skipped: the collision code indexes
it by line id for early rejection, but it is absent from `MPGeometryData`
because the runtime derives it on load. Yakumono transforms are runtime state,
so lines belonging to a moving group are in that group's space, not world
space.

**Update.** Packed and queried as of RE-030; the "not yet packed" limit this
entry originally recorded is closed.

**Confidence: certain.** Two stages reconstruct feature for feature from
independent knowledge of what they look like, all 41 decode, and the array
lengths are derived rather than assumed.

---

## RE-030 — Surface flags say how a stage plays, and spawns prove the query

**Question.** RE-029 decoded the collision lines but stopped there. Two things
were still unknown: what a collision vertex's `flags` word means, and whether
the whole chain — extractor, pack, reader, query — actually agrees, which no
unit fixture can tell you.

**The flags.** `mpdef.h` names the bits: the upper byte is surface state and
the lower byte an `MPMaterial` that selects friction. Two bits decide how a
floor behaves:

```c
#define MAP_VERTEX_COLL_PASS  (1 << 14)   // may be dropped through
#define MAP_VERTEX_COLL_CLIFF (1 << 15)   // its ends can be hung from
```

Reading them back on Dream Land is what makes this certain, because the answer
is checkable against how the stage plays:

```
line 0 Floor  pass    (570,1542) (0,1542) (-570,1542)
line 1 Floor  pass    (1892,907) (1421,907) (951,907)
line 2 Floor  pass    (-951,904) (-1396,904) (-1841,904)
line 3 Floor  cliff   (-2318,0) (2318,0)
line 4 Ceiling        (1972,-1072) (-1972,-1072)
line 5 RightWall      (2318,0) ... (1972,-1072)
```

All three floating platforms are `pass` — in Dream Land you drop through them
by holding down. The main platform is `cliff` and *not* `pass` — you cannot
drop through it, and you can grab its ledges. The ceiling and walls are
neither. Four independent facts about the stage, four matches. No structural
check could have produced that.

**The query.** `mpCollisionCheckFloorLineCollisionSame` does not test a point
against a surface; it tests the **swept segment** from where a fighter was to
where it wants to be. That is what stops a fast faller from crossing a platform
in one frame. It dispatches on whether the segment is level, because the
tilted solver divides by the segment's horizontal extent:

* `mpCollisionCheckFCSurfaceFlat` — level segments, and only while falling.
* `mpCollisionCheckFloorSurfaceTilt` — sloped segments, a line/line crossing
  with the endpoints snapped when a fighter lands within `0.001` of a joint,
  so a landing between two segments falls on one of them rather than through.

`0.001` is not a tuning knob; it is written literally throughout
`mpcollision.c`, and it is what lets a fighter standing exactly on a surface
still register as touching it.

**Validation the structure does not contain.** The game places a spawn point
just above the surface it starts on. So drop every stage's player spawns
straight down through the packed data and see where they stop:

```
stage  0  file 255  7 floor segments in 1 group(s)
  P1  spawn (     0,     6)  lands on line 3 at y    0.0,   6 below cliff
  P2  spawn ( -1397,   906)  lands on line 2 at y  904.0,   2 below pass
  P3  spawn (     1,  1545)  lands on line 0 at y 1542.0,   3 below pass
  P4  spawn (  1421,   909)  lands on line 1 at y  907.0,   2 below pass

40/41 stages catch every spawn (158 landed, 4 did not)
```

**158 of 162** spawns land, and almost all of them come to rest **2–6 units**
below where they started. That margin is the real result: nothing about the
pack format or the solver forces it, and a wrong offset anywhere in the chain
would scatter it. Sloped stages land on fractional heights (`y 303.5`), which
means the tilted solver is exercised on real data too.

**The four misses are the known limit, not a bug.** All four are on one stage,
file 284 — the bonus stage built entirely from moving platforms. Its floors sit
in yakumono groups 1–11, every one of them an identical rectangle at the
origin, which is what a platform stored in its *own* space looks like. The
runtime offsets those by the group's `DObj` before testing; we have no group
transforms, so they are tested where they rest. One stage failing, and it being
exactly the stage made of moving platforms, is the failure isolating itself.

**Layering.** The query lives in `ssb-game` (Layer A) and takes an iterator of
segments, so it allocates nothing and never learns the pack format; `ssb-rom`
never learns the game logic. The six-line adapter between them is duplicated
per consumer, which is cheaper than a shared type dragging one crate into the
other. `romtool collide <pack>` is that adapter plus the check above.

**Seen, not just counted.** The viewer draws a stage's render layers with its
collision polylines over them, through the same transform, and Dream Land's
lines land on its platforms: two short floor lines exactly on the top surfaces
of the left and right side platforms, one long line along the top of the main
platform, slightly inset from the rendered slab — which is right, because
collision is `±2318` and the drawn geometry is a little wider. 16 segments
drawn, and Dream Land's seven polylines are `2+2+2+1+1+4+4 = 16`. 658 µs at 60
FPS (`docs/images/m4-stage-collision.png`).

Peach's Castle cross-checks it on ground that is not flat: its lines follow the
sloped castle top, matching the fractional landing height (`y 303.5`) the spawn
drop reported for that stage. A consistent offset would survive every numeric
check above and would be obvious here; there is none.

**Confidence: certain** for the flags, which Dream Land confirms against
gameplay, and for the geometry, which now agrees visually on two stages.
**High** for the solver: it is a faithful port and the spawn margins agree
across 40 stages, but no fighter has stood on it yet.

*Update (RE-031): one has. 158 of 158 spawns now hold a simulated fighter still
for a second, and two stages confirm it on device.*

---

## RE-031 — A fighter stands on a stage, and two solvers agree it is the right one

**Question.** RE-030 left the collision query proven against single-shot
queries but never driven by anything. Does the ported physics, stepped a tick
at a time through the ported collision process, actually leave a fighter
standing on a stage — and stay there?

**What was missing.** The query answers "did this movement cross a floor". A
tick needs three more things from `mp/mpprocess.c`, and each is a behaviour
rather than a formula:

* **`mpProcessSetCollideFloor` — a grounded fighter's height is re-read every
  tick**, not carried over from the last one. There is no slope code in Smash
  64; walking up a hill *is* this function sampling the surface under the new
  x. When a fighter's x leaves its line, that same absence is how it falls off
  a ledge.
* **`mpProcessSetLandingFloor` — landing past the end of a line moves the
  fighter to the line's corner** rather than leaving it hanging at whatever
  height it arrived with. This is what puts a fighter *on* a ledge.
* **`mpProcessUpdateMain` — a tick's movement is subdivided** into 250-unit
  pieces before any of it is tested. The original's own comment gives the
  reason: 250 is a tenth of maximum knockback velocity, 2500 units per frame.

And one query the swept test cannot answer, `mpCollisionCheckProjectFloor`: a
straight downward probe for what a body is *above*. That is how a spawn point
becomes a standing position, and it shares no arithmetic with the swept
line/line solver.

The floor material turned out to be a real table rather than a guess —
`dMPCollisionMaterialFrictions` @ 0x8012C4E0, sixteen multipliers on the
character's own traction. `nMPMaterial3` is 1.0 against the common material's
4.0, and the decomp's comment on that enum reads "presumably ice due to low
traction". The number agrees: nothing else in the table is close.

**Validation: two solvers that must agree.** Place a fighter at each of the
158 landable spawns twice. Once with the vertical probe, once by dropping it
under real gravity and letting the swept query catch it. They have no code in
common beyond reading the same segments.

```
spawns      162
settle      158 have a floor beneath them
agree       158/158 land where the vertical probe says they should
at rest     158/158 do not move over 60 ticks (worst drift 0)
substep     158/158 caught when dropped at maximum knockback velocity
```

**Worst drift is exactly 0**, not "small". A sign error in the landing snap or
in the grounded update shows up as drift, and drift compounds — 158 fighters
holding still for 60 ticks each is not something a nearly-right implementation
produces. Sloped stages are in that set: Peach's Castle's P3 settles on
`y 303.5`, the fractional height RE-030 measured, now reached by simulation
rather than a single query.

**An honest negative.** Subdividing the movement changed the outcome for **0**
of the 158. That is expected and it is reported rather than glossed: the swept
test is exact along a straight line, so while only floors exist there is
nothing for substepping to catch. It will earn its keep when a wall can deflect
a fighter mid-tick, and porting it now is cheaper than discovering later that
knockback tunnels.

**On device.** Dream Land, at the stage's own first spawn:

```
stage 0/41  file 255  @0x14
fighter ground   x 0  y 0  line 3  mat 0  air 0
tris 175  layers 4  coll-segs 16
cpu 666us / budget 16667us          60.0 FPS
```

The host simulation says `spawn (0, 6) lands line 3 at y 0.00 after 12 ticks`.
The device says line 3, y 0. Peach's Castle repeats it on ground that is not
flat: `x -2556  y 557  line 3`, against the host's `spawn (-2556, 572) lands
line 3 at y 557.00 after 18 ticks`. Same line, same height, on a slope.

The whole tick costs **8 µs** — 666 against 658 for the same view without it —
because the collision adapter is an iterator that reads segments straight out
of the mapped pack. There is no per-frame setup to pay for and nothing is
allocated (`docs/images/m4-fighter-on-stage.png`).

**What the marker is, and is not.** It is a stem standing on the contact point
with a crossbar at the origin, green when grounded and white when airborne —
not a character model. The pack holds no record naming which object is Mario,
and picking one that looked right would be a fingerprint rather than a fact
(the rule RE-027 was built on). What is being shown is the simulation, not the
character.

**Confidence: high.** The two solvers agreeing across 158 real spawns, at zero
drift, is strong evidence the floor path is right. It is not evidence about
walls, ceilings, ledge-grabs or moving platforms, none of which are ported —
and a fighter here has no state machine, so it cannot walk, jump or be hit.

---

## RE-024 — The shipped light colours really are neutral

**Question.** RE-021 substituted a single neutral key light for
`MObjSub::light1color`/`light2color`, and flagged that as a known compromise.
How wrong is it?

**Evidence.** Every `MObjSub` the decomp has typed, across the whole
`src/relocData` corpus:

```
0xFF, 0xFF, 0xFF, 0x00    35
0x80, 0x80, 0x80, 0x00     1
```

**Conclusion.** 35 of 36 are pure white and the last is neutral grey. The
substitution is not a meaningful approximation — the colours carry no hue, so
the shading difference is at most a brightness scale on one material.

**Confidence: medium.** Only 36 `MObjSub` instances have been typed so far, so
this is a sample rather than the full corpus. It is enough to demote the light
colours from "known limitation" to "not worth chasing", but not enough to claim
no material anywhere tints its lights.

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
* `view 362x272` — the value `coord::pillarboxed_viewport()` returns. **This
  line overstated what it showed.** Printing the helper's return value confirms
  only that the helper was called; it says nothing about the GE viewport, which
  was in fact still the full 480x272 until RE-034 measured the resulting
  distortion. "Confirmed on-device" should mean a rendered consequence was
  measured, not that a number reached the overlay.

**Caveat.** The scene is four triangles and one fighter's worth of physics, so
13us says nothing about how a real match will perform. Its value is as a
**baseline**: the platform layer, clock and submission path cost essentially
nothing, so future frame time can be attributed to the game rather than to
scaffolding.

**Confidence: high** for the measurement; explicitly **not** a performance
prediction. Real PSP hardware is ~333 MHz against an emulator on a desktop CPU
— these numbers do not transfer (plan §37).

---

## RE-032 — The fighters' real numbers, and what a guessed constant hides

**Question.** RE-031 stood a fighter on a stage using physics constants I made
up. `PhysicsAttributes::default()` carried a comment calling them "a neutral
baseline for tests". Where are the real ones, and does using invented numbers
actually cost anything if the shape of the physics is right?

**Answer to the second question first: yes, and not subtly.** Mario's real
gravity is `2.4` per frame and his terminal velocity `44.0`. My baseline used
`0.09` and `1.7` — off by 26x. Smash 64 works in the same large world units as
its collision geometry, where a stage spans several thousand units and Mario
stands 320 tall, so a constant that looks like a sensible small acceleration is
not conservative, it is a different game. The old numbers still produced a
fighter that fell and landed in the right place; it simply took 300 frames to
drop what should take 20. Nothing failed. That is what made it worth fixing.

**Where they live.** Every per-character constant is one `FTAttributes` struct
at a fixed offset inside that character's main archive file:

```c
attr = lbRelocGetFileData(FTAttributes*, *fp->data->p_file_main, fp->data->o_attributes);
```

So the record that matters is `(file id, byte offset)`, and it sits in
`dFT<Name>Data` — in the game code's data segment, not in any archive file.
The decompilation's `relocData` sources annotate each fighter main file with
the size of everything preceding the attribute struct, which is where the
offsets in `ssb_rom::fighter::FIGHTER_FILES` come from. That is a record naming
both sides, not a value picked because it looked plausible in a hex dump — the
distinction RE-027 turns on.

**Validation: two independent readings of the same bytes.** An offset table is
a claim about where 27 structs begin, and the cheapest way to be wrong is to be
*almost* right: a table off by one word still decodes into floats that look
like numbers. So `romtool fighters --verify` decodes all 27 out of the ROM and
compares every scalar against the values the decompilation writes out in its
own C literals — one reading from the compressed archive, one transcribed by
hand years ago by somebody else.

```
27/27 decode to plausible values
verified    27 fighters, 1215 fields against the decompilation
            all agree
```

Five of the 27 have no size comment to read the offset from and had to be
derived from the last annotated block instead; those five are in the 27 that
agree, which is what makes the derivation trustworthy rather than assumed. A
wrong offset does not yield 44 matches and one miss.

The table also reads correctly against the game as played: Kirby and Jigglypuff
have 6 jumps where everyone else has 2, Metal Mario's gravity is 4.8 against
Mario's 2.4 with terminal velocity 100 against 44, Giant DK is heaviest and
fastest-falling, Link's jumpsquat is 7 frames. None of that was used to find
the offsets, so it is a free check.

**The collision body is a diamond.** `MPObjectColl` is `{top, center, bottom,
width}` and I had ported it as a box named `{top, bottom, left, right}`. The
four points are `(0, top)`, `(±width, center)` and `(0, bottom)` — so `center`
is a *height*, the waist where the body is widest, not a centre point. Mario's
`{320, 190, 0, 150}` is a body 320 tall whose widest span is 300 across, at hip
height. `bottom` is `0.0` for every playable character, which is why the
grounded update can put the translation straight onto the surface with no
offset. `ftDisplayMain` sizes the shadow from `width` and `center`, so these
are not physics-only numbers.

**A bug the real numbers exposed.** With `air_accel = 0.025` the ported air
drift barely moved the fighter, and chasing that found two errors in
`ftPhysicsApplyAirVelDrift`:

* Drift scales with **how far** the stick is pushed — `vel_air.x += stick_x *
  air_accel` — not just which way. My port used the sign only, making drift 80x
  too weak at full deflection. Under the invented `air_accel = 0.05` that
  looked like "drifting is a bit sluggish"; under the real value it takes 1200
  frames to reach a cap the game reaches in 17.
* Friction runs **every** frame, including while the stick is held. So the real
  steady-state drift speed is not the clamp but the clamp minus one frame of
  friction, and releasing the stick does not change which function runs.
* The deadzone is a band, `|stick_x| < 8`, not just zero.

The first of those is the kind of error a hand-picked constant can hide
indefinitely, because both the constant and the formula were wrong in
compensating directions.

**Re-validation.** RE-031's whole simulation was proven under the fake
constants. Re-run under the real ones, terminal velocity 44 instead of 1.7:

```
agree       158/158 land where the vertical probe says they should
at rest     158/158 do not move over 60 ticks (worst drift 0)
```

Unchanged — and the resting heights are identical to the last digit
(`557.00`, `303.50`, `-269.00`), while the fall times drop from 18 ticks to 4.
That is the right invariant: where a fighter lands must not depend on how fast
it fell. Had the swept query been resolving to a substep boundary rather than
the true crossing, a 26x change in fall speed would have moved those fractions.

**Confidence: high** for the values — 1215 fields agreeing with an independent
transcription is not a coincidence, and the roster reads true against the game
as played. The pack now carries all 27 characters' constants (5 KB, format
version 4), including camera, shadow and shield fields no subsystem reads yet.

**Not verified on device this round.** The session was locked when the
screenshot was taken, so the capture is the lock screen rather than the
emulator — the failure mode `tools/run-ppsspp.sh` documents at length and the
one that has cost this project the most time. The EBOOT builds for the PSP
target and PPSSPP loads it; the on-device overlay showing `attrs pack` and
Mario's real constants is unconfirmed until the next run.

---

## RE-033 — The status machine, and a tap that is a counter rather than an edge

**Question.** RE-031's fighter could stand and fall and nothing else. Smash 64
drives every action through a *status*: a fighter is always in exactly one, and
it decides which physics run, which inputs are heard, and what can be entered
next. What does that machine actually look like, and what does it take to walk,
dash, jump and drop through a platform?

**The interrupt chain is an ordered list.** `ftCommonGroundCheckInterrupt` is a
macro of nineteen `||`-chained calls, each of which sets a status *as a side
effect* and returns whether it did. Short-circuit evaluation is the priority
system — there is no dispatch table and no explicit precedence. Jumpsquat is
tested before dash, dash before squat, squat before turn, turn before walk, so
holding up-and-forward on the frame you flick gives a jumpsquat and not a dash.
Reordering that list silently changes what the game does on ambiguous inputs.

**The most important thing in the whole input model is one counter.**
`ftMainProcessInput` keeps `tap_stick_x`, and despite the name it is not an edge
flag. It **resets to 1 on the frame the stick crosses ±20**, increments while
the stick stays outside, and is pinned to 254 while inside. A dash is
`|stick_x| >= 56 && tap_stick_x < 3`.

The window is therefore measured from the **deadzone crossing at 20**, not from
reaching the dash's own threshold at 56. Consequences that all fall out of that
one fact, with no gesture recognition anywhere:

* Flick from neutral to full: 20 and 56 are crossed on the same frame, the
  counter reads 1, you dash.
* Roll the stick out over five frames: 20 is crossed long before 56, the counter
  is past 3 by then, you walk.
* Tilt to 30 and *then* push to full within two frames: still a dash, because
  the counter never saw the stick come back to neutral. This one surprised me —
  it broke a test I had written expecting a walk, and it is correct.
* Cross straight from +30 to −30: a new crossing, because the test is per-sign
  and not on magnitude, so the counter restarts and a dash-back is available
  immediately.

**Two chains, not one.** A walking fighter uses `ftCommonWalkCheckInterrupt`,
which differs from the standing chain in two ways that both matter:

* It ends in **Wait**, not Walk. A walk never re-enters itself; changing walk
  speed happens *after* the chain declines, by phase-matching the animation:
  `new_frame = (frame / old_length) * new_length`. Ending the chain in Walk
  instead — which is what I wrote first — makes the phase code unreachable and
  resets the leg animation to frame zero on every frame the stick moves.
* There is **no Turn** in it. Pushing gently behind you while walking satisfies
  `ftCommonWaitCheckInputSuccess` (`stick * lr < 0`) and goes to Wait; Wait's
  chain turns on the following frame. So a walking turnaround costs one frame
  that a standing turnaround does not. A *hard* flick backwards is a dash input
  and turns immediately either way, which is why dash-dancing feels different
  from walking back and forth.

**What the extracted attributes bought.** RE-032's numbers are what make any of
this real rather than parameterised. Jumpsquat is `kneebend_anim_length` — Mario
3 frames, Link 7, Metal Mario 8. Dash-to-run is `dash_to_run`, and it is a
**one-frame window**: `dash_to_run <= anim_frame < dash_to_run + anim_speed`,
so holding forward through exactly frame 14 runs and missing it does not. Walk
animation lengths (90 / 60 / 40) are what the phase-matching divides by.

The jump arc falls straight out of the same data with nothing tuned:
`80 * 0.7 + 26 = 82` units of initial velocity against gravity 2.4 and terminal
velocity 44 gives a peak of **1360 units over 74 frames** — Mario is 320 tall,
so a full hop clears a little over four of him and lasts about 1.2 seconds.
That is Smash 64's characteristically floaty jump, and it was not aimed at.

**An ordering bug the machine introduced, and the test that names it.**
Entering a grounded status transfers `vel_air.x` into `vel_ground.x`
(`mpCommonSetFighterGround`). So does `Fighter::land`. Doing both — which is
what happens now that landing sets a status *and* lands — runs the second
transfer against an already-zeroed `vel_air` and deletes the fighter's
horizontal momentum. The symptom is a fighter that lands from a running jump
and stops dead, which reads as a physics problem and is an ordering one.
`land` now guards on actually being airborne, mirroring `become_airborne`, and
`landing_keeps_the_horizontal_momentum_a_jump_carried` fails without the guard.

**What cannot work yet, and why it is `None` rather than a guess.** Most
statuses end when their animation runs out, which the original tests as
`gobj->anim_frame <= 0.0`. That reads like a countdown and is not: `anim_frame`
counts *up* by `anim_speed`, and `ftAnimParseDObjFigatree` writes the leftover
negative remainder into it when the animation script ends. So `<= 0.0` is a
sentinel, while `anim_frame <= 5.0` a few lines away in the same file is a
genuine "within the first five frames" test. Both readings are right and they
coexist.

The lengths themselves live in `AnimJoint` / `AObjEvent32` data that is not
extracted. The motion scripts in `<Name>MainMotion` are *event* scripts — sound,
dust, flags — and end well before the animation does, so they are not a
substitute: `dMarioMainMotion_Turn` waits 6 frames, sets a flag and ends, while
the Turn status runs as long as the animation. So `StatusTiming::anim_length` is
`Option<f32>` and `None` means the status cannot time out. Dash, Turn, RunBrake,
Landing and Squat-to-SquatWait are affected: they end only by being interrupted.
A made-up duration would be invisible in a screenshot and wrong in every replay,
so there isn't one.

**Confidence: high** for the transition logic and the input model, which are
transcribed from the decompilation with the addresses cited and tested against
the behaviours they are supposed to produce. **Explicitly incomplete** on
timing: five statuses cannot end on their own until animation data is
extracted.

**Not verified on device.** The session was locked for both attempts, so the
harness never got a frame — it reported the cause correctly the second time
("PPSSPP never finished graphics init -- a locked screen does this"). The EBOOT
builds and the input is wired (stick moves, C-left jumps), but nobody has
watched Mario walk on a PSP yet.

---

## RE-034 — The pillarbox that was never applied, found by measuring a fighter

**Question.** RE-033 shipped without a device check because the session was
locked. With it unlocked, does the status machine actually run on hardware, and
do the extracted constants reach it?

**Yes.** Dream Land, at the stage's own first spawn:

```
stage 0/41  file 255  @0x14
fighter land       x 0  y 0  line 3  mat 0  air 0
attrs pack         grav 24/10  tvel 44  body 150w 320h
tris 175  layers 4  coll-segs 16
cpu 686us / budget 16667us            60.0 FPS
```

`attrs pack` says the constants came from the pack rather than the built-in
fallback, and `grav 24/10  tvel 44  body 150w 320h` are Mario's real extracted
values (RE-032) arriving intact on the far side of the pack format. `fighter
land` is the status machine: the fighter fell the few units from its spawn and
is in `LandingLight`. `x 0 y 0 line 3` matches the host simulation exactly.

**Then measuring the marker found a rendering bug that had been there all
along.** The fighter is drawn as its collision diamond, whose proportions are
known: 300 units wide, 320 tall, waist at 190. Those are *ratios*, so they can
be checked from a screenshot without knowing the camera at all:

| | measured | expected |
|---|---|---|
| width / height | **1.227** | 0.938 |
| waist / height | 0.571 | 0.594 |

The waist is right, so the diamond is being built correctly. The width is 31%
too large — and 1.227 / 0.938 = **1.31**, which is 480/362.

`coord::pillarboxed_viewport()` returns a 362x272 region, and `main.rs` fed its
aspect ratio to `sceGumPerspective`. But `Gpu::init` set `sceGuViewport` and
`sceGuScissor` to the full 480x272. So the projection was built for a 4:3
viewport and then stretched across a 16:9 one — producing precisely the
distortion the pillarbox exists to prevent, on every frame this project has
ever rendered. Setting the GE viewport and scissor to the same region fixes it:

| | before | after | expected |
|---|---|---|---|
| width / height | 1.227 | **1.000** | 0.938 |

The residual is 1.3 pixels on a 21-pixel-tall shape.

**The documentation had claimed this was verified.** RE-005 listed
`view 362x272` under "confirmed on-device". What that actually confirmed was
that the helper had been *called* and its return value formatted into the
overlay. Nothing had checked the GE viewport, and nothing had looked at a
rendered consequence. Those notes are now corrected in place.

The lesson generalises past this bug: **printing a value proves the value was
computed, not that anything acted on it.** A number in a debug overlay is an
input to the renderer, not evidence about its output. "Confirmed on-device"
has to mean a rendered consequence was measured — which is why the fighter
marker being a shape with *known proportions* was worth more here than any
amount of overlay text. It could be checked against nothing but itself.

**A second, smaller thing the same screenshot showed.** The grounded fighter
and solid floors were both drawn `0xFF40_FF40`, so a fighter standing on a
floor was the same colour as the line under its feet. It is now magenta — the
one hue the collision palette had left (green floors, red ceilings, blue and
amber walls), so the fighter cannot be confused with any surface it touches.

**Verifying the absolute size, not just the proportions.** The first pass
stopped at ratios and said the absolute size was unverifiable because "the
camera spins and is tilted". That was wrong, and worth recording: the stage view
passes `[0.0, 0.0, 0.0]` to `model_transform` and is face-on always — `spin`
only applies to the object and mesh views. So every point at world z = 0 shares
one depth, screen position is exactly linear in world x and y, and ratios of
extents need no camera model at all.

That makes the stage's own collision the ruler. Dream Land's line 3 runs
`(-2318, 0)` to `(2318, 0)`, exactly 4636 units, and its side platforms sit at
y = 904 and y = 907:

| | measured | from | px/unit |
|---|---|---|---|
| horizontal | 297 px | line 3, 4636 units wide | 0.064064 |
| vertical | 58 px | y = 0 to y = 904 | 0.064159 |

**The two agree to 0.15%.** Before the viewport fix they were 0.0848 against
0.0641 — 32% apart. That is the fix confirmed against stage geometry rather
than against the fighter it was found with.

Against that ruler the diamond measures 328 units tall (expected 320, +0.5 px)
and its waist 187 (expected 190, −0.2 px). Width read 328 against 300, which is
+1.8 px — more than the others, and worth chasing rather than waving through.

**Chasing it needed a bigger fighter.** At 21 px tall one pixel is 15 game
units, so 300 and 320 are 1.3 px apart and simply not resolvable. Zooming in
(`docs/images/m4-fighter-diamond.png`, 98 px tall, 1 px = 3.3 units):

| | 21 px tall | 98 px tall | expected |
|---|---|---|---|
| width / height | 1.000 (+6.7%) | 0.918 (−2.0%) | 0.938 |
| absolute error | +1.3 px | −1.9 px | — |

The *relative* error falls from 6.7% to 2.0% as the shape grows 4.7x while the
*absolute* error stays around 1.5 px. That is the signature of a fixed
measurement bias — antialiasing spreading a stroke outward at the vertices,
which the platform lines confirm by reading 1 px under their known lengths —
and not of a scale error, which would hold its percentage at every zoom.

Taking the height as its known 320 units, the drawn diamond is 294 wide and its
waist at 193, against 300 and 190. Correct to within two pixels of a 98-pixel
shape.

**Confidence: high**, now for the absolute size as well as the proportions. The
scale is verified against Dream Land's own collision geometry in both axes, and
the residual error is shown to be measurement bias by watching it shrink with
zoom rather than by arguing that it ought to be.

---

## RE-035 — Animation lengths, and the eighteen scripts that agree on each one

**Question.** Five ground statuses — Dash, Turn, RunBrake, Squat and Landing —
had no duration. `FTAttributes` does not contain one, so `StatusTiming::
anim_length` was `None` for all of them and they could only end by being
interrupted. A dash held forever stayed a dash.

**Where the length actually lives.** The status update functions say it
outright:

```c
void ftCommonDashProcUpdate(GObj *fighter_gobj) {
    if (fighter_gobj->anim_frame <= 0.0F) {
        fp->physics.vel_ground.x *= 0.75F;
        ftCommonWaitSetStatus(fighter_gobj);
    }
}
```

`anim_frame <= 0.0` is the sentinel `ftAnimParseDObjFigatree` writes when the
animation script runs out. So the duration is a property of the *animation*,
and the five statuses that lacked one are exactly the five whose `proc_update`
is an animation-end test — `ftAnimEndSetWait` for RunBrake, SquatRv and both
landings, `ftAnimEndSetFall` for Pass.

**Three pairing records, not a fingerprint.** Getting from a status to its
animation file is a chain where every hop is a record that names both sides:

```
dFTCommonActionStatusDescs[status - 6].mflags.motion_id
  -> dFT<Name>MotionDescs[motion_id].anim_file_id
    -> relocData file <id>_FT<Name>Anim<X>.c
```

The first two tables are in the game code's data segment, which this project
does not read, so the resolved `(fighter, status) -> file id` pairing is
transcribed — the same arrangement as `FIGHTER_FILES`. `tools/gen-anim-table.py`
produces it, so the transcription is reproducible rather than hand-typed.

**The check that caught the one real bug.** A first pass matched motion table
entries with `\{\s*&ll(\w+?)FileID`, which silently skips the
`{ 0x00000000, 0x80000000, 0x00000000 }` placeholders. Kirby has two of them —
he has no aerial-jump animation — so every motion id after 17 shifted by two
and his Pass resolved to `FTKirbyAnimTeeter`. The generator now checks the
resolved animation's *name* against the slot: the name comes from the
decompilation's file names while the index comes from the `FTCommonMotion`
enum, so a table parsed one entry out of step resolves to an animation whose
name no longer fits its status. That check reports the fault directly instead
of leaving it to be noticed as a wrong number.

Two other mismatches it flagged turned out to be real and were kept:
Jigglypuff's landing animation is called `JumpSquat` (it serves both KneeBend
and Landing, as everyone else's `LandingAirX` does), and Master Hand's entire
common status table points at one looping idle because it never walks.

**Why the decoded number can be trusted.** A figatree file opens with a pointer
table, one script per model joint. The table's length is not stored: the first
non-null pointer is the offset of the first script, which is exactly where the
table ends. Each script is a stream of 16-bit `{ opcode:5, flags:10, toggle:1 }`
commands, and the animation's length is the sum of the payloads of the ones
that advance the clock (opcodes 1, 2, 4, 7, 9 and 14 — the `Block` variants;
the non-`Block` ones set a track's interpolation length without consuming
time).

Every joint carries its own independently encoded script, and the exporter gave
them all the same total. So the decoder walks *all* of them and requires
unanimity — eighteen scripts agreeing on one number. That is a real test rather
than a formality, because the walk is self-checking: a wrong word count for any
command desynchronises the stream, and the walk then runs off the end of the
script instead of finding its terminator.

Across the decompilation's 1775 animation files the model agreed on 1736. The
37 exceptions are all entry and cutscene animations — `Appear`, `Arwing`,
`BlueFalcon`, the Master Hand set — which use the 32-bit `AnimJoint` encoding
and are not figatrees at all. Not one gameplay animation was among them.

The looping animations fall exactly where a status should never time out: Wait,
the three walks, Run, Fall, FallAerial and SquatWait all contain a `Loop`
command and never terminate. Those are precisely the statuses that leave by
being interrupted.

**Two independent readings.** `romtool anims --verify` compares the lengths
decoded from compressed archive bytes against the ones the generator computed
from the decompilation's hand-written C macros. **189 lengths across 27
fighters, all agreeing.**

```
fighter         Dash      Turn  RunBrake     Squat   SquatRv   Landing      Pass
Mario             23        12        23         8        12         7        25
Donkey            31        12        30         6         8         8        25
Captain           29        12        30         8        10        11        30
Link              31        12        25         4         4         8        24
Boss           loops     loops     loops     loops     loops     loops     loops
```

Free consistency checks nobody aimed at: **Turn is 12 frames for every
character in the game** — the one length the whole roster shares. Donkey Kong
and Link have the longest dashes and Captain Falcon the worst landing lag at
11 frames against everyone else's 7-8, which is what he is known for. Luigi
shares Mario's Turn, Squat, SquatRv, Landing and Pass *files* (507, 515, 517,
518, 519) while having his own Dash and RunBrake — a Mario clone with a few
unique animations, which is exactly what he is.

**Landing is where playback speed matters.** `ftCommonLandingSetStatus` passes
`anim_speed` 1.0 for a light landing and **0.5** for a heavy one, so the same
7-frame animation takes 14 frames after a fastfall. Storing a length without
the speed would have made both landings identical. `FTCOMMON_LANDING_INTERRUPT_
BEGIN` is 4.0, which is why Mario's landing lag is commonly quoted as 4 frames
while the animation is 7 — the last three are interruptible.

**Confidence: high.** Two independent readings of the same bytes agree on all
189 values, every file's joints agree internally, and the loop/finite split
matches the status machine's own structure without being told to. Confirmed
on-device: the overlay reads `anim dash 23f  land 7f` for Mario, out of the
pack, at 60.0 FPS.

---

## RE-036 — The figatree's tracks, and the mask that says which joints exist

**Question.** RE-035 walked figatree scripts far enough to total their
durations. That is the smallest useful thing the format holds. Reading the rest
— the per-joint transform tracks — needs three things it did not answer: what a
command's value words *mean*, how those values are interpolated between keys,
and which joint of which model each script belongs to.

**The tracks.** A command's `flags` field is a bitmask over ten tracks,
`RotX RotY RotZ TraI TraX TraY TraZ ScaX ScaY ScaZ`, and its trailing value
words are read one or two per *set bit, in bit order* — not indexed by track.
`ftAnimGetTargetValue` scales the raw `s16` by track group, and values and
rates do not share a table:

| group | value | rate |
|---|---|---|
| rotation | 1/512 | 1/512 |
| translation | 1/4 | 1/32 |
| scale | 1/4096 | 1/8192 |
| `TraI` | 1/16384 − 3e-12 | 1/16384 − 3e-12 |

Rotations come out in radians. Translations come out in the same large world
units as everything else (RE-032): Mario's dash sets his root joint's Y to
`755/4 = 188.75` against a rest height of 150.

**The interpolation.** Each track carries an `AObj` — base and target value,
base and target rate, a running length, and the reciprocal of its duration —
and a command *rewrites* the tracks it names, pushing the old target down to
the base. The pose is then read back by evaluating a cubic Hermite (or a line,
or a step) at the track's current length.

That indirection is the point of the format. `anim_wait`, the clock, and each
track's duration are separate numbers, so one command can hold the clock up for
11 frames while a track set earlier is still interpolating over 26. A decoder
that treated each command as a keyframe covering the time until the next
command would be wrong for most of Mario's dash.

**Which joint a script belongs to.** `lbCommonAddFighterPartsFigatree` walks
the fighter's `DObj` tree and the pointer table together:

```c
lbCommonAddFighterPartsFigatree(fp->joints[nFTPartsJointTopN]->child, fp->figatree, frame_begin);
```

so script *n* belongs to the *n*-th `DObj` in pre-order from `TopN`'s child.
The obvious reading — that this is the *n*-th entry of the model's `DObjDesc`
array — is wrong, and the counts say so: Mario's model has 25 descriptors and
his animations have 24 scripts. Every fighter was off, most by one.

The missing hop is `setup_parts`, a pointer in `FTAttributes` to two `u32`s
that `lbCommonSetupFighterPartsDObjs` walks alongside the descriptor array:

```c
for (i = 0; ((flags0 != 0) || (flags1 != 0)) && (dobjdesc->id != DOBJ_ARRAY_MAX); i++) {
    current_flags = (i < NBITS(u32)) ? flags0 : flags1;
    if (current_flags & (1 << 31)) { ... gcAddChildForDObj(...) ... }
    dobjdesc++;
    if (i < NBITS(u32)) flags0 <<= 1; else flags1 <<= 1;
}
```

A cleared bit means the descriptor never becomes a joint. So a fighter's joint
count is the mask's population count, and animation script *n* belongs to the
*n*-th **set** bit. The bits are read most significant first, which is the
opposite of how a bitmask usually reads; taking the words as plain little-endian
masks reverses every joint in the fighter.

**Offsets, counted rather than searched.** `setup_parts` is at `FTAttributes +
0x29C` and `animlock` at `+0x2A0`, counted back from `unused_0x2CC` — the one
field the decompilation names after its own offset. Counting *forward* from the
same anchor puts `commonparts_container` at `+0x2D4`, and the arithmetic
lands exactly on the next self-naming field, `filler_0x30C`. Two independent
anchors, one on each side.

`commonparts_container` matters as much as the mask. It names the fighter's
skeleton outright:

```c
struct FTCommonPartContainer { FTCommonPart commonparts[2]; };   // high, low detail
struct FTCommonPart { DObjDesc *dobjdesc; MObjSub ***p_mobjsubs; ... };
```

Reaching it is an intern relocation to the container and then an extern
relocation into the model file — two archive records, no shape-matching.
Picking the biggest graph a fighter's `*Main` file points at instead gets Mario
wrong: he and Luigi share a 26-node graph that is not either one's body.

**The check.** `romtool figatree` resolves every fighter's skeleton this way
and compares its joint count against all seven of that fighter's movement
animations, then plays each script for 40 frames.

* **170 of 189 animations have exactly as many scripts as their fighter has
  joints.** The other 19 have exactly one spare, and the rule below accounts
  for every one of them.
* **No script desynchronised** — roughly 4,000 scripts played to their
  terminator without a command's word count going wrong.
* Mario's dash resolves to 18 scripts across 24 slots, and the six null slots
  fall at indices 3, 9, 14, 17, 19 and 22 — *exactly* where the
  decompilation's own transcription of that table puts its `NULL`s.
* Mario's joint 1 translates 31.0 units in Z at its peak. The decompilation's
  source for that script reads `ftAnimSetVal0RateBlockT(FT_ANIM_TRAX |
  FT_ANIM_TRAZ, 4), -16, 124` — and `124/4 = 31.0`. The scale factor, the
  bit-order of the value words and the track assignment are all confirmed by
  one number.

**The spare script is `TransN`, with no exceptions.** The 19 are Kirby,
Jigglypuff and their polygon variants in Squat, SquatRv and Pass (three each),
and Master Hand in all seven. Those, and *only* those, are the motions whose
`FTMotionDesc` carries `FTANIM_FLAG_TRANSN_JOINT` — Mario's equivalents carry
`FTANIM_FLAG_NONE`, and Master Hand's entire table is `TRANSN`. The flag puts
`TransN`, a runtime joint rather than a model one, in the chain as `TopN`'s
child, so it takes script 0 and pushes the model's joints down by one. The law
is therefore exact rather than an inequality:

```
scripts == popcount(setup_parts) + (motion uses TransN ? 1 : 0)
```

None of the seven movement animations of any fighter in the current vertical
slice uses the flag, so the current mapping needs no special case; a fighter
that does will need one, and `romtool figatree` will say which.

Worth recording because it nearly went the other way: the polygon-model
variants looked at first like they shared the full character's animations while
having fewer joints — NSamus appeared to have 16 joints against Samus's 23
scripts. That was the *graph* being chosen by size rather than by record.
Read through `commonparts_container`, NSamus has 23 joints and matches exactly.
A plausible-looking explanation had been waiting for the wrong data.

**Two things deliberately left alone.** `TraI` decodes but is not applied: it
needs the spline control points that only opcode 12 supplies, and no fighter
figatree in the ROM contains an opcode 12. And `FTAttributes.translate_scales`
(`+0x324`) makes `ftParamUpdateAnimKeys` scale a joint's animated translation
per-joint; the fighters that have it are not yet identified.

**Also confirmed, incidentally.** Jigglypuff's SquatRv really is
`FTKirbyAnimCrouchEnd` — she has no `CrouchEnd` file of her own, and
`dFTPurinMotionDescs` names Kirby's outright, alongside Kirby's walk-end,
crouch-idle and entire damage set. RE-035's animation table was right about a
pairing that looks like a transcription slip.

**Implementation.** `crates/ssb-rom/src/figatree.rs` (the decoder and the
`AObj` state machine), `crates/ssb-rom/src/fighter.rs` (`setup_parts`,
`animlock`, `common_parts`). `anim.rs`'s length walk now runs on the same
decoder, so RE-035's 189 verified lengths are a test of *this* code's word
counts rather than of a second copy of them.

**Carried into the pack.** All of the above is build-time work, and none of it
should have to happen again on a PSP. Pack version 6 stores the answer: an
`AnimDesc` per `(fighter, slot)` and an `AnimJoint` per joint holding the
script's byte offset and the **absolute pack node** it drives. The runtime is
handed a script and a node; it never sees `setup_parts`, `FTCommonPart` or an
archive file id. `NodeDesc` gained the node's local rest transform for the same
reason — an animation overwrites only the tracks it names, and the tracks it
does not name have to start somewhere. A baked world matrix cannot supply that:
decomposing one back into a rotation and a scale is lossy.

Animation files are deduplicated by archive file id, which matters because
sharing is common — Jigglypuff borrows three of Kirby's outright and every
polygon variant shares all seven with the character it copies. 189 animations
and 4709 joint entries cost 342 KiB.

**The check that the tables are right.** `romtool figatree --pack` plays every
animation twice: once from the pack's tables, once by re-deriving the whole
chain from the ROM. **3444 joints, 64 frames each, every pose identical.** That
is what makes the stored pairing trustworthy rather than merely plausible — and
it caught a real fault immediately, the new tables having been written *after*
the blob-alignment padding, so the reader found zeros where the animation table
should have been. "Loads back cleanly" did not notice, because the sizes were
self-consistent; replaying the data did.

**Confidence: high** for the format, the scales and the joint mapping — a
number-for-number match against the decompilation's own transcription on the
one file examined in detail, no desynchronisation across ~4,000 scripts, and
joint counts that agree with an independently recovered mask for all 189
animations under a rule with no exceptions. **Not yet validated on device**: nothing renders these poses yet.

---

## RE-037 — Why the stages draw white: the textures are in another file

**Question.** Fighter models texture correctly on device. Dream Land does not —
its geometry lands in the right place, its collision lines sit exactly on its
platforms, and every surface renders white. Since fighters and stages go
through the same converter, the same pack format and the same draw path, the
difference had to be in the data rather than the code.

**Evidence.** `romtool textures --file 104` — Dream Land's geometry file —
reports **1 of 2 textures packed**. Two is not a plausible texture count for a
stage with a tree, three platforms and a background. Link's model file binds 22.

`romtool dump 104` says what the missing ones are:

```
intern relocs  114
extern relocs  57
depends on:
  file 103   (57 pointer(s))
```

**Fifty-seven cross-file pointers, every one into file 103.** The archive
records extern relocations rather than applying them — the target address
depends on runtime layout — so those slots read as zero. `mesh::convert` sees a
`G_SETTIMG` address of 0, cannot resolve it, and the primitive comes out
untextured. The renderer then correctly disables texturing for it.

**The reported failure count understates it.** `romtool textures` deduplicates
on `(file id, data_offset)`, and every unresolved cross-file texture has
`data_offset == 0`. All of a file's missing textures therefore collapse into
one entry. The headline "54 null (extern reloc, texture in another file)" is
54 *files*, not 54 textures.

**Why this is a data-plumbing gap and not a decoder bug.** Nothing about the
texture format is in question: the same decoder packs 482 textures, including
every fighter's, and RE-022 validated the swizzle, CLUT upload and UV scale on
device. What is missing is the hop from a display list in one archive file to
texel data in another. `TextureRef` can only name an offset, not a file, so
there is nowhere to put the answer even once it is known.

**The fix, scoped.** `G_SETTIMG`'s operand slot needs to be looked up in the
file's `extern_relocs` when it reads zero, which means the display-list decoder
has to carry each command's byte offset, `TextureRef` has to carry a file id
alongside its offset, and the converter has to be handed the whole archive
rather than one file's bytes. The palette path needs the same, since a TLUT
load reads whatever image address is current.

**Implementation.** `Cmd::SetTimg` now carries the file-relative offset of its
own address word, filled in by `dl::decode_list_at`. `mesh::Source` pairs a
file's bytes with its extern relocations, and the walk looks a zero address up
by that slot; `TextureRef` grew a `data_file` and a `palette_file` so the
answer has somewhere to live. The two halves are resolved independently,
because they need not be in the same file — a fighter's palette is in its own
file while a stage's texels are in a shared one.

**Result.** Dream Land's geometry file goes from **1 of 2 textures packed to
16 of 19**, and archive-wide from 482 to 545. The pack gains 68 textures for
100 KiB. Confirmed on device: the stage that rendered as a white silhouette
now renders as Dream Land, tree and platforms textured, at 60.0 FPS and 796 us
CPU (`docs/images/m4-stage-textured.png`).

**What the same count also says.** Bound references rose from 586 to 664, and
the 54 "null" entries did not move. Those are genuinely unresolved: an address
of zero with no relocation naming the slot. They are a separate question from
this one and are still open, along with 13 segmented addresses and 36
references whose resolved offset lands past the end of the file they name.

**Confidence: high.** The diagnosis, the fix and the on-device result agree,
and three host tests pin the behaviour that has no ROM in CI: that a slot with
a relocation resolves to the named file, that one without still refuses to
sample offset zero, and that a relocation for a *neighbouring* slot does not
satisfy this one — which would have given every stage some other stage's
textures.

---

## RE-038 — The pose was right; the way I was looking at it was not

**Question.** The animation pipeline ran end to end on device and the result
looked wrong: Mario's parts appeared scattered, a foot reached `z = -134` on a
model whose entire rest pose sits within 5 units of the Z plane, and both
ankles sat 89 units off the ground mid-dash. This entry originally recorded
that as a defect. It was not one.

**Three ways of looking that were each wrong.**

1. *An arbitrary view angle.* The viewer advances `spin` every tick
   unconditionally, so captures taken seconds apart differ by most of a turn.
   Three screenshots being compared as though the difference were the pose were
   in fact three different rotations of it.
2. *An expectation about the model's facing.* Mario is authored facing **+Z** —
   his shoulders span X, and the one descriptor `setup_parts` excludes sits at
   `z = +120`, in front of him. So a leg swinging through 130 units of Z is a
   leg swinging *forward*. Rendered at rest with the angle frozen, he stands
   facing the camera exactly as he should.
3. *Judging a pose by eye at all*, on a low-polygon model most of whose
   materials do not convert yet, framed by a camera fitted to the rest bounds
   while the pose had moved out of them.

**What settles it instead.** Two checks that need no opinion about what a pose
should look like.

*The feet.* A fighter's origin is at its feet (RE-032), and in a grounded
animation they stay on the ground. Mario's foot nodes sit at `y = 8.3` at rest:

```
Turn     7  7  4  3  9  6  2  1  5  4  6  9
Squat    7  8  8  9  9  9  9  7  8  8  8  8
Landing  9  5  9  9  8  8  9 10 10 10 10 10
Dash    34 17 17 14 11 10 11 14 26 40 57 75 97 91 74 54 40 36 32 28 23 20
```

Planted through all three static poses. The one that moves is the dash, in a
single arc up and back down — a stride.

*The bones.* A skeleton poses by rotating joints, so a node's distance from its
parent is fixed. Two things may break that legitimately: animating a node's own
translation, and animating the **scale** of anything above it, since a parent's
scale multiplies its children's offsets. That second one is not hypothetical —
it is how Kirby and Jigglypuff squash, and excluding only translation left 28
false positives that were all theirs and Pikachu's.

With both exclusions: **204,547 bone lengths across all 189 animations, worst
change 0.009 units** on a 300-unit fighter. That is float rounding through a
chain of matrix products, not motion. The skeleton is rigid everywhere.

**What the search did turn up.** Chasing this ruled out, by measurement rather
than by reading, the rotation order (`from_trs` matches `syMatrixRotRpyRF` term
for term, and the `PyrR` alternative the original also ships renders visibly
worse), the track-to-axis mapping, the `setup_parts` bit order at the raw-word
level, and sibling ordering (`gcAddChildForDObj` appends, so a tree walk is
array order; prepending would have swapped every branch point). The rotation
scale of `1/512` is positively confirmed rather than merely assumed: Mario's
Turn rotates his root joint 2.99 radians over its twelve frames, which is the
half-turn the animation is named for.

And it found one real fault. RE-036 predicted that the 19 animations flagged
`FTANIM_FLAG_TRANSN_JOINT` carry one extra script, because `TransN` — a runtime
joint, not a model one — is spliced in as `TopN`'s child and the attach walk
reaches it first. The packer was ignoring that, so for Kirby, Jigglypuff, their
polygon variants and Master Hand, **every joint's rotation was landing on its
neighbour**. The count is derivable without new transcription: RE-036 proved
`scripts == popcount(setup_parts) + (TransN ? 1 : 0)` holds with no exceptions,
so one script more than joints *means* TransN. The packer now shifts those, and
reports `19 using TransN` — the exact number predicted.

That fault is worth noting for what it says about the bone-length check: it
would never have caught it. Any assignment of rotations to nodes keeps a
skeleton rigid. Invariants bound the search; they do not close it.

**Confidence: high.** The composed poses match the ROM exactly (RE-036's replay
check, 3444 joints), the skeleton is rigid across 204,547 bone measurements,
the feet behave, and Turn's opening frame renders as a standing Mario
(`docs/images/m4-animation.png`). **Still open:** the viewer frames its camera
on the rest bounds, so a pose that moves drifts out of shot, and most of
Mario's materials do not convert yet (RE-037's remaining 119) — which is why
the model is grey.

---

## RE-039 — Mario is grey because his colour is not in his model

**Question.** Mario's model rendered almost entirely white. His torso and cap
showed texture, everything else was a grey blob. He is supposed to be red and
blue.

**His vertices carry shade, not colour.** Every vertex in his model is a pure
grey — `0xfcfcfc`, `0xd4d4d4`, `0x999999`. Those are not colours the exporter
got wrong; they are the N64's *shade* term, and the colour is supposed to come
from the combiner's other input:

```
G_SETCOMBINE 0x0032_7e05 0xff17_fdff   ->  (PRIM - 0) * SHADE + 0
```

The pack stored `prim_color` per primitive from the beginning and the renderer
never read it, so every flat-shaded part of every fighter drew as bare shade.
Multiplying the two at conversion time costs nothing at runtime and needs no
second colour source in the vertex format — and the existing vertex dedup
splits a vertex shared by two primitives of different colours by itself,
because the folded colour is part of its key.

**Not every part uses it, and that matters.** Mario's model sets three
combiners:

| nodes | cycle 0 | what it is |
|---|---|---|
| 2, 8 | `TEXEL0 * SHADE` | torso and head, textured |
| 4, 5, 10, 11, 15, 16, 20, 21 | `PRIM * SHADE` | upper arms and thighs |
| 6, 12, 18, 23 | `SHADE` alone | gloves and shoes |

Folding the primitive colour in unconditionally turned his white gloves green,
because the last colour set was still in force even though the combiner ignores
it. So the conversion now decodes `G_SETCOMBINE` and only folds when
`PRIMITIVE` appears in cycle 0's colour equation — in any of A, B, C or D,
which are four different widths at four different shifts.

**Why the colours that remain are still wrong.** With that in place Mario has a
red cap, a blue-and-red torso, white gloves and grey shoes — and green upper
arms and orange thighs. Those two come from `MObjSub::primcolor`, and the raw
bytes really do say so:

```
MObjSub @0x190 (Mario's upper arm)
  +30: 02 00 ...            flags 0x0200, MOBJ_FLAG_PRIMCOLOR set
  +50: 00 ce 00 ff          primcolor = (0, 206, 0, 255)
```

Luigi's equivalent is `(0, 181, 0)`. Both green, differing only in one channel,
which is the tell: **the baked value is a placeholder.** The real colour is
per-costume and arrives from a pointer this project has never read — the third
one in `FTCommonPart`, alongside the two RE-027 recovered:

```c
struct FTCommonPart {
    DObjDesc *dobjdesc;                       // RE-027
    MObjSub ***p_mobjsubs;                    // RE-027
    AObjEvent32 ***p_costume_matanim_joints;  // this
    u8 flags;
};
```

`lbCommonAddMObjForFighterPartsDObj` attaches it, evaluates it, and throws the
`AObj`s away again — so it is a one-shot overwrite of the `MObjSub`'s baked
colour, not an animation that runs:

```c
gcAddMObjMatAnimJoint(mobj, costume_matanim_joint, anim_frame);
gcParseMObjMatAnimJoint(mobj);
gcPlayMObjMatAnim(mobj);
gcRemoveAObjFromMObj(mobj);
```

And `anim_frame` there is `fp->costume`. **One script per joint holds every
costume, one per frame** — evaluate it at frame 0 for Mario's default red,
frame 1 for his green alternate, and so on. Mario's is at file 296 offset 9856;
Luigi's at 323:10384; Kirby's at 328:6432. Every fighter has one.

Reading it needs the **32-bit** `AObjEvent32` encoding rather than the 16-bit
figatree one RE-036 ported — `{ opcode:7, flags:10, payload:15 }`, a different
opcode set, and the material track range (`nGCAnimTrackPrimColor` = 37 onward)
instead of the ten joint tracks.

**Confidence: high** for the diagnosis and for both fixes, which are unit-tested
against Mario's own three combiner words. The remaining colours are a known
gap with a named source, not a mystery. Screenshot:
`docs/images/m4-fighter-colours.png`.

---

## RE-040 — A fighter's colours are a script, one costume per frame

**Question.** RE-039 got Mario's flat-shaded parts coloured but wearing the
wrong ones: green upper arms, orange thighs. The values came from
`MObjSub::primcolor` and the raw bytes really did say green, so the question
was where the game gets red from.

**The record's third pointer.** `FTCommonPart` has three, and RE-027 recovered
the first two:

```c
struct FTCommonPart {
    DObjDesc *dobjdesc;                       // RE-027
    MObjSub ***p_mobjsubs;                    // RE-027
    AObjEvent32 ***p_costume_matanim_joints;  // this
    u8 flags;
};
```

`lbCommonAddMObjForFighterPartsDObj` attaches the third, evaluates it, and
throws the `AObj`s away again — so it is a one-shot overwrite of the baked
colour rather than an animation that runs:

```c
gcAddMObjMatAnimJoint(mobj, costume_matanim_joint, anim_frame);
gcParseMObjMatAnimJoint(mobj);
gcPlayMObjMatAnim(mobj);
gcRemoveAObjFromMObj(mobj);
```

**And `anim_frame` is `fp->costume`.** That is the whole idea: one script per
material holds *every* costume, one per frame. Mario's upper arm, at file 296
offset `0x2744`:

```
24008000  ff0000ff    SetExtValAfterBlock(PrimColor, 0)   costume 0 — red
24008001  ffe700ff    SetExtValAfterBlock(PrimColor, 1)   costume 1 — yellow
24008001  f7e78cff    SetExtValAfterBlock(PrimColor, 1)   costume 2
24008001  5242ffff    SetExtValAfterBlock(PrimColor, 1)   costume 3 — blue
26008001  00ce00ff    SetExtValAfter(PrimColor, 1)        costume 4 — green
04000061              Wait(97)
00000000              End
```

The green that was reaching his sleeves is the **last** entry. `MObjSub`'s
baked colour is simply whatever the exporter left there, and it is the final
costume — which is why Luigi's baked value `(0, 181, 0)` looked nearly right
while Mario's looked absurd. Both are alternates; Luigi's alternate happens to
be green too.

**The encoding is not the figatree one.** `AObjEvent32` is a single `u32` per
command:

```text
bits  31..25   24..15   14..0
      opcode   flags    payload
```

Both the track mask and the duration are in the command word — there is no
`toggle` word — and each set track is followed by one `u32` of value. For a
colour track those four bytes *are* the colour: `gcPlayMObjMatAnim` reads
`*(SYColorPack*)&aobj->value_target` rather than converting a float. Opcode 18
is `SetExtValAfterBlock`, 19 `SetExtValAfter`, and `nGCAnimKindStep` selects
`value_target` once `length_invert <= length`, which at frame *n* is the *n*th
entry.

**Implementation.** `crates/ssb-rom/src/matanim.rs`. Opcodes the decoder does
not model are an error rather than a skip: guessing a word count
desynchronises the stream, and a colour read from a desynchronised stream still
looks like a colour. The costume list is chosen at pack time —
`DEFAULT_COSTUME = 0`, the one the character select opens on.

**Result.** Mario renders in red and blue: red cap and sleeves, blue overalls,
white gloves, textured torso (`docs/images/m4-fighter-colours.png`). Nothing
about the animation checks moved — poses still match the ROM across 3444
joints and no bone stretches.

**Confidence: high.** The decode is pinned by a unit test holding Mario's arm
script verbatim and asserting all five costumes, and the result is the colour
the character is known to be. **Open:** only costume 0 is packed, so the
alternate palettes are unreachable until a match can choose one; the material
tracks other than the three colours (texture ids, UV scroll, palette id) are
decoded far enough to step over and no further.

---

## RE-041 — The other thirteen movement animations

**Question.** RE-035 recovered seven animations per fighter: the statuses that
*end when their animation runs out*, because those were the ones the status
machine needed a length for. The status machine has nineteen states. A fighter
that animates only while dashing or crouching spends a match in a rest pose.

**The same three-record chain, thirteen more times.** Nothing new was needed —
`dFTCommonActionStatusDescs[status - 6].mflags.motion_id` into
`dFT<Name>MotionDescs[motion_id]` into a relocData file, exactly as before. The
work was in the *checks*, which is where the interest is.

**Two systematic name mismatches, and why neither is a fault.**
`tools/gen-anim-table.py` verifies the resolved animation's *name* against the
slot, so a motion table parsed one entry out of step reports itself. Two slots
tripped it for almost the whole roster:

* **Wait resolved to `EggLay`** for everyone but Mario. `nFTCommonMotionWait` is
  4, and Mario's motion 4 is `FTMarioAnimWait` — the index is right by
  construction. Fox's motion 4 is a file symbol-named `FTFoxAnimEggLay`. That
  is a naming quirk, not a shift: a shift would misname every *later* slot too,
  and every fighter's seven length-bearing slots still verify against the ROM
  byte for byte. Pikachu's is `Idle`.
* **KneeBend resolved to `LandingAirX`** for the whole roster. Jumpsquat and
  landing are the same knees-bent pose and most of the cast shares one file
  between them — the mirror of the Jigglypuff case RE-035 recorded from the
  other direction, where her *landing* is named `JumpSquat`.

Both are now allowed by name, with that reasoning written where the check is.

**Absence is data.** Kirby and Jigglypuff have no aerial jump, and their motion
slots 18 and 19 are the `{ 0, 0x80000000, 0 }` placeholders RE-035 found while
chasing a different bug. A null motion is now recorded as "no animation" — file
id 0, the runtime keeps the rest pose — rather than failing the build. Yoshi
has one aerial-jump animation and uses it for both directions.

**Lengths only mean something for seven of the twenty.** Wait, the three walks,
Run, Fall, FallAerial and SquatWait all contain a `Loop` command and never
terminate, which is exactly right: they are the statuses that leave by being
interrupted. The verification against the decompilation still covers the seven
timed slots, and still agrees on all **189**.

**Result.** 532 animations in the pack against 189 — some fighters lack a
couple — for 342 KiB more. Everything the animation pipeline is checked by
scales with it: **9622 joints replayed from the pack all match the ROM exactly,
and none of 565,646 bone lengths changes by more than 0.064 units.** Mario's
idle plays on device (`docs/images/m4-animation.png`).

**Confidence: high.** The mapping is the same one RE-035 validated, the name
check is stricter than it was rather than looser — two exceptions argued rather
than waived — and the pack-versus-ROM replay covers every new animation.

---

## RE-042 — The status machine drives the animation

**Question.** RE-041 put every movement status's animation in the pack and
RE-036 to RE-038 made them play. Nothing connected the two: the viewer could
browse animations and the simulation could move a fighter, but a fighter on a
stage was a sliding rest pose.

**Where the join goes.** `ssb-game` must not know the pack format, and starting
an animation needs it, so the split follows the one the physics constants
already use: the *mapping* is game logic and lives in `Status::anim_slot`, and
the *skeleton* lives in `psp::play::Play` next to the fighter it belongs to.
`ssb_rom::anim`'s slot numbering is repeated in Layer A rather than imported,
and a test pins the two together — a reordered generator would otherwise have a
fighter walking with a crouch animation.

**Restarted on change, not on tick.** An animation carries its own clock, so
re-seeding it every frame freezes every fighter on frame zero. `Play` remembers
the status the skeleton was started for and only calls `Skeleton::start` when it
differs. A looping animation is left to loop; a finite one that has run out
holds its last pose, which is what the original does when a status outlives its
animation.

**The speed comes from the status.** `ftCommonLandingSetStatus` passes 1.0 for
a light landing and 0.5 for a heavy one — the same seven-frame file taking
fourteen frames after a fastfall (RE-035). `Status::anim_speed` is that, and it
is read at start time rather than baked into the pack, because it is a property
of how the status was entered rather than of the animation.

**A quarter turn.** Fighter models are authored facing `+Z`, shoulders spanning
X (RE-038), while a match runs along X. So the model is turned ±90 degrees by
the fighter's facing — the first place that fact has had to be acted on rather
than merely understood.

**Result.** Mario stands on Dream Land in his own colours, animated by whatever
status the ported machine has him in, at 60 FPS: 495 triangles and 52 draws for
stage and fighter together, 1319 us CPU against a 16667 us budget
(`docs/images/m4-fighter-status.png`).

**Confidence: high** for the wiring, which is small and directly observable.
**Untested:** every status *transition* has an animation but only walking,
dashing, jumping and landing have been watched happen; and the heavy-landing
half-speed path has not been distinguished from the light one by eye.

---

## RE-043 — What is still white, measured

Two things still render white or grey. Neither is a mystery, but neither is
fixed, and this records what they are so the next attempt starts from
measurements rather than from a screenshot.

**Mario's gloves, shoes and face.** They are not missing a colour; their
combiner does not read one. `G_SETCOMBINE 0xfcfffe05 0xff167dff` on nodes 6,
12, 18 and 23 is `SHADE` in cycle 0 and `COMBINED * ENV` in cycle 1, so the
final colour is `SHADE * ENV` with no primitive colour anywhere in it. RE-039's
gate is therefore doing the right thing by leaving them alone — the question is
what `ENV` is. The `MObjSub` field says `(0, 0, 0, 255)`, which would render
them black rather than white, and **the costume lists set no environment
colour at all**: of Mario's materials, every one returns `env: None` from
`matanim::colors_at`. So either the env colour arrives from somewhere not yet
read, or `SHADE * ENV` is not the right reading of that combiner word.

Worth noting because it points at the second possibility: the costume list
*does* set a primitive colour for node 18 — `(0, 0, 247)`, the blue of his
overalls — on a material whose combiner, as decoded, ignores primitive colour
entirely. Data that exists to be unused is usually a sign the decode is wrong,
not the data. **The two-cycle combiner is only half-decoded: cycle 1 is read for
this note but not modelled anywhere in the converter.**

**Dream Land's white.** Not a stage-wide problem. Of the four layers' ~100
primitives, exactly four have no texture:

```
layer 0  node  1    2 tris   untextured, all 4 vertices grey
layer 2  node  1    5 tris   untextured, all 7 vertices grey
layer 2  node  2    6 tris   untextured, all 8 vertices grey
layer 2  node  3    6 tris   untextured, all 8 vertices grey
```

Everything else in the stage is textured and draws correctly. Those four have
grey vertices, no texture and no primitive colour, so they come out white — and
two triangles is a large quad, which is why a handful of primitives covers a lot
of screen. Layer 2 is the one the stage record names a material table for, so
the colours are most likely in its `MObj` chain and being dropped for the same
reason Mario's are: a combiner whose second cycle is not modelled.

**The hypothesis was wrong, and implementing it is what showed that.** The
converter now evaluates *both* combiner cycles rather than asking one question
of cycle 0. Each input resolves to
`k + s*SHADE + t*TEXEL + st*SHADE*TEXEL` per channel, cycle 1 takes cycle 0's
result as its `COMBINED` input, and a result that is not a plain scale on the
shade is declined rather than approximated. Mario's three combiner words are in
the tests, along with the two-cycle case, the additive case that must be
refused, and the rule that an unset constant reads as white rather than black —
a combiner reading a colour nothing set is reading whatever the RDP had, and
white is the only choice that cannot darken geometry that should be lit.

**The render is unchanged.** So cycle 1 was never the answer: either these lists
run in one-cycle mode, or their `COMBINED * ENV` multiplies by an environment
colour nothing ever sets. Either way the combiner really does say `SHADE`, and
those surfaces are white because that is what the hardware would draw.

That leaves the actual cause where RE-037 left it. Dream Land's geometry file
binds 19 textures and packs 16, and its four untextured primitives are three
short — they are meant to be textured and the conversion is still failing on
them. Archive-wide 119 of 664 references fail: 54 null pointers nothing
resolves, 36 whose resolved offset lands past the end of the file they name, 28
paletted without a recorded TLUT, 13 segmented. Mario's gloves, by contrast,
are *correct* — they are white in the game too.

**Confidence: high.** The measurements are reproducible with
`romtool textures --file 104` and the draw dump, and the combiner is now
implemented rather than hypothesised — which is what turned "one hypothesis
short of proven" into a ruled-out cause and a named remaining one.

---

## RE-044 — A tile's size is not the texture's size

**Question.** RE-043 narrowed the last white surfaces to texture conversion:
Dream Land bound 19 textures and packed 16, and 119 of the archive's 664
references failed. The largest single class was 36 whose data ran past the end
of the file holding it — which looked like a resolution bug, since an offset
that lands outside its own file is not a plausible thing for the game to ship.

**It was not the offset.** Printing the failures with their dimensions:

```
file 103 @0x000e20  256x128  Ci/Bits4  need 16384  end 20000  len 12224  <<<
file 103 @0x001880  192x96   Ci/Bits4  need  9216  end 15488  len 12224  <<<
```

A 256x128 CI4 texture needs 16 KiB out of a 12 KiB file. The offsets are fine;
the **dimensions** are wrong, and by enough that the texture is larger than
everything around it.

**`G_SETTILESIZE` is the rectangle being drawn.** The converter was taking it
as the texture's extent. For a texture that *wraps*, the drawn rectangle is
larger — often much larger — than the texture: Dream Land renders a 64x32 tile
across a 256x128 span of ground. What says how big the texture actually is are
`masks` and `maskt` in `G_SETTILE`, because the RDP repeats it every
`1 << mask` texels. A mask of zero means it does not wrap, and then the drawn
rectangle *is* the texture.

Taking `min(drawn, 1 << mask)` per axis is the whole fix.

**Result.** Every one of the 36 goes away — not most of them, all of them,
which is what a correct reading of a field looks like against a guess that
happens to help. Archive-wide **545 packed of 664 rises to 581**; Dream Land
goes from 16 of 19 to 18, and its ground draws textured instead of white.

**And the VRAM problem was the same bug.** Textures had been converted at their
*drawn* size all along, inflating every wrapping one by the area it covered:

```
before   packed 1077.9 KiB   1.5x over the ~700 KiB budget — needs streaming
after    packed  607.4 KiB   fits, all at once
```

The pack shrank from 3711 KiB to 3062 KiB and draw calls fell from 4302 to
3739, because textures that are the same size now merge where before they
differed. The per-scene texture residency RE-022 recorded as a requirement is
not one; it was an artefact of measuring the wrong number.

**Confidence: high.** The rule is the RDP's own, the failure class it addresses
goes to zero rather than down, and the on-device result is the surface drawing
its texture (`docs/images/m4-fighter-status.png`).

**Still failing: 83.** 54 null pointers nothing resolves, 28 paletted without a
recorded TLUT, 16 `MissingPalette` at decode, 13 segmented addresses — the
palette-tracking cases are now the largest group.

---

## RE-045 — An `MObj`'s texture is emitted twice under different flags

**Question.** RE-044 fixed 36 of the 119 failing texture references. Dream
Land's ground was not among them: layer 2's nodes 2 and 3 still drew pure
white.

**They ask the material for their texture.** Their display lists are almost
nothing:

```
node 2  Combine  SetTile0  SETTIMG(0x0)  Texture(true)  Call(0x0E000000)
node 3           SetTile0  SETTIMG(0x0)  Texture(true)  Call(0x0E000000)
```

A `G_SETTIMG` of zero and a call into the graphics heap — so the texture is
whatever the `MObj` puts there, exactly the arrangement RE-027 recovered for
fighters' palettes. And the `MObj` read back as contributing nothing at all:
no sprite, no palette, no colour.

**`gcDrawMObjForDObj` emits the texture image twice.** Under two different
guards, for two different purposes:

```c
if (flags & (MOBJ_FLAG_FRAC | MOBJ_FLAG_SPLIT))   // stages the *next* texels
    gDPSetTextureImage(.., mobj->sub.sprites[mobj->texture_id_next]);
...
if (flags & (MOBJ_FLAG_FRAC | MOBJ_FLAG_ALPHA))   // the one actually sampled
    gDPSetTextureImage(.., mobj->sub.sprites[mobj->texture_id_curr]);
```

`mobj.rs` was reading the first guard — the block-load one — and so missed
every material that simply names a texture without animating between two.
`MOBJ_FLAG_ALPHA` is bit 0 and common, which is why the miss was large.

Both indices are zero in a static read, so accepting any of the three flags
reads the same address either guard would.

**Result.** Bound references rise from 664 to **695** and packed from 581 to
**612** — 31 textures that were never being looked for. Failures stay at 83,
because these were not failing conversions; they were materials whose texture
was not being read at all.

**It did not fix Dream Land's ground.** They are in the 54 "null pointer,
nothing resolves it" class, and that class is now the whole of the remaining
visible problem rather than one of several candidates.

**Confidence: high** for the flag reading, which is the decompilation's own and
adds 31 textures without moving the failure count. **The white ground is not
fixed**, and its cause is now narrowed to a single question: what a
`G_SETTIMG` of zero, with no relocation and no `MObj` sprite, is supposed to
resolve to.

> **Correction (RE-046).** This entry originally explained the surviving white
> by asserting that those two `MObj`s "have none of the three flags set, so
> they genuinely supply no sprite". That is false. Their `MObjSub.flags` is
> `0x006B`, which includes both `ALPHA` and `SPLIT`. The claim came from
> reading the *converter's* output — `sprite: None` — and reporting it as a
> fact about the ROM, when it was a fact about the bug. The real cause was one
> level further down and is in RE-046. The framing "narrowed to a single
> question" was therefore confidently wrong about which question.

## RE-046 — A material's sprite table can leave the file too

**Question.** After RE-045, Dream Land's ground still drew white, and the
texture report still said 54 references were "null pointer, nothing resolves
it". What is a `G_SETTIMG(0)` with no relocation and no `MObj` sprite supposed
to resolve to?

**The premise was wrong.** The `MObj`s do have sprites. Read straight out of
the file, Dream Land's two ground materials are:

```
node 2 mobjsub @0x1F78: flags=0x006B sprites=0x00001F60 palettes=0x00000000
node 3 mobjsub @0x1FF0: flags=0x006B sprites=0x00001F6C palettes=0x00000000
```

`0x6B` includes `ALPHA` and `SPLIT`, so RE-045's gate passes and the sprite
table is read. Every entry in it reads back zero:

```
sprites[0..3] = 0x0, 0x0, 0x0
```

**They are zero because they leave the file.** The archive blanks a pointer
that targets another file and records an extern relocation instead. File 104's
relocation list has one for every entry of both tables:

```
EXT 0x001F60 -> file 103 +0x1BE0     node 2 sprites[0]
EXT 0x001F64 -> file 103 +0x1E10
EXT 0x001F68 -> file 103 +0x2040
EXT 0x001F6C -> file 103 +0x1BE0     node 3 sprites[0]
```

This is exactly RE-037 — a stage's texels living in a separate archive file —
one level deeper than RE-037 looked. RE-037 taught `G_SETTIMG` to follow an
extern relocation. The `MObjSub` sprite and palette tables need the same thing
and did not have it.

**Two rules had to change.** `read_material`'s `indirect()` required the table
entry to be an *intern* relocation:

```rust
(array != 0 && is_ptr(at + field) && is_ptr(array))
```

`pointer_slots()` builds `is_ptr` from `intern_relocs` only, so a cross-file
entry fails that test; and even if it passed, the word behind it is zero. The
entry is now resolved as either an intern pointer or an extern one, and
`MObjMaterial::sprite`/`palette` carry an optional file id the way
`TextureRef` already did.

**Result.** Dream Land's ground resolves to file 103 `+0x1BE0`, a 32×32 CI4
with a TLUT, and **renders as the stage's basket-weave underside on device**.
The stage goes from 17 to 19 of its 20 references packed.

**The count was also wrong.** The report deduplicated textures on
`(home file, data offset)`. Every unresolved reference has offset zero, so all
of them in a file collapsed to one entry — "54 null pointers" was counting
*files*, not textures. Keying on the dimensions as well, the honest totals are:

| | before | after |
|---|---|---|
| unique references bound | 695 | **732** |
| packed | 612 | **615** |
| failed | 83 | **117** |
| unresolved `G_SETTIMG(0)` | 54 | **75**, across 54 files |

The rise in "failed" is a reporting fix, not a regression: those references
were always failing and were being hidden by the collapse. Segmented addresses
went 13 → 26 the same way.

**What the remaining 75 need is not in the ROM.** They belong to 71 scene
graphs that no data structure names. A fighter's table is named by
`FTCommonPart` and a stage layer's by `MPGroundDesc` (RE-030), both of which
this crate reads. These graphs are paired in *code*:

```c
gGRCommonStruct.pupupu.map_gobj[1] = grPupupuMakeMapGObj(
    &llGRPupupuMapWhispyMouthTransformKindsDObjDesc,
    &llGRPupupuMapWhispyMouthTransformKindsMObjSub, ...);
```

The graph offset and the table offset are two link-time constants passed as
arguments. There is no record in the data that relates them, so recovering
them from the ROM alone would mean searching each file for a table that
happens to parse for that graph — the kind of fingerprint that fits by
coincidence often enough to be worthless. Note too that `map_gobj[2]` and
`[3]` pass `o_mobjsub = 0x0`: some of these graphs correctly have no material
table at all, so even a perfect search has no unique right answer to find.

Dream Land's four remaining unresolved references are Whispy Woods' eyes and
mouth. The trunk, the ground, the platforms and the foliage all draw.

**Confidence: high.** The sprite table's extern relocations are read from the
archive's own relocation record, not inferred, and the recovered address is
the one the relocation names. The result is visible: the stage's underside
draws its texture. **The remaining 75 are not fixed**, and the reason is that
the pairing they need was a compile-time constant in the game's code.

## RE-047 — A discovered display list was decoded at the wrong base

**Question.** RE-046 left 75 unresolved `G_SETTIMG(0)` references across 54
files, and concluded they were unrecoverable: they belonged to 71 scene graphs
the original pairs to a material table in *code*, so the pairing is not in the
archive. Could any of them be recovered anyway?

**The table search says no, and says it clearly.** Each node's display lists
state how many `MObj`s they call, so `demand[i]` is an independent constraint
on any candidate table — a table that satisfies the whole vector, including the
nodes where the chain must be *absent*, is not one that merely parses.
`mobj::search_tables` applies that, and `romtool mobj --search
--expect-tables` scores it against the tables the decomp declares as
`MObjSub **name[]`:

```
material-table search over 71 unnamed graph(s):
  exactly one candidate   16
  several candidates      55
  of the unique ones, against the decomp's declarations:
    confirmed             2
    contradicted          4
```

Sixteen of seventy-one narrow to one answer, and of the six the key can score,
**four are wrong**. That is the fingerprint-that-fits-by-coincidence outcome,
measured rather than assumed, and it settles the question: the search is not
usable. The code and its scoring are kept so the next attempt starts from the
measurement instead of repeating it.

**But the premise was wrong again.** Converting only via scene graphs, and not
via discovery, drops the unresolved count from 75 to **zero**. None of them
were in graph-drawn geometry at all. They were in display lists found by
*scanning*, and the scan decoded them at the wrong base:

```rust
let Ok(cmds) = dl::decode_list(&file.data[off..]) else { continue };
```

`decode_list` is `decode_list_at(data, 0)`. The base is what a `G_SETTIMG`'s
relocation slot is computed from, and a relocation is keyed by a *file* offset.
So every list discovered by scanning looked its relocations up at
`offset-within-the-list` instead of `offset-within-the-file`, found nothing,
and reported a perfectly ordinary cross-file texture as an unresolved null.
File 118 has exactly 36 `G_SETTIMG(0)` and exactly 36 extern relocations into
file 110; they are the same 36.

This is RE-037's mechanism failing in one of its two callers. The scene-graph
path already used `decode_list_at` with the node's own offset, which is why the
bug was invisible there and why the two paths disagreed.

**Three smaller things fell out of the same investigation.**

*A discovered list that calls the graphics heap cannot be converted.* Its
texture, palette and colour come from an `MObj` chain that belongs to a scene
graph node, and a list found by scanning bytes has no node. The converter
correctly drops the binding rather than guessing, which leaves geometry whose
palette never loads — and the packer shipped it. Skipping those costs **zero**
packed textures and no geometry any object draws (object triangles stay at
28,957), and removes 12 knowingly-broken bindings.

*Intensity textures were not getting the CLUT `choose_psm` picks for them.*
`choose_psm` maps I4/I8 to `PsmT4`/`PsmT8` with the comment "a CLUT keeps them
at 4/8 bits instead of expanding 8× to 8888" — but the conversion only took the
paletted path when a TLUT had been read *from the ROM*, and an intensity
texture has none, because on the N64 it needs none. All thirteen fell through
to `Psm8888`. They now generate their ramp, matching `texture::decode`'s own
`(v << 4) | v` expansion. The palette has to be RGBA8888 rather than the
RGBA5551 a ROM TLUT is: intensity drives alpha too, and 5551 has one alpha bit.

*Twenty-six failures are not ROM textures at all.* Every one of the segment
`0x01` references is in an `LBTransition*` file, and `lbtransition.c` says what
segment 1 is there:

```c
gSPSegment(gSYTaskmanDLHeads[0]++, 0x1, sLBTransitionPhotoHeap);
...
heap_pixels = sLBTransitionPhotoHeap = syTaskmanMalloc(300 * 220 * sizeof(u16), 0x10);
```

A runtime buffer holding a captured screenshot. The wipe effects texture their
geometry with the previous frame. No extraction can ever resolve these; they
need render-to-texture.

**Result.**

| | RE-046 | now |
|---|---|---|
| references bound | 732 | 647 |
| packed | 615 | **617** |
| failed | 117 | **30** |
| unresolved `G_SETTIMG(0)` | 75 | **0** |
| texture VRAM | 634.6 KiB | **577.7 KiB** |
| pack size | 3091.7 KiB | **2881.2 KiB** |

"Bound" falls because references that used to read as null, and were counted
per file, now resolve to the shared textures they always named.

The 30 remaining failures are 26 transition screenshots and 4 CI textures whose
`MObj` supplies only a primitive colour, so the `G_LOADTLUT` beside them reads
whatever texture image the previous list left set. That last group is a real
open question — the RDP's image register persists across lists and this
converter does not model that — but it is four textures.

**Confidence: high.** The decode-base fix is checked by a unit test that fails
without it, the 36-for-36 correspondence in file 118 is exact, and the packed
count went *up* while the failure count went down. The device render is
unchanged, which is the point: nothing that was drawing has stopped.

## RE-048 — The odd shapes in Dream Land's canopy are billboards

**Question.** Dream Land renders with correct ground, bark and foliage, but six
flat pink, purple and gold triangles sit in the tree canopy looking wrong. Are
they textures that should be animated?

**Not animated colours.** `MPGroundDesc` has four fields and this crate read
two:

```c
struct MPGroundDesc {
    DObjDesc *dobjdesc;            // +0x0  read
    AObjEvent32 **anim_joints;     // +0x4  ignored
    MObjSub ***p_mobjsubs;         // +0x8  read
    AObjEvent32 ***p_matanim_joints; // +0xC ignored
};
```

Reading both: **40 of 100 stage layers carry joint animation and 12 carry
material animation**, and neither is played. For fighters, RE-040 found that
the values baked into `MObjSub` are *not* the initial ones — the costume script
overwrites them at setup — so the obvious suspicion was the same bug for
stages. Decoding every stage's `p_matanim_joints` at frame 0 and diffing it
against the baked colours gives **no differences at all**, across all 12
layers. The stage draws the right first frame. It simply does not move.

**They are billboards.** The decomp names those six display lists
`dStagePupupuFile2_Layer0Anim_DL_*`, and their `DObjDesc` entries share an `id`
of `16385` = `0x4001` where the rest of the graph uses `0`, `1`, `2` or
`0x2001`. `gcSetupCommonDObjs` reads the high nibble as a matrix kind:

```c
if (dobjdesc->id & 0x4000) { ... nGCMatrixKind46 : nGCMatrixKind45; }
else if (dobjdesc->id & 0x2000) { ... nGCMatrixKind48 : nGCMatrixKind47; }
```

and kind 45 builds the MVP straight out of the *projection* matrix, zeroing
every cross term:

```c
sGCMatrixMvpF[0][2] = sGCMatrixMvpF[1][2] = 0.0F;
sGCMatrixMvpF[2][0] = sGCMatrixMvpF[2][1] = 0.0F;
sGCMatrixMvpF[0][0] =  gGCMatrixPerspF[0][0] * scaX * cosf(rot.x);
sGCMatrixMvpF[1][0] = -gGCMatrixPerspF[0][0] * scaX * sinf(rot.x);
sGCMatrixMvpF[0][1] =  gGCMatrixPerspF[1][1] * scaY * sinf(rot.x);
sGCMatrixMvpF[1][1] =  gGCMatrixPerspF[1][1] * scaY * cosf(rot.x);
```

Object X and Y map directly onto screen X and Y, spun in-plane by `rotate.x`;
Z contributes depth only. That is a screen-aligned billboard, and it is why
they read as flat triangles at arbitrary angles: they are drawn with the node's
static matrix instead.

`scene::DObjDesc::transform_kind()` already parses this and is unit-tested —
and is **called nowhere outside its own tests**. Across the archive:

| kind | nodes |
|---|---|
| `TraRotSca` (plain) | 3008 |
| `Kind48` (`0x2000`) | 47 |
| `Kind46` (`0x4000`) | 34 |
| `RecalcRotRpyRSca` (`0x8000`) | 28 |

109 nodes want a transform this renderer does not apply, 81 of them
camera-relative. Note also that the decomp picks kind 45 *or* 46 by a
`rot_mode` this crate does not model — it always reports the `46` branch.

**A fourth cross-file bug, found on the way.** `romtool texdump` read texels
and TLUT from the drawing file rather than following `TextureRef::data_file`,
so every stage texture dumped as noise — which looks exactly like a broken
decoder and would have sent the next investigation in the wrong direction.
Dream Land's textures are correct; the dumper was not. That is the same
indirection as RE-037, RE-046 and RE-047, in a fourth caller.

**Confidence: high** for the diagnosis: the `id` values are in the ROM, the
matrix kind they select is the decompilation's own, and kind 45's body is
unambiguous about what it computes. **Nothing is fixed here.** Billboards need
a per-frame camera-relative matrix on the PSP side, which is a rendering
feature rather than a converter change, and the joint animations need the
`AObj` player pointed at stage nodes.

> **Correction (RE-049).** This entry said billboarding was "why six sprites
> sit flat in Dream Land's canopy". That does not follow. The stage view sets
> the camera rotation to exactly zero — *"A stage is a place, not an object:
> spinning it would make the collision overlay impossible to read. Face-on,
> always."* — and with an unrotated camera a screen-aligned billboard and a
> static XY quad are the *same matrix*. Implementing billboards changed that
> screenshot by zero pixels, as it had to. The shapes look how they look
> because that is their geometry; billboarding only matters once the camera
> turns.

## RE-049 — Billboards, and a screenshot that could not have shown them

**Question.** RE-048 identified 81 nodes asking for a camera-relative matrix
kind that nothing applied. Implement it.

**The flag.** `NodeDesc` had exactly four bytes of tail padding, which is now a
`flags` word carrying `FLAG_BILLBOARD` for `DObjDesc.id & 0x6000` — kinds 45-48.
A pack written before it existed reads those bytes back as zero, which is "no
billboards", i.e. the old behaviour, so the field is backward-compatible on its
own; the version went to 7 regardless because the pack is rebuilt every run and
a silent format change is worth more than the convenience. 81 nodes carry the
flag, matching RE-048's count exactly, and a test asserts a `0x4001` node
reaches the reader flagged — it fails if the writer stops mapping `Kind46`.

**The matrix.** The original writes the MVP straight from the projection basis.
This build cannot copy that literally, because it never builds an MVP: it keeps
the view matrix at identity and puts the whole camera into the model matrix it
passes down. In that arrangement "aligned with the eye" is simply "unrotated in
world space", so the sprite wants the *composed* position and scale of
`base * local` with the orientation discarded, plus `rotate.x` as a spin about
Z. Scale is recovered as the length of each composed basis column rather than
read from `rest_scale`, because a node inherits its ancestors' scale and only
the composed matrix knows the product.

**Verifying it needed a rotated camera.** The first device run after
implementing this changed **zero pixels**, which looked like a failure and was
not. Two checks separated those cases:

1. *Are these even the right nodes?* Skipping every flagged node made the six
   canopy triangles — and only them — disappear. So the flag reaches the draw
   path and identifies exactly the shapes in question.
2. *Is the transform doing anything?* The stage view fixes the camera rotation
   at zero, and with no rotation a screen-aligned billboard **is** the static
   matrix. Forcing the stage camera to `[0, 0.7, 0]` and rendering with the
   flag honoured and ignored gives two clearly different images: ignored, the
   sprites are skewed and squashed into slivers by the camera angle; honoured,
   all six are upright, symmetric and identical in shape wherever they sit.

That second run is the evidence. The face-on screenshot being byte-identical is
a *prediction* the implementation had to satisfy, not a null result — a diff
there would have meant the billboard path was wrong.

**Result.** 81 nodes across the archive now face the camera. Nothing else in
the frame moved, object triangles are unchanged, and the pack still verifies.
Dream Land's canopy sprites are pink, purple and gold upright triangles.

**Confidence: high** for the mechanism and its scope, **medium** for the exact
appearance: the decomp picks kind 45 *or* 46 by a `rot_mode` this crate does
not model, and the 28 `0x8000` (`RecalcRotRpyRSca`) nodes are still drawn
plainly. Neither has been tested against hardware.

## RE-050 — Stage joints run on the 32-bit event stream

**Question.** RE-048 found that 40 of 100 stage layers carry a joint animation
that nothing plays. Play them.

**A different encoding, the same machine.** A fighter's joints are driven by
the 16-bit figatree stream (RE-036). A stage's come from `MPGroundDesc`'s
`anim_joints`, one `AObjEvent32 *` per node, handed out by `gcAddAnimJointAll`
and run by `gcParseDObjAnimJoint` — which feeds *the same `AObj` tracks*
`gcPlayDObjAnimJoint` reads. So `figatree`'s state machine is reused verbatim
and only the instruction decoding is new.

One thing about the values is not shared. The 16-bit stream stores an `s16`
that `ftAnimGetTargetValue` scales — 1/512 for rotations, 1/4 for translations.
A 32-bit event stores **a real `f32`**, already in radians or model units.
Applying figatree's scale factors here would be wrong by three orders of
magnitude, and a unit test pins that a `SetVal` of π/2 comes back as π/2.

**`AObjAnimAdvance` is a post-increment**, which settles a reading that three
opcodes depend on:

```c
#define AObjAnimAdvance(script) ((script)++)
```

So `flags = AObjAnimAdvance(event32)->command.flags` reads the flags out of the
command word *itself* and leaves the cursor past it. That confirms the layout
[`matanim`](../crates/ssb-rom/src/matanim.rs) already used, and fixes the three
opcodes the ROM actually needed:

| opcode | | words | effect |
|---|---|---|---|
| 12 | — | 0 | `length += payload` on each named track; no key |
| 13 | `SetInterp` | **1** | hands `TraI` a pointer to spline control points |
| 14 | `SetAnim` | 1 | continues at a new script, like `Jump` |

Before them, 89 of 206 scripts hit an unmodelled opcode.

**The check that mattered was not the one that looked convincing.** Replaying
every script gives 206 of 206 running, 123,600 frames, no unmodelled opcode,
and — across about 1.1 million pose components — zero non-finite values and
zero denormals. Denormals are the signature of a desynchronised stream, since a
command word reinterpreted as a float is around 1e-35, so that reads like a
strong result.

It is not sufficient. Deliberately dropping `SetInterp`'s pointer word — a real
one-word desync — still gives **206 scripts, 0 failures, 0 denormals**. What
moves is the frame total, 123,600 to 85,863, because a slipped stream runs into
an `End` early.

That is the discriminating check, and it is only meaningful because of what
these animations *are*: ambient scenery, which loops forever. `206 × 600` is
exactly 123,600 — every script is still running when the budget expires. Under
the one-word slip it is **143 of 206**. So the reported number is now "still
running after 600 frames", not "replayed without failing".

The word count itself comes from the decompilation (`AObjAnimAdvance` twice
around the pointer read), not from the replay. The replay confirms it; it could
not have found it.

**Result.** All 206 stage joint scripts decode and play, on every stage.
Largest pose magnitude is 26,721 model units, which is large but in range for
background scenery on a stage whose map bounds are ±9,000.

**Confidence: high** for the decoding, which is the decompilation's own and is
checked by 206 scripts looping indefinitely. **Nothing is animated on device
yet**: the pack has no table for these scripts and the PSP side does not tick
them. That is the next increment, and it is mechanical — the hard part, being
sure the stream is read correctly, is done.

## RE-051 — Stage scenery animates on device

**Question.** RE-050 decoded and validated the 32-bit joint stream on the host.
Get it onto the PSP.

**No new pack tables.** A stage animation needs exactly what a fighter's needs:
a file of script bytes, and per joint a `(script offset, node index)` pair.
That is `AnimDesc` and `AnimJoint` unchanged, so stage entries go in the same
tables with `fighter = AnimDesc::STAGE` (`u32::MAX`) and `slot` as the stage
index. `Pack::stage_anim` scans for them, because unlike a fighter's they are
not at a computable row.

Copying each animation file whole costs **790 KiB** — the pack goes 2881 to
3674 KiB. That is the same thing the fighter path already does, and it is what
keeps the scripts' absolute `Jump` and `SetAnim` targets valid without
rewriting them. Copying only each file's reachable span and rebasing those
targets would be much smaller and is the obvious later optimisation; it is not
done here because it trades a verified-correct copy for a rewrite that could be
subtly wrong.

**One ordering bug, found by a triangle count.** `Pack::fighter_anim` finds a
row by arithmetic:

```rust
let a = self.anim(fighter * slots + slot)?;
```

which only holds while the fighter entries are a dense block starting at index
0. Stages are packed *before* fighters, so writing the stage animations where
they were produced put 35 rows in front of that block, and every fighter
animation resolved one stage too far. The pack still verified, every host check
still passed, and the on-device overlay read `fighter 4294967295 slot 1` with
`joints 0` — the fighter had quietly been handed a stage's skeleton and drew
nothing. The only number that moved was the stage view's triangle count, 495 to
175.

Stage entries are now emitted after the fighter loop, and a test builds a full
fighter block plus one stage row and asserts every fighter slot still resolves
to itself. The same collision bit the `figatree` verifier, which walked every
`AnimDesc` and tried to decode a 32-bit script as a 16-bit one; it now skips
`STAGE` rows.

**Dream Land is not the example.** Through RE-048 and RE-050 this document said
"Whispy never sways". Dream Land's layers carry **no `anim_joints` at all** —
only a material animation on layer 2. Its scenery is moved by the wind state
machine in `grpupupu.c`, which is game code, not data. The stages that do carry
joint animation start at file 256.

**Verified with a control.** Two captures twelve seconds apart, differenced:
with the animation on, the overlay's counters change and there is a speckled
cluster exactly over the tree canopy. With it off, and the same two timestamps,
the canopy is **pixel-identical** and only the counters and the fighter remain.
The motion is subtle — a sway, not a lurch — so the difference image is the
evidence rather than the screenshots.

**Result.** 35 stages, 215 animated nodes, ticked and composed each frame at a
locked 60 FPS. A node the animation does not drive keeps its packed rest
matrix, and `draw_stage` is `draw_stage_animated(.., None, ..)`, so the still
and moving paths are one piece of code.

**Confidence: high** that the scripts play and move the right nodes.
**Medium** on faithfulness: nothing compares a posed stage node against the
original frame by frame, the way `figatree` does for fighters against the ROM.
The material animations (12 layers) are still not played, and their frame 0
already matches what renders, so nothing is visibly wrong from that.

## RE-052 — Checking the packed stage animation against the archive

**Question.** RE-051 got stage scenery moving on device, but nothing compared a
*packed* stage pose against the original the way `figatree` does for fighters.
That gap is where RE-051's one real bug had lived. Close it.

**The check.** `romtool stages --pack` replays every packed stage animation
twice: once through `StageAnimator` reading the pack's tables and blob, and
once by rebuilding the same joints straight off the archive file. Both run 240
frames and every one of the ten track values is compared per joint per frame.
It also asserts the packed blob is the archive file byte for byte, because an
altered byte would leave every offset agreeing while the data under them moved.

This is not testing the decoder — RE-050 already replayed that against the ROM.
It tests everything the *packing* adds between archive and device: script
offsets, node indices, the copied blob, and which table row a stage lands in.

**It found a second bug immediately.** Stage 5 (file 107) failed with a script
offset of 202,375,168 — not an offset at all. The `anim_joints` table was being
read with the *object's* node count:

```rust
let nodes = writer.object_node_count(object).unwrap_or(0) as usize;
```

An object is not just its graph. The packer appends "extra leaf nodes for lists
a node could not hold" — 20 of them archive-wide — so `object.node_count`
exceeds the graph's, and reading the table that far walks off its end into
whatever follows. Nine of the 215 "animated nodes" were bytes past the end of a
table, read as pointers.

The count is now the graph's own, and the packed total drops 215 → **206**,
which is exactly the number of scripts the host-side replay finds. Two
independent paths agreeing on 206 is worth more than either number alone.

**Result.**

```
stage animations replayed from assets/generated/ssb64.pak: 35 stage(s), 206 joint(s)
  444960 pose value(s) compared against the archive
            every packed pose matches the archive exactly
```

Note also that the on-device stage view's triangle count returned to 495 from
175 once RE-051's ordering bug was fixed — the fighter had been drawing with a
stage's skeleton. That number is now the cheapest regression signal for this
whole area, which is worth knowing given how much passed while it was wrong.

**Confidence: high.** The two replays are independent in the way that matters —
one reads the pack, the other the archive — and 444,960 compared values agree
exactly. What is still *not* checked is faithfulness to the original console:
both paths run this crate's own player, so a shared misreading of the format
would agree with itself. RE-050's opcode semantics come from the decompilation
rather than from this check.

## RE-053 — Mipmaps, and a tree that is still not right

**Question.** Dream Land's tree canopy does not look like the N64's. Why, and
can it be fixed?

**The textures are dithered.** Dumping them shows what the canopy is made of: a
64×64 CI4 *dithered gradient*, sixteen colours arranged to fake a smooth green
ramp, and a second 64×64 whose highlight is a dithered diagonal wash. The
dither **is** the shading. The N64 resolves it with its own filtering into a
320×240 composite signal; a sharp display at one texel per pixel does not.

The `textures` report now prints how far a texture is stretched across the
surface using it — UVs are S10.5, 32 units per texel, so the span in texels
over the texture's size is the repeat count. Dream Land's canopy runs at
3.70 × 1.36 repeats of a 64×64, about 1.25 texels per pixel: mild minification,
which is the range where a dithered pattern aliases.

Box-filtering the texture to 32×32 by hand turns it straight back into a smooth
gradient, so mipmapping is the indicated fix rather than a guess.

**Implemented.** `psp_texture::pack_mipped` generates the chain from the
decoded image and re-encodes each level. For a paletted texture that keeps it
at 4 bits per texel: each level's texel takes the *nearest palette entry* to
the local average, and on a gradient ramp the nearest entry to an average of
two dithered neighbours is the shade between them. Level 0 is regenerated the
same way and comes back bit-identical, which a test pins.

The chain stops where swizzling would be lost. The GE's swizzle flag is per
texture, not per level, so requiring every level to be swizzlable cost *all*
of them their swizzling — 58% down to 0%. Since the levels that resolve dither
are the first one or two, the chain now ends rather than the swizzling.

| | before | after |
|---|---|---|
| textures with extra levels | 0 | **151 of 617** |
| texture VRAM | 577.7 KiB | **717.3 KiB** |
| swizzled | 356 (58%) | 356 (58%) |
| pack | 3674 KiB | 3794 KiB |

VRAM now exceeds the 700 KiB "all at once" figure, which was accepted
deliberately rather than discovered.

**A second report that had drifted.** The `textures` command kept its own copy
of the conversion, so it reported a VRAM figure with no mip levels in it while
the pack shipped them — the same two-implementations problem as RE-047. It now
calls the packer's `convert_texture` and only classifies the failures itself.

**The tree still does not look right, and this entry does not fix it.** The
mip levels are generated, packed, uploaded and demonstrably change the frame,
but the canopy's diagonal pattern survives. Two observations argue the
diagnosis above is incomplete:

* Under PPSSPP's software rasteriser the canopy is essentially unchanged.
* Under OpenGL, which renders at a higher internal resolution, the pattern gets
  **crisper**, not softer. Minification moiré would do the opposite.

A pattern that sharpens with resolution is being *magnified*, not minified — so
at least that surface is sampling below one texel per pixel and mipmaps cannot
help it. What it needs is either the filtering to soften the dither at
magnification, or the dither resolved at conversion time.

The mipmaps are worth keeping regardless: they are correct, cheap, and fix
minification everywhere else. But **the reported symptom is not resolved**, and
the measurement is further muddied by the two PPSSPP backends disagreeing
(RE-014 again). Deciding it on real hardware, or by rendering one surface in
isolation at a known scale, is the next step.

**Confidence: high** that the chain is generated and uploaded correctly — the
level-0 round trip and the swizzle cut-off are unit-tested, and 151 textures
carry levels. **Low** that this addresses what was actually asked.

## RE-054 — BattleShip cross-reference (PLAN.md R0.2)

**Question.** `AGENTS.md` §10 says BattleShip should be actively consulted for
GBI/RDP/RSP questions, but it was never actually cloned into this checkout and
nothing in this log references it. What does it say about our open rendering
questions, and where does it agree or disagree with what we've already
recovered from the ROM and decomp directly?

**Setup.** Cloned `JRickey/BattleShip` and its `libultraship` submodule
(`ssb64` branch) into `refs/`. The relevant code is
`refs/BattleShip/libultraship/src/fast/interpreter.cpp` (~7,500 lines) — a
software RSP/RDP interpreter that translates F3DEX2 (plus some S2DEX)
display lists to a modern GPU API. This is architecturally the opposite of
our approach (D-001: no RSP emulation, build-time conversion), so most of it
is not directly portable, but it is a working, decomp-accurate reference for
what each command *means*.

**Opcode coverage agrees.** Every opcode in `docs/rendering.md`'s "measured
usage" table has a handler in BattleShip's `f3dex2Handlers` table. No
disagreement found in what the opcodes we already track actually do.

**New lead for R0.13 (framebuffer rendering).** BattleShip explicitly handles
`G_BG_1CYC` (0x09) and `G_BG_COPY` (0x0a) — S2DEX background-image commands —
mixed directly into SSB64's F3DEX2 display lists, with a comment noting SSB64
"mixes S2DEX BG commands into F3DEX2 display lists without a G_LOAD_UCODE
switch." Our `crates/ssb-rom/src/dl.rs` decoder has **no opcode constants for
these at all** — `grep` for `BG_1CYC`/`BG_COPY`/`S2DEX` in that file returns
nothing. These opcodes draw a full-screen background image
(`F3DuObjBg`/`Gfxs2dexBg1cyc`/`Gfxs2dexBgCopy`) and are a plausible mechanism
for screen wipes, which R0.13 has not started and which currently show up as
unresolved segment-0x01 texture references under R0.3. Not confirmed against
our own ROM data yet — that is the next step before implementing anything.

**Corroborates RE-053 (Dream Land canopy).** BattleShip's texture-tile sampler
has a comment reading "No LOD support: force both slots to the base mip
level" (`interpreter.cpp:3152`) — the reference PC port does not implement
N64 LOD/mipmap selection at all, it always samples level 0. This supports
RE-053's conclusion that mipmapping is not the fix for the canopy: even a
port with zero LOD support has to get the canopy right some other way
(dithering/magnification handling, not minification). Does not by itself
explain what BattleShip *does* do differently that makes it look right (or
whether it does) — that would need running BattleShip against the same ROM
and comparing, which was out of scope for this pass.

**Confirms the shape of R0.8's open transform question, doesn't resolve it.**
`interpreter.cpp`'s `G_MW_MATRIX` handler (~line 3794) has a detailed comment
and implementation for patching individual halves of the RSP's live MVP
matrix in place, following a `gSPMvpRecalc`. Cross-checked against
`refs/ssb-decomp-re/src/sys/objdisplay.c` (already present in `refs/`, no
need to clone anything further): the pattern at objdisplay.c:753-782 and
:806-818 is real — SSB64 computes a custom 4×4 matrix on the CPU (into
`sGCMatrixProjectL` or `mtx_hub.gbi`, depending on the draw-type branch) and
then emits `gSPMvpRecalc` + eight `gMoveWd(G_MW_MATRIX, ...)` calls to patch
it into the RSP's current MVP, rather than uploading a whole new matrix via
`G_MTX`. Since we don't emulate the RSP (D-001), the RSP-side patching
mechanics don't matter to us — what matters is that **the actual transform is
CPU-computed matrix math in `objdisplay.c`**, not a novel RDP/RSP behavior.
That means R0.8's "0x8000 transform" / `RecalcRotRpyRSca` kinds are exactly
the kind of thing D-001 says to port directly from the decompilation, the
same as every other transform. This pass did not locate the specific
matrix-building function for those draw-type branches (the name
`RecalcRotRpyRSca` used in `docs/porting-status.md` did not turn up literally
in `objdisplay.c` — it may be named differently, or be in a different file);
finding it is R0.8's actual next step, not this entry's.

**Confirms the wrap-mode gap is real, not hypothetical.** BattleShip decodes
`cms`/`cmt` (clamp/mirror bits) per axis independently and applies them
per-draw (`interpreter.cpp:3242-3251`). `psp/src/meshdraw.rs` currently
hardcodes `sceGuTexWrap(Repeat, Repeat)` for every draw regardless of what
`G_SETTILE` specified — already tracked as an open R0.5 item; this just
confirms the reference implementation treats it as a real per-texture setting
worth threading through, not a corner case.

**Confidence: high** that the opcode-level findings above (BG commands,
LOD-forced-to-base, MVP patch mechanism, per-axis wrap) are accurately
transcribed from BattleShip's source — these are direct code reads, not
inference. **Low/unconfirmed** that any of them is *the* fix for R0.13, R0.5
or R0.8 — none of this pass tested a hypothesis against our own ROM or
device output. It only established there is a plausible, reference-backed
lead where one was previously missing.

## RE-055 — The 26 segment-0x01 texture failures are the loading-transition photo, not S2DEX BG (PLAN.md R0.3)

**Question.** RE-054 raised a lead that the 26 segment-0x01 texture-conversion
failures under R0.3 might be S2DEX `G_BG_1CYC`/`G_BG_COPY` background draws
that our `dl.rs` misreads as ordinary `G_SETTIMG`. STATUS.md's R0.3 next step
was to check this against the actual failing display lists before writing a
fix for either hypothesis.

**Test 1: does the opcode even appear in this ROM?** `romtool scan --exhaustive`
walks every discoverable display list in the archive and tallies opcode
frequency. `0x09`/`0x0A` (`G_BG_1CYC`/`G_BG_COPY`) appear **zero times** in
the full opcode table (checked against both the default reloc-target scan and
`--exhaustive`, which also finds lists not reachable from typed relocations).
Whatever produces the segment-0x01 addresses, it is not an S2DEX BG command
anywhere in this ROM's display lists.

**Test 2: what does the raw display list actually say?** Dumped file 39 (one
of the 13 files with segment-0x01 failures) via `romtool dump` and walked its
bytes directly. The failing `G_SETTIMG` sits at file offset `0x0E10`:

```
0x0E00: fc127e24 fffff3f9   G_SETCOMBINE
0x0E08: d7000002 ffffffff   G_TEXTURE (on)
0x0E10: fd10012b 01000000   G_SETTIMG  addr=0x01000000 (segment 1, offset 0)
0x0E18: f5109600 07020090   G_SETTILE
0x0E20: f5109600 000a0290   G_SETTILE
0x0E28: de000000 00000e38   G_DL -> 0x0E38 (LOADTILE/SETTILESIZE)
```

This is not misaligned or garbled — it is a completely ordinary, well-formed
F3DEX2 texture-bind idiom (`SETCOMBINE` → `TEXTURE` → `SETTIMG` → `SETTILE`×2
→ `DL` doing the tile load). The same exact instruction word
(`fd10012b 01000000`) recurs verbatim in files 40, 41, 45, 50 and 51 at their
own offsets, confirming this is one shared reference repeated across many
files, not a per-file coincidence.

**Test 3: what does segment 1 mean in the decompilation?** Ground truth:
`refs/ssb-decomp-re/src/lb/lbtransition.c`:

```c
// 0x800D6488 - Heap for "photocopy" of last frame drawn to framebuffer
void *sLBTransitionPhotoHeap;
...
void lbTransitionProcDisplay(GObj *gobj) {
    gDPPipeSync(gSYTaskmanDLHeads[0]++);
    gSPSegment(gSYTaskmanDLHeads[0]++, 0x1, sLBTransitionPhotoHeap);
    gcDrawDObjTreeForGObj(gobj);
    ...
}
...
heap_pixels = sLBTransitionPhotoHeap = syTaskmanMalloc(300 * 220 * sizeof(u16), 0x10);
```

Segment `0x1` is bound at runtime, once per frame, to `sLBTransitionPhotoHeap`
— a `300 * 220` 16-bit heap buffer the engine fills with a copy of the last
frame rendered to the framebuffer, for the between-match "LB" (loading break)
transition wipes (`dLBTransitionDescs`: aeroplane, curtain, cannon/"kannon",
star, sudare/bamboo-blind ×2, camera, block, rotscale, check, "gakubuthi" —
11 transition variants, close to the 13 files affected). The width (300) and
pixel size (16-bit) match exactly: `romtool textures --file 39` reports the
two failing binds as `300x5 Rgba/Bits16` and `300x6 Rgba/Bits16` — thin
horizontal strips of exactly that 300-wide photocopy buffer, consistent with
a wipe effect that reveals the captured frame a few scanlines at a time. This
is `gSPSegment`/segment-0x0E's own pattern (already handled in
`crates/ssb-rom/src/mesh.rs` for the `MObj` graphics heap) applied to a
different runtime buffer: an address that is bound by the RSP at draw time
and never exists at a fixed offset in any ROM file.

**Conclusion.** The 26 segment-0x01 failures are not a texture-conversion bug
and not an S2DEX decoding gap. They are references to a per-frame
framebuffer photocopy that our build-time, no-RSP-emulation pipeline (D-001)
cannot resolve from ROM data, because the data does not exist in the ROM —
it is generated at runtime from whatever was last drawn. This is R0.13
("framebuffer effects") territory, not R0.3: there is no fix to write under
texture conversion, only a decision (deferred to R0.13, which is still
blocked on R0.6) about whether to ever implement the LB transition system at
all. RE-054's S2DEX-BG lead is refuted by Test 1 and superseded by this
entry.

**Confidence: high.** The decomp citation names the exact segment number, the
exact heap variable, its exact size, and the exact call site; the byte-level
read is a direct, unambiguous decode of a well-formed instruction sequence;
and the dimensions independently corroborate the theory without having been
sought out in advance. Not yet checked: whether all 26 failures (vs. just the
6 files sampled) share this exact address, though the per-file failure counts
(each affected file loses exactly 2 textures) are consistent with it.

## RE-056 — A lead on the 4 MissingPalette cases (PLAN.md R0.3), not a fix

**Question.** With RE-055 closing the segment-0x01 question, R0.3's only
remaining gap is 4 `MissingPalette` failures (files 52 ×2, 86, 353), each a
CI4 texture that packs with a "CI texture, no TLUT recorded" note.

**What the bytes show.** File 52's failing texture (offset `0x1960`) is
bound by six separate `G_SETTIMG` instructions across the file, not one. At
least one of them (offset `0x6530`) is immediately preceded, a few
instructions earlier in the same list, by a `G_LOADTLUT` whose source image
was set via a `G_SETTIMG` at `0x6500` (addr `0x1c0`) — the ordinary
`SETTIMG(palette)` → `LOADTLUT` → `SETTIMG(texture)` → `SETTILE` idiom that
converts correctly everywhere else in this ROM. That occurrence, read in
isolation, has a palette.

**Why it still fails.** `romtool textures` dedups texture bindings by
`(home file, data offset, width, height)` (`tools/romtool/src/main.rs`,
`seen` set) and only evaluates the *first* occurrence it walks to for a given
key; the other five bindings of the same texture are never separately
checked. If the walk order reaches a *different* occurrence first — one
whose call path never executes a `G_LOADTLUT` before this `SETTIMG` (for
example because `crates/ssb-rom/src/mesh.rs`'s `forget_texture()` ran between
the palette load and this bind, which clears `palette_offset` along with
everything else, or because this occurrence is reached via a joint/continuation
list that starts after the palette load) — it reports no palette, even though
a palette does exist for this exact texture somewhere else in the file.

**Not confirmed.** This pass did not trace which of the six occurrences
`file_meshes`/`mesh::convert_sequence` actually visits first, nor whether the
"losing" occurrence's own state genuinely has no reachable `G_LOADTLUT` (a
real per-occurrence gap) versus an artifact of the dedup key discarding a
valid binding in favor of a bad one (a `romtool` reporting bug, not a
conversion bug — the packer itself runs `convert_texture` per-primitive and
may already handle the good occurrence correctly for actual gameplay). This
overlaps with R0.4's open "palette inheritance/state...leakage" item and
R0.15. Either way, this is not S2DEX/segment-related and unrelated to
RE-055.

**Confidence: medium.** The byte-level read of one occurrence is a direct
decode, not inference. The explanation for *why the failure still happens
despite a valid occurrence existing* is a plausible mechanism grounded in
`mesh.rs`'s actual state-reset logic and `romtool`'s actual dedup key, but it
was not stepped through with a debugger or instrumented print to confirm
which occurrence is "first," so treat it as the next concrete step, not a
finding.

## RE-057 — The 4 MissingPalette cases are a `PartTables` pairing gap, not a dedup artifact (PLAN.md R0.3 / R0.7)

**Question.** RE-056 left open which of a failing texture's several
occurrences `mesh::convert_sequence` visits first, and whether the loss is a
`romtool` reporting artifact or a real per-occurrence gap. This entry
answers it by instrumenting `crates/ssb-rom/src/mesh.rs` (temporary
`eprintln!`s in `convert_sequence`, the segment-0x0E `Call` handler, and
`SetTimg`/`LoadTlut` — reverted after use, not committed) and re-running
`romtool textures --file 52`.

**What actually happens.** Every single node (`item N, mobjs len 0`) in file
52's scene graph gets **zero** materials from `PartTables` — not a partial
mapping, all of them. Every segment-0x0E `Call` in the graph therefore hits
the `None => forget_texture()` branch (`mesh.rs:838-842`), which clears
`palette_offset` along with the texture image address
(`State::forget_texture`, `mesh.rs:543-549`). The trace shows the same
`0x1960` texture bind alternating between `palette_offset = Some(448)` and
`palette_offset = None` across its many occurrences in the file, entirely
depending on whether an interleaved 0x0E call (belonging to some *other*
joint's material, which this file cannot resolve at all) landed between the
nearest `G_LOADTLUT` and that occurrence's own `G_SETTIMG`. `romtool`'s dedup
key (RE-056) does determine *which* occurrence gets reported, but it is not
the root cause — the underlying condition (this file has no discovered
material table) makes the *majority* of occurrences fail, not just the one
`romtool` happens to check.

**File 86 is a partial version of the same thing**: most of its graph's
nodes do have `mobjs` (`len 1`/`len 2`), but at least one does not (`len 0`),
and its single `MissingPalette` failure falls on that one. Same mechanism,
smaller blast radius.

**What these files are.** `refs/ssb-decomp-re/src/relocData/` names files by
ID: `52_MVCommon.c`, `86_ITCommonObject.c`, `353_LinkSpecial2.c` (and,
corroborating RE-055, `39_IFCommonObject.c` — the same "Common"/interface
naming pattern as the LB-transition files). 52 and 86 are shared
menu/item-common asset containers, not fighter models — they may never have
had an `FTCommonPart`-style pairing record to find in the first place. File
353 is more notable: it is one of Link's own model files
(`224_LinkMain.c`/`225_LinkMainMotion.c`/`324_LinkModel.c` are his primary
files; `226_LinkSpecial1.c`, `353_LinkSpecial2.c`, `325_LinkSpecial3.c`,
`326_LinkBoomerangModel.c` look like per-special-move sub-models). If Link's
material table is a single record living in `324_LinkModel.c` rather than
duplicated into each special-move file, `PartTables::scan`'s requirement
that the `p_mobjsubs` pointer target the *same file* as the `DObjDesc` graph
(`mobj.rs:441-444`, `same == model`) would correctly fail to pair 353's own
graph, even though a real table exists for Link elsewhere. Not confirmed —
this is a plausible reading of the file layout, not a traced pairing.

**Reclassification.** This is not a texture-format or palette-decode bug
(R0.3's actual scope) and not what RE-056 guessed (a `romtool` dedup
artifact). It is a gap in recovered `MObj`/material-table pairings — exactly
what `PLAN.md` R0.7 ("Missing Material Tables") already tracks, whose
"Current evidence" section already flags an unreconciled 56-vs-71 count of
graphs without a table. These three files (especially 353, since it should
have Link's normal fighter pairing available somewhere in the archive) are
now concrete, reproducible test cases for that reconciliation.

**Confidence: high** that the mechanism (zero/partial `mobjs`, forget on
unresolved 0x0E, clearing an otherwise-valid palette) is correct — it was
observed directly via instrumented trace, not inferred. **Medium** on the
353-specific "table lives in a sibling file" explanation — plausible from
file naming and `PartTables::scan`'s known same-file constraint, but not
confirmed by actually locating Link's table record and checking which file
it names.

## RE-058 — `WPAttributes` is a second, unscanned pairing shape; the 353 sibling-file guess is retracted

**Question.** RE-057 guessed that file 353 (`LinkSpecial2`)'s material table
lives in a sibling file and is missed by `PartTables::scan`'s same-file
requirement. Reading the decompilation's own layout for 353 tests that guess
directly.

**353 already has its graph and table in the same file.** `353_LinkSpecial2.c`
declares its own `DObjDesc` arrays (`dLinkSpecial2_EntryWaveDObjDesc` at file
offset `0x3F8`, `dLinkSpecial2_EntryBeamDObjDesc` at `0x7B8`, plus a
`SpinAttackDObjDesc`) *and* its own `MObjSub **` tables in the same file
(`dLinkSpecial2_EntryWaveMObjSub` at `0x130`, pointing at
`dLinkSpecial2_gap_0x01B0`, an `MObjSub *` list). The same-file requirement
RE-057 blamed does not apply here — both halves of the pairing this graph
would need are already in 353. RE-057's specific guess about 353 is
**retracted**.

**What's actually missing: `PartTables` doesn't know about `WPAttributes`.**
`refs/ssb-decomp-re/src/wp/wptypes.h:36-45` defines a second struct with
exactly the shape `PartTables::scan` looks for — `void *data` (a `DObjDesc*`
when `WEAPON_FLAG_DOBJDESC` is set), `MObjSub ***p_mobjsubs`,
`AObjEvent32 **anim_joints`, `AObjEvent32 ***p_matanim_joints` — used for
weapon/projectile hitbox objects (a fighter's thrown items, projectiles, and
some special-move sub-models), completely separately from `FTCommonPart`
(fighter joints) and `MPGroundDesc` (stage layers), the only two shapes
`crates/ssb-rom/src/mobj.rs`'s comments and code currently mention. This is
a real, previously undocumented gap in what `PartTables` looks for, not a
bug in the same-file check — `PartTables::scan`'s core matching logic is
generic (it doesn't care what struct holds the adjacent pointers), so a
`WPAttributes` instance's `data`/`p_mobjsubs` fields are structurally the
same shape it already searches for; whether it's actually finding and
accepting them was not tested this pass.

**But the one confirmed `WPAttributes` instance argues against a fix here.**
`226_LinkSpecial1.c` defines `dLinkSpecial1_Boomerang_WeaponAttributes`, a
real `WPAttributes` instance, but it names `p_mobjsubs = NULL` outright
(explicit in the initializer) — its `data`/`anim_joints` point into file 325
(`LinkSpecial3`), not 353, and it has no material table by design. This is
one weapon (the boomerang), not the ones in 353 (`EntryWave`/`EntryBeam`/
`SpinAttack`), so it doesn't directly explain 353's failure — but it shows
`p_mobjsubs = NULL` is a legitimate, intentional value for a `WPAttributes`
instance, not automatically a discoverable-record gap. It is equally
possible that whatever `WPAttributes` (if any) names 353's `EntryWave`/
`EntryBeam`/`SpinAttack` graphs also has `p_mobjsubs = NULL` on real
hardware, in which case the game itself does not apply an `MObj` palette to
these draws and RE-057's classification of this as a "pairing gap" would be
wrong — the right explanation might instead be back in `mesh.rs`'s own
state handling (RE-056's original direction), not material-table discovery
at all. This pass did not locate the specific `WPAttributes` instance (if
one exists) for 353's own sub-models to settle it either way.

**Revised next step for R0.7.** Two independent threads, not one:

1. Whether `PartTables::scan` already structurally matches `WPAttributes`-shaped
   records elsewhere in the archive (fighters with projectiles: Samus,
   Fox/Falco-style specials do not exist in this roster, but Mario's
   fireball, Yoshi's egg, Pikachu's thunder-jolt, etc. are candidates) is an
   open, likely-valuable question independent of file 353.
2. File 353 specifically needs its actual `WPAttributes` instance (if any)
   located and its `p_mobjsubs` field read, before assuming a discoverable
   record exists at all.

**Confidence: high** that `WPAttributes` is a real, decomp-confirmed second
pairing shape `PartTables` does not currently search for — this is a direct
struct/instance read, not inference. **Low** that this explains file 353's
specific `MissingPalette` failures — the one instance checked argues against
it, and no `WPAttributes` instance naming 353's own sub-graphs was found.

## RE-059 — Two of file 353's three graphs paired via `EFDesc`, fixed (PLAN.md R0.7)

**Question.** RE-058 identified `WPAttributes` as a second pairing shape but
couldn't confirm it explains file 353's `MissingPalette` failures. Is there a
third mechanism, and does reading it all the way through actually fix
anything?

**A third struct: `EFDesc`.** File 353's three unpaired graphs
(`romtool mobj --file 353`: offsets `0x3F8`, `0x7B8`, `0x11C0`) are Link's
per-move entrance effects — the flash/warp-in a character does at match
start, plus his Spin Attack. Their code path
(`refs/ssb-decomp-re/src/ft/ftcommon/ftcommonentry.c:214-215` →
`efManagerLinkEntryWaveMakeEffect`/`efManagerLinkEntryBeamMakeEffect` →
`refs/ssb-decomp-re/src/ef/efmanager.c:1162-1219`) uses `EFDesc`
(`refs/ssb-decomp-re/src/ef/eftypes.h:11-24`), a struct with fields
`o_dobjsetup` immediately followed by `o_mobjsub` — the same adjacency
`PartTables::scan` already looks for, just under a third name, alongside
`FTCommonPart` (fighters) and `MPGroundDesc` (stages).

**Confirmed, non-null, and fully typed.**
`dEFManagerLinkEntryWaveEffectDesc`/`dEFManagerLinkEntryBeamEffectDesc` in
`efmanager.c` name `&llLinkSpecial2EntryWaveDObjDesc`/
`&llLinkSpecial2EntryWaveMObjSub` (and the `EntryBeam` equivalents) —
concrete, non-null, fully-typed C symbols, not raw bytes. Cross-checked
against `353_LinkSpecial2.c`'s own offset comments:
`EntryWaveDObjDesc @ 0x3F8`, `EntryWaveMObjSub @ 0x130`,
`EntryBeamDObjDesc @ 0x7B8`, `EntryBeamMObjSub @ 0x4F0`.

**Why `PartTables::scan` structurally cannot find these.** Unlike
`FTCommonPart`/`MPGroundDesc`, the `EFDesc` instances themselves live in the
game's static executable data (their ROM addresses are commented as fixed
`0x8012E4E4`-style addresses in `efmanager.c`, not relocData file offsets),
not in any relocData archive file. `PartTables::scan` only ever looks at
`file.extern_relocs` — the archive's own inter-file relocation records — so
a pointer the *executable* holds into an archive file leaves no reloc record
anywhere in the archive for the scanner to see. No amount of scanning the
archive more thoroughly can find this; it can only be read from the
decompilation and entered by hand, exactly the escape hatch
`PartTables::insert()` already exists for (used previously for stage
layers, whose `MPGroundDesc` records live in the archive but at a different
field offset than `FTCommonPart`).

**Fix.** `tools/romtool/src/main.rs`'s `load_all` now hand-inserts
`(353, 0x3F8, 0x130)` and `(353, 0x7B8, 0x4F0)`, gated the same way the
stage-layer inserts already are — only if `read_table` actually parses at
that offset for that graph's node count, so a wrong offset fails silently
closed rather than shipping garbage.

**Verified.** `romtool mobj --file 353`: pairings 56→58, both new graphs'
chain length matches their display-list demand exactly (0 mismatches).
`romtool textures --file 353`: 0 failures (was 1). Archive-wide
`romtool textures`: 618/647 packed (was 617), `MissingPalette` 4→3. `cargo
test --workspace`: 338 passing, unaffected (this fix lives in `romtool`, not
the library crate).

**What's still open.** File 353's third graph (`SpinAttackDObjDesc @
0x11C0`) is named by a `WPAttributes`
(`refs/ssb-decomp-re/src/wp/wplink/wplinkspinattack.c`:
`dWPLinkSpinAttackWeaponDesc` → `&llLinkMainSpinAttackWeaponAttributes` in
file 225), but unlike the boomerang's `WPAttributes` (RE-058), this instance
is **not yet typed** in the decompilation — it's still raw bytes at some
offset in `225_LinkMain.c`, so `p_mobjsubs` cannot be read from source.
Deliberately not inserted; inserting an unverified table offset would risk
shipping a wrong pairing rather than none. Files 52 (`MVCommon`) and 86
(`ITCommonObject`) remain completely untouched by this entry — nothing here
suggests they're `EFDesc`- or `WPAttributes`-shaped; they still need their
own tracing.

**Confidence: high**, on all counts — the `EFDesc` struct read, the two
inserted offsets, and the fix are all independently confirmed by both the
decompilation source and a real `romtool` run against the ROM, not
inference.
