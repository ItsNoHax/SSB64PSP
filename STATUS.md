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

Last completed: `RE-276` — a live N64 reference render (Mupen64Plus/Rice,
reached via a temporary patched-and-reverted `refs/ssb-decomp-re` build) of
Mario's face at RE-274's exact wide-UV overscan renders clean, no repeating
pattern — evidence against "faithfully-reproduced ROM quirk" and for "real
PSP-side addressing/scale bug", closing the two-hypothesis question RE-274
left open. A pure RDRAM scene warp could not win the race against
`scManagerRunLoop`'s once-per-scene dispatch read (traced and documented),
and menu-driven navigation flaked too heavily under this session's host
load, so the working route was a temporary decomp-source patch, rebuilt and
reverted after one capture. Root cause of the bug itself is still open,
scoped in `TODO.md`. Before that, `RE-275` ported RE-216's scripted
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
reference render, but the specific bug in this project's own addressing code
is not yet found. See `TODO.md`. Does not need physical-hardware access.

Current verification baseline: `cargo fmt --all --check` clean; `cargo test
--workspace` 616 passed; all 22 deterministic goldens exact; effects 46/46
manager objects, 35/35 transform + 24/26 material animations, 160/160
particle scripts; 109 billboards, 0 anomalies; 11/11 framebuffer transitions;
`romtool texgen --verify` pass; plain feature-free `cargo psp --release`
pass; strict Clippy not clean (3 pre-existing `needless_range_loop` lints in
`coord.rs`/`matanim.rs`/`objanim.rs`, unrelated to this work). No code
changed this session (RE-276 only touched a temporary, reverted
`refs/ssb-decomp-re` checkout, not this project's own crates).

Required next action: resume the physical R2 matrix if PSP-1000 hardware
becomes available; otherwise record the concrete access blocker. Separately,
find RE-272/274's actual root cause now that RE-276 narrowed it to a real
PSP-side addressing/scale bug: most likely this project's own
`mirror_extend`/addressing code for overscan periods beyond what RE-220/
RE-221 already handle, or how `meshdraw::bind_texture` sets up the GE wrap
state for this overscan magnitude (>3x the tile). Do not reach for a
`TEXTURE_FILTER_CORRECTIONS`-style named acceptance — RE-276 found evidence
against that hypothesis. Does not require physical-PSP-hardware access. Do
not start R3 or combat before R2's physical matrix is closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-269, RE-270, RE-271, RE-272,
RE-273, RE-274, RE-275, RE-276 (see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–276). Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-271 code (no code changes since; RE-273's plain EBOOT hash
matched byte-for-byte). Pack `a79b0aa9...5935f` (28,531.3 KiB). Plain
feature-free EBOOT `66ce9869...a5336`.
