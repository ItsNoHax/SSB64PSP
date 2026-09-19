# TODO — Discovered Future Work

Work that is **discovered but not currently scheduled** as the active batch
in `STATUS.md`, and not already represented by a `PLAN.md` milestone bullet.
Completed items are not archived here — they live in
`docs/porting-status.md` and their evidence record
(`docs/evidence/re/RE-XXX.md`).

Do not duplicate `PLAN.md`'s roadmap or `STATUS.md`'s active-batch detail
here. Milestone-shaped backlog (all fighters, all stages, items, CPU AI,
audio, etc.) belongs to `PLAN.md` `P2`–`P4`, not this file.

---

## Rendering-Correctness Remainders

### WPAttributes pairing shape
Status: DEFERRED
Evidence: RE-058, RE-059
Reason: `PartTables::scan` has no case for the `WPAttributes` shape (weapon/
projectile sub-objects). The one confirmed instance (Link's boomerang) has
`p_mobjsubs = NULL` by design, so there is no known live bug — revisit only
if a new `WPAttributes` instance with real sub-objects is found.

### Material animation — stage texture-frame/UV tracks and costume PaletteID
Status: IN_PROGRESS (partial)
Evidence: RE-086, RE-089–095, RE-211
Reason: 33 palette-cycling scripts are resolved, packed and ticked. Stage
texture-frame/UV tracks and the 200/441 fighter costume scripts carrying
`PaletteID` remain unconsumed.

### Fighter costume palettes beyond costume 0
Status: DEFERRED
Evidence: RE-096, RE-261
Reason: costume zero resolves palette ID and all five colour/light tracks.
Runtime costume selection and packing all costumes is future asset work.

### Independent fighter animation validation
Status: DEFERRED
Reason: stage animation already derives expected frame state from decomp/ROM
(RE-050/051/052/142). Fighter costume/material animation does not yet have
the equivalent independent check.

### Texture streaming
Status: DEFERRED
Evidence: RE-076
Reason: archive-wide packed textures measure 1170.9 KiB (1.7x the ~700 KiB
budget), but a direct per-scene measurement found the worst realistic case
(Dream Land + 4 largest fighters) at only 217.1 KiB — likely an undercount
until remaining unpaired `MObj` graphs close. Re-measure then; the planned
per-scene `AssetArena` (`docs/memory.md`) may already be sufficient but is
untested.

### Scene dependency graph
Status: DEFERRED
Reason: no explicit `scene → nodes → materials → textures → palettes`
dependency graph exists yet; needed before texture streaming can be
re-measured with confidence.

### Strict rendering mode
Status: DEFERRED
Reason: no fail-fast mode exists for unresolved texture/missing palette/
unknown transform; would help catch regressions earlier than a silent
fallback.

---

## Deferred Hardware Acceptance

Status: DEFERRED (explicit user instruction)
Evidence: RE-284, RE-288

### PSP-1000 real-content confirmation
Reason: RE-288 confirmed the current ~25.6 MiB pack fails to fit in a
PSP-1000's 32 MiB RAM (`MEMSIZE=1` is ignored on that hardware class) —
`LoadError::OutOfMemory`, clean fallback to the placeholder tetrahedron, zero
exceptions. Confirming the *renderer itself* on PSP-1000 needs a build that
actually fits (a reduced-content or streaming pack), which doesn't exist
yet. Revisit once a real game scene, not the full asset-viewer content set,
is what actually loads at runtime.

### Second physical unit at the 30-minute sustained-run duration
Reason: RE-284 sustained 30 minutes on one unit (PSP Slim) with zero
exceptions; RE-273 sustained 10 minutes on a second unit (PSP-3000). No unit
has run the full 30-minute duration with real pack content loaded on a
second unit. A unit that can actually load the pack (Slim/2000/3000-class,
64 MiB) is needed to repeat RE-284's methodology on a second unit.

---

## RE-XXX Open Questions

Investigations recorded in `docs/evidence/re/` with `Status: OPEN`. See
`docs/evidence/INDEX.md` to confirm current status before trusting this list.

### RE-008 — C-button mapping
Status: Placeholder (C-Up→Triangle, C-Down→Square; C-Left/C-Right unmapped).
Blocker: needs `ft/ftkey.c` and menu input paths read against the decomp.

### RE-009 — PSP nub deadzone
Status: Deadzone 20 nub units, linear rescale to ±80 — a guess, not measured.
Blocker: needs measurement against real PSP nub and decomp thresholds.

### RE-010 — MObjSub unknown fields
Status: Not consumed; converter reads only named fields.
Blocker: revisit if materials look wrong. Decomp can answer by finding readers.

### RE-011 — Level of detail selection
Status: Setter (`sGCDetailLevel`) not traced. Likely tied to player count/
options. Revisit during `P5` profiling, not before.

---

## Technical Debt / Refactoring

- [ ] **Extern relocation runtime loader** — pack records them zeroed; need loader to patch at scene load
- [ ] **AssetArena implementation** — contiguous per-scene block sized by dependency closure
- [ ] **GameArena / FrameArena / ObjectPool** — explicit allocators (not implemented yet)
- [ ] **VFPU math module** — after `P5` profiling identifies hot paths
- [ ] **Coordinate conversion hardening** — on-hardware confirmation (RE-004, RE-005)
- [ ] **PSP audio backend** — `sceAudio` mixer on dedicated thread (`P4`)

---

## Notes

- Items are **not prioritized** — this is a holding area, not a schedule.
- Current focus and single active batch: `STATUS.md`.
- When an item here becomes active work, reflect it in `STATUS.md`, not here.
- Update this file when new work is discovered during a batch; remove an
  item once it's covered by a completed batch or an evidence record.
