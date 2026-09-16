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

Last completed: `RE-273` — physical confirmation on a second hardware unit
(PSP-3000): scene1, depth-mask diagnostic (RE-271's fix holds), Mario
fighter capture, 10-minute sustained run, zero exceptions throughout. That
Mario capture surfaced `RE-272`: a real, pre-existing fighter-face-texture
rendering bug (reproduces on PPSSPP too, so not hardware-specific and not
caused by this session) that the regression-capture golden methodology
cannot detect by construction — open, root cause not yet found, scoped in
`TODO.md`.

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed — neither the Slim nor
the PSP-3000 tested so far is in that RAM class. No dedicated capture scenes
exist yet for full stage/effect coverage or runs longer than 10 minutes.
None of these block R2.2 (closed) — they gate the *physical* R2 matrix only,
and are deliberately deferred until the game structure grows beyond the
asset viewer. Separately, `RE-272`'s face-texture bug is open and
unblocked — it needs its own scoped investigation (see `TODO.md`), not
physical-hardware access.

Current verification baseline: `cargo fmt --all --check` clean; `cargo test
--workspace` 616 passed; all 22 deterministic goldens exact; effects 46/46
manager objects, 35/35 transform + 24/26 material animations, 160/160
particle scripts; 109 billboards, 0 anomalies; 11/11 framebuffer transitions;
`romtool texgen --verify` pass; plain feature-free `cargo psp --release`
pass; strict Clippy not clean (3 pre-existing `needless_range_loop` lints in
`coord.rs`/`matanim.rs`/`objanim.rs`, unrelated to this work).

Required next action: resume the physical R2 matrix if PSP-1000 hardware
becomes available; otherwise record the concrete access blocker. Separately,
open a scoped investigation for `RE-272`'s face-texture rendering bug
(candidates: UV/tile addressing, texture scale, mip interaction — not yet
distinguished) when picked up; it does not require physical-hardware access
to investigate (root cause is likely in `mesh.rs`/pack conversion or
`meshdraw.rs`, reproducible under PPSSPP alone). Do not start R3 or combat
before R2's physical matrix is closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-269, RE-270, RE-271, RE-272,
RE-273 (see [docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full
R2.2/physical chain, RE-240–273). Toolchain note: the global `cargo-psp`
install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), [docs/psplink.md](docs/psplink.md)

Current build: RE-271 code (no code changes since; RE-273's plain EBOOT hash
matched byte-for-byte). Pack `a79b0aa9...5935f` (28,531.3 KiB). Plain
feature-free EBOOT `66ce9869...a5336`.
