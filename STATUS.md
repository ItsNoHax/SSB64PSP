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

Last completed: `RE-280` — **found RE-272/274's Mario-face bug's actual root
cause**: `ssb_rom::psp_texture`'s `Ci4` (`PsmT4`) packing (`encode_level`,
`pack_indexed`'s straight ROM-copy path, `pad_edge_repeat_nibbles`) packs two
4-bit texels per byte high-nibble-first, "matching the N64 order" — but
PPSSPP's `PsmT4` reader (the deterministic golden source this project already
trusts for GE addressing) wants the opposite order, low nibble first. Built a
permanent rig (`psp/src/tri_addr_diag.rs`, `tri_addr_diag_probe` feature)
drawing real triangles with a wide interpolated UV delta through the actual
`Ci4`/CLUT bind path plus a `Psm8888` control with identical geometry: the
control read back a perfect linear ramp (ruling out interpolation precision
entirely) while the `Ci4` rows — both interpolated and, critically, *fixed
non-interpolated* single-coordinate samples — mismatched with a periodic
every-other-texel saw pattern. Swapping the nibble order in the rig's own
synthetic texture made all 12 fixed probes read exactly the predicted value
(was 2/12) and turned the corrupted interpolated readback into the same clean
ramp the control showed — decisive confirmation. Fix identified (three
coordinated sites in `psp_texture.rs`) but **not yet applied**: real,
non-trivial scope (touches every `Ci4` texture the archive packs, requires
rebuilding `assets/generated/ssb64.pak` and refreshing whichever of the 22
goldens change, each with its semantic delta explained). See `TODO.md`.
Before that, `RE-279` confirmed RE-274's real primitive is correctly routed
through `meshdraw`'s signed-UV float path (`needs_float_uv=true`), ruling out
the unsigned-16-bit UV reinterpretation bug as a candidate. Before that,
`RE-278` — built a permanent measurement rig
(`psp/src/addr_diag.rs`, `addr_diag_probe` feature, same
`texgen_normal_diagnostic_*`/`depth_mask_diagnostic` shape) sampling a
synthetic 64-texel mirror+clamp texture at fixed, non-interpolated raw
texel-address coordinates spanning RE-274's own measured range plus two
half-texel probes straddling the clamp boundary, under both `Nearest` and
`Linear` filtering, captured with PPSSPPHeadless. Every probe read exactly
the predicted value, including both half-texel probes right at the clamp
edge — ruling out "the GE mis-samples a known fixed coordinate near this
boundary" as RE-272's cause, on top of RE-277's ruling out of the addressing
formula itself. Before that, `RE-277` — ran RE-274's own named next step: a dense
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
asset viewer. Separately, `RE-272`'s face-texture bug is open, unblocked, and
now root-caused: `RE-280` found `ssb_rom::psp_texture`'s `Ci4`/`PsmT4`
packing uses the wrong nibble order for the PSP GE (high-nibble-first,
matching the N64 source, when PPSSPP's reader — and plausibly real hardware
— wants low-nibble-first). Confirmed decisively (swap-and-reconfirm: 12/12
fixed-probe matches, was 2/12). Fix identified, not yet applied — see
`TODO.md` for the three coordinated `psp_texture.rs` sites and the expected
golden-refresh scope. Does not need physical-hardware access.

Current verification baseline: `cargo fmt --all --check` clean; `cargo test
--workspace` 617 passed; all 22 deterministic goldens exact; effects 46/46
manager objects, 35/35 transform + 24/26 material animations, 160/160
particle scripts; 109 billboards, 0 anomalies; 11/11 framebuffer transitions;
`romtool texgen --verify` pass; plain feature-free `cargo psp --release`
pass; strict Clippy not clean (3 pre-existing `needless_range_loop` lints in
`coord.rs`/`matanim.rs`/`objanim.rs`, unrelated to this work). This session's
only production-code change is the new, permanent, off-by-default
`tri_addr_diag_probe` diagnostic (`psp/src/tri_addr_diag.rs`,
`psp/Cargo.toml`, `psp/src/main.rs`), alongside the earlier `addr_diag_probe`
one; plain feature-free `cargo psp --release` and the psp crate's own `cargo
fmt --check` both reconfirmed clean after adding it. No change has yet been
made to `crates/ssb-rom/src/psp_texture.rs` itself (RE-280's fix is
identified, not applied) or to `assets/generated/ssb64.pak`. RE-279's probe
was added to and reverted from `crates/ssb-rom/src/pack.rs`; RE-277's probe
test was added to and reverted from `crates/ssb-rom/src/n64_addressing.rs`;
RE-276 only touched a temporary, reverted `refs/ssb-decomp-re` checkout.

Required next action: resume the physical R2 matrix if PSP-1000 hardware
becomes available; otherwise record the concrete access blocker. Separately,
apply RE-280's identified fix for RE-272's root cause: flip the nibble order
at `psp_texture.rs`'s three coordinated `Ci4`/`PsmT4` sites (`encode_level`,
`pack_indexed`'s straight ROM-copy path, `pad_edge_repeat_nibbles`) together,
update `ci4_packing_round_trips_through_swizzle_and_clut`'s test and its
`unpack_ci4` helper to the corrected convention, rebuild
`assets/generated/ssb64.pak`, then re-run the full regression-capture matrix
and refresh whichever of the 22 goldens change — each with its semantic delta
explained (texture corrected, not a regression), per the visual-regression
skill's own rule. Confirm the fix visually against Mario's face specifically
(the original RE-272 complaint) before considering this closed. Does not
require physical-PSP-hardware access. Do not start R3 or combat before R2's
physical matrix is closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-269, RE-270, RE-271, RE-272,
RE-273, RE-274, RE-275, RE-276, RE-277, RE-278, RE-279, RE-280 (see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–280). Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-271 code (no code changes since; RE-273's plain EBOOT hash
matched byte-for-byte). Pack `a79b0aa9...5935f` (28,531.3 KiB). Plain
feature-free EBOOT `66ce9869...a5336`.
