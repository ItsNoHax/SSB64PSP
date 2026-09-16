# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: physically confirm the parts of R2's matrix not yet
covered on real hardware. All 12 playable fighters, the depth-mask
diagnostic, and a 10-minute sustained run are confirmed on PSP Slim/6.6.1;
the depth-mask diagnostic, a Mario fighter capture, and a second 10-minute
sustained run are now also confirmed on a second unit, PSP-3000. PSP-1000
coverage (32 MiB RAM, distinct from both units tested so far) and
broader/longer coverage remain, deliberately deferred until the game
structure grows beyond the current asset viewer.

Last completed: `RE-278` — built a permanent measurement rig
(`psp/src/addr_diag.rs`, `addr_diag_probe` feature, same
`texgen_normal_diagnostic_*`/`depth_mask_diagnostic` shape) sampling a
synthetic 64-texel mirror+clamp texture at fixed, non-interpolated raw
texel-address coordinates spanning RE-274's own measured range plus two
half-texel probes straddling the clamp boundary, under both `Nearest` and
`Linear` filtering, captured with PPSSPPHeadless. Every probe read exactly
the predicted value, including both half-texel probes right at the clamp
edge — ruling out "the GE mis-samples a known fixed coordinate near this
boundary" as RE-272's cause, on top of RE-277's ruling out of the addressing
formula itself. Explicitly does not test per-pixel triangle interpolation
(perspective-correct UV across real geometry, not a fixed 2D-sprite sample)
or the real primitive's `Ci4`/CLUT paletted path — named as the next step in
`TODO.md`. Before that, `RE-277` — ran RE-274's own named next step: a dense
per-texel sweep (not just the 19 vertex extremes RE-220/RE-221's
archive-wide census already covered) of `n64_addressing`'s hardware
reference model against this project's PSP-lowering model, across both axes
of RE-272/274's exact wide-overscan Mario-face primitive (file 296, node 8),
including well past the drawn rect's far edge. Zero mismatches on both axes
— the addressing *formula* is now ruled out three independent ways (RE-220's
archive-wide census, RE-276's live N64 capture, this dense sweep). Temporary
probe test added to and reverted from `crates/ssb-rom/src/n64_addressing.rs`
(no permanent code change from RE-277 itself). Before that, `RE-276` — a live N64
reference render (Mupen64Plus/Rice, reached via a temporary
patched-and-reverted `refs/ssb-decomp-re` build) of Mario's face at RE-274's
exact wide-UV overscan renders clean, no repeating pattern — evidence
against "faithfully-reproduced ROM quirk" and for "real PSP-side
addressing/scale bug", closing the two-hypothesis question RE-274 left open.
A pure RDRAM scene warp could not win the race against
`scManagerRunLoop`'s once-per-scene dispatch read (traced and documented),
and menu-driven navigation flaked too heavily under this session's host
load, so the working route was a temporary decomp-source patch, rebuilt and
reverted after one capture. Before that, `RE-275` ported RE-216's scripted
Mupen64Plus capture driver to the RMG flatpak to drive `angrylion-rdp-plus`
(a cycle-accurate RDP reference renderer found locally available there, not
in the M64Py flatpak RE-151/RE-216 already used); root-caused and fixed a
`PluginStartup` segfault (RTLD_GLOBAL symbol interposition into Core's own
data segment), but hit a further, unsolved headless-GL hurdle
(`SDL_CreateWindow`/GLX) — closed as superseded once RE-276's Rice-based
route worked instead. Before that, `RE-273` physically confirmed the
renderer on a second hardware unit (PSP-3000): scene1, depth-mask diagnostic
(RE-271's fix holds), Mario fighter capture, 10-minute sustained run, zero
exceptions throughout. That Mario capture surfaced `RE-272`: a real,
pre-existing fighter-face-texture rendering bug (reproduces on PPSSPP too,
so not hardware-specific and not caused by this session) that the
regression-capture golden methodology cannot detect by construction — open,
`RE-274` traced it to one draw call, root cause still not found, scoped in
`TODO.md`.

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed — neither the Slim nor
the PSP-3000 tested so far is in that RAM class. No dedicated capture scenes
exist yet for full stage/effect coverage or runs longer than 10 minutes.
None of these block R2.2 (closed) — they gate the *physical* R2 matrix only,
and are deliberately deferred until the game structure grows beyond the
asset viewer. Separately, `RE-272`'s face-texture bug is open and
unblocked — `RE-274` traced it to one self-contained, wide-UV draw call
(file 296, node 8, dl `0x1990`) and ruled out cross-node texture leakage and
mirror-bake corruption; `RE-276` narrowed root cause to "real PSP-side
addressing/scale bug" (not a ROM-faithful filtering gap) via a live N64
reference render; `RE-277` then ruled out the addressing formula itself
(dense per-texel sweep, zero divergence on both axes); `RE-278` ruled out
fixed-coordinate GE sampling too (a permanent probe rig, zero mismatches
including at the Linear clamp-boundary half-texel positions). Remaining
scope: triangle-interpolation precision and the real `Ci4`/CLUT path, not
yet measured. See `TODO.md`. Does not need physical-hardware access.

Current verification baseline: `cargo fmt --all --check` clean; `cargo test
--workspace` 616 passed; all 22 deterministic goldens exact; effects 46/46
manager objects, 35/35 transform + 24/26 material animations, 160/160
particle scripts; 109 billboards, 0 anomalies; 11/11 framebuffer transitions;
`romtool texgen --verify` pass; plain feature-free `cargo psp --release`
pass; strict Clippy not clean (3 pre-existing `needless_range_loop` lints in
`coord.rs`/`matanim.rs`/`objanim.rs`, unrelated to this work). This session's
only production-code change is the new, permanent, off-by-default
`addr_diag_probe` diagnostic (`psp/src/addr_diag.rs`, `psp/Cargo.toml`,
`psp/src/main.rs`); plain feature-free `cargo psp --release` and the psp
crate's own `cargo fmt --check` both reconfirmed clean after adding it.
RE-277's probe test was added to and reverted from
`crates/ssb-rom/src/n64_addressing.rs`; RE-276 only touched a temporary,
reverted `refs/ssb-decomp-re` checkout.

Required next action: resume the physical R2 matrix if PSP-1000 hardware
becomes available; otherwise record the concrete access blocker. Separately,
find RE-272/274's actual root cause now that RE-277/RE-278 have ruled out
both the addressing formula (three independent checks: RE-220's archive-wide
census, RE-276's live N64 capture, RE-277's dense per-texel sweep) and
fixed-coordinate GE sampling including the `Linear` clamp-boundary blend
(RE-278). What remains untested: per-pixel triangle-interpolation precision
across real geometry with a comparable per-triangle UV delta (not a fixed 2D
sprite sample), and the real primitive's actual `Ci4`/CLUT paletted path
(RE-278's rig used plain `Psm8888`). Next step: extend `addr_diag_probe` (or
a sibling rig) to draw a small number of real triangles with authored
`GU_TEXTURE_16BIT` UVs through the same `Ci4` `bind_texture` path, comparing
per-pixel output against the addressing model. Do not reach for a
`TEXTURE_FILTER_CORRECTIONS`-style named acceptance until that is measured.
Does not require physical-PSP-hardware access. Do not start R3 or combat
before R2's physical matrix is closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-269, RE-270, RE-271, RE-272,
RE-273, RE-274, RE-275, RE-276, RE-277, RE-278 (see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–278). Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-271 code (no code changes since; RE-273's plain EBOOT hash
matched byte-for-byte). Pack `a79b0aa9...5935f` (28,531.3 KiB). Plain
feature-free EBOOT `66ce9869...a5336`.
