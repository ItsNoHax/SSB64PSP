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
stages and effect files, which use a different setup path. Stage tables also
reach chains in a *different* archive file through extern relocations; those
slots parse but read back empty. Only costume 0 and animation frame 0 are
taken, since `palette_id` and `texture_id` are runtime counters.

**Confidence: certain.** The pairing is read from a struct that names both
sides, and two independent checks — chain length against display-list demand,
and every offset against the decomp — agree completely.

![Samus with her Varia suit palettes](images/m4-fighter-materials.png)

The same frame as RE-026, same 326 triangles and 25 draws. 707 µs, 60 FPS.

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
* `view 362x272` — `coord::pillarboxed_viewport()` confirmed on-device.

**Caveat.** The scene is four triangles and one fighter's worth of physics, so
13us says nothing about how a real match will perform. Its value is as a
**baseline**: the platform layer, clock and submission path cost essentially
nothing, so future frame time can be attributed to the game rather than to
scaffolding.

**Confidence: high** for the measurement; explicitly **not** a performance
prediction. Real PSP hardware is ~333 MHz against an emulator on a desktop CPU
— these numbers do not transfer (plan §37).
