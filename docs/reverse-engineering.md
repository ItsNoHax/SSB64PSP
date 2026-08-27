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
