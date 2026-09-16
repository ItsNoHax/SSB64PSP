# TODO — Discovered Future Work

Work that is **discovered but not currently scheduled** as the active task in
`STATUS.md`. Uses `PLAN.md`'s milestone names (R0–R3 rendering gate, G0–G5
post-combat roadmap). Completed items are not archived here — they live in
their owning task spec (`plans/rendering/*.md`, `plans/gameplay/*.md`) and
evidence record (`docs/evidence/re/RE-XXX.md`).

Do not start G0+ work until R0–R3 are complete (`AGENTS.md`, `PLAN.md`).
Do not duplicate `PLAN.md`/`STATUS.md`'s active-task detail here.

---

## Rendering-Correctness Remainders (feeds PLAN.md R0.x)

### WPAttributes pairing shape
Target: R0.7 (COMPLETE; documented remainder)
Status: DEFERRED
Evidence: RE-058, RE-059
Reason: `PartTables::scan` has no case for the `WPAttributes` shape (weapon/
projectile sub-objects). The one confirmed instance (Link's boomerang) has
`p_mobjsubs = NULL` by design, so there is no known live bug — revisit only
if a new `WPAttributes` instance with real sub-objects is found.

### Material animation — stage texture-frame/UV tracks and costume PaletteID
Target: R0.10 (`VERIFYING`)
Status: IN_PROGRESS (partial)
Evidence: RE-086, RE-089–095, RE-211
Reason: 33 palette-cycling scripts are resolved, packed and ticked. Stage
texture-frame/UV tracks and the 200/441 fighter costume scripts carrying
`PaletteID` remain unconsumed.

### Fighter costume palettes beyond costume 0
Target: R0.11 (COMPLETE; documented remainder) / future asset work
Status: DEFERRED
Evidence: RE-096, RE-261
Reason: costume zero resolves palette ID and all five colour/light tracks.
Runtime costume selection and packing all costumes is out of R0.11's scope.

### Independent fighter animation validation
Target: R0.10
Status: DEFERRED
Reason: stage animation already derives expected frame state from decomp/ROM
(RE-050/051/052/142). Fighter costume/material animation does not yet have
the equivalent independent check.

### Texture streaming
Target: R0.11 / R1
Status: DEFERRED
Evidence: RE-076
Reason: archive-wide packed textures measure 1170.9 KiB (1.7x the ~700 KiB
budget), but a direct per-scene measurement found the worst realistic case
(Dream Land + 4 largest fighters) at only 217.1 KiB — likely an undercount
until R0.7's remaining unpaired `MObj` graphs close. Re-measure then; the
planned per-scene `AssetArena` (`docs/memory.md`) may already be sufficient
but is untested.

### Scene dependency graph
Target: R0.11 / asset pipeline
Status: DEFERRED
Reason: no explicit `scene → nodes → materials → textures → palettes`
dependency graph exists yet; needed before texture streaming can be
re-measured with confidence.

### Strict rendering mode
Target: R1 / R3
Status: DEFERRED
Reason: no fail-fast mode exists for unresolved texture/missing palette/
unknown transform; would help catch regressions earlier than a silent
fallback.

### Fighter face texture repeats/corrupts (Mario, at least)
Target: R2 (blocks full confidence in "representative fighters render",
already marked complete)
Status: OPEN
Evidence: RE-272, RE-274
Reason: Mario's eye/eyebrow texture (file 296, `Ci4 32x32`) is clean in the
raw ROM dump but renders as a repeating, aliased pattern with red patches
near the ears on both PPSSPP and physical PSP hardware — same renderer code
path on both, so not hardware-specific and not new. The regression-capture
golden methodology cannot detect this class of bug by construction (it only
checks PSP-vs-PPSSPP self-consistency, not fidelity to the source texture),
so other fighters' passing captures are not evidence they are unaffected.
RE-274 traced the exact draw call: one self-contained 24-triangle primitive
(file 296, node 8, dl `0x1990`) whose own freshly-loaded vertices carry a UV
span 3.46x/1.19x the packed mirror+clamp tile, ruling out cross-node texture
leakage and confirming texture decode/mirror-baking are correct. Two
hypotheses remain undistinguished: a faithfully-reproduced ROM quirk needing
a `TEXTURE_FILTER_CORRECTIONS`-style named fix (RE-263/264 precedent), or a
real addressing/scale bug specific to this overscan magnitude. Needs an
accurate N64 reference render (real hardware or `angrylion-rdp-plus`) to
tell which.

---

## Deferred Work Behind the Rendering Gate (PLAN.md G1–G5)

Blocked until R0, R1, R2 and R3 are all complete and G0 (first combat
vertical slice) has landed. See `plans/gameplay/G1.md`–`G5.md` for each
milestone's scope; these are additional discovered items within that scope.

### G1 — Full Combat
- [ ] All 12 original characters (Mario, Fox, Donkey Kong, Samus, Luigi, Link, Yoshi, Captain Falcon, Kirby, Pikachu, Jigglypuff, Ness)
- [ ] Unlockable characters (4 + Fighting Polygon Team + Giant DK + Metal Mario + Master Hand)
- [ ] CPU AI
- [ ] Items (spawn, behavior, pickup, effects)

### G2 — Complete Match Systems
- [ ] All original stages loadable in a match (41 including bonus/1P) — the viewer already browses stage data, a match does not yet select one
- [ ] Game modes (VS, 1P, Training, etc.)
- [ ] Stocks, timers, win conditions, match transitions

### G3 — Menus and Persistence
- [ ] Title screen, character select, mode select, options, pause menu, results screen, credits
- [ ] PSP-native save system (unlocks, records, settings, progression)

### G4 — Audio
- [ ] Build-time VADPCM decode for 439 samples (117 + 322 waveforms)
- [ ] Sequence conversion for 47 music sequences (ALSeqFile compressed-MIDI)
- [ ] Software mixer on dedicated thread (PSP audio block ≈ 23 ms > 16.67 ms frame)
- [ ] SFX engine (FGM voice IDs from `gmFGMVoiceID`)
- [ ] Music playback with correct sequencing/timing, volume/mixing
- [ ] Media Engine acceleration (after CPU implementation stable)

### G5 — Final Optimization
- [ ] VFPU acceleration for hot math paths (matrix mul, transforms, collision, animation)
- [ ] GPU batching + state sorting beyond build-time material merge
- [ ] Memory optimization (arenas, pools)
- [ ] Audio optimization
- [ ] Profile-guided optimization

Rendering-specific performance work (frame time, GE/CPU bottlenecks, VRAM
measurement) belongs to `PLAN.md` R3, not here.

---

## RE-XXX Open Questions

Investigations recorded in `docs/evidence/re/` with `Status: OPEN`. See
`docs/evidence/INDEX.md` to confirm current status before trusting this list.

### RE-008 — C-button mapping
Target: G0 (before combat slice — input mapping is not part of the rendering gate)
Status: Placeholder (C-Up→Triangle, C-Down→Square; C-Left/C-Right unmapped).
Blocker: needs `ft/ftkey.c` and menu input paths read against the decomp.

### RE-009 — PSP nub deadzone
Target: G0
Status: Deadzone 20 nub units, linear rescale to ±80 — a guess, not measured.
Blocker: needs measurement against real PSP nub and decomp thresholds.

### RE-010 — MObjSub unknown fields
Target: R0.6 / R0.7
Status: Not consumed; converter reads only named fields.
Blocker: revisit if materials look wrong. Decomp can answer by finding readers.

### RE-011 — Level of detail selection
Target: R3 — do not force a tier before R3 measures whether it is needed
Status: Setter (`sGCDetailLevel`) not traced. Likely tied to player count/options.

---

## Combat Vertical Slice (PLAN.md G0 — blocked, not current milestone)

Recorded so the shape of the first combat slice isn't lost. None of this may
start until `PLAN.md` R0–R3 are complete and `STATUS.md` records G0 as
eligible.

- [ ] Grounded attack end-to-end — input → hitbox → hurtbox → damage → knockback
- [ ] Hitbox/hurtbox system from `FTAttributes` descriptors (RE-032, only 45 leading scalars decoded)
- [ ] Damage/knockback physics ported from `ftphysics.c`
- [ ] Opponent + match loop — second fighter, stock system, blast zones, KO
- [ ] Stage loader — match selects stage, not viewer browse
- [ ] Fighter states: attacks (Attack11-13, AttackS3/4, AttackHi3/Lw3, AttackAir F/B/N/Hi/Lw, Specials), grab/throw, shield, damage/hitstun/knockback, ledge grab/climb/attack, tech/roll/air dodge
- [ ] Systems: hitbox/hurtbox collision, hitlag/hitstun frames, knockback velocity (weight/damage/angle/DI), blast zone + KO, stock + respawn, match timer/sudden death

---

## Technical Debt / Refactoring

- [ ] **Extern relocation runtime loader** — pack records them zeroed; need loader to patch at scene load
- [ ] **AssetArena implementation** — contiguous per-scene block sized by dependency closure
- [ ] **GameArena / FrameArena / ObjectPool** — explicit allocators (not implemented yet)
- [ ] **VFPU math module** — after profiling identifies hot paths
- [ ] **Coordinate conversion hardening** — on-hardware confirmation (RE-004, RE-005)
- [ ] **PSP GU backend completion** — textured mesh path, material state, CLUT handling
- [ ] **PSP audio backend** — `sceAudio` mixer on dedicated thread

---

## Notes

- Items are **not prioritized** — this is a holding area, not a schedule.
- Current focus and single active task: `STATUS.md`.
- When an item here becomes active work, reflect it in `STATUS.md`, not here.
- Update this file when new work is discovered during implementation; remove
  an item once its owning task spec or an evidence record fully covers it.
