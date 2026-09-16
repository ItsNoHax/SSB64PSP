# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: physically confirm the parts of R2's matrix not yet
covered on real hardware. All 12 playable fighters, the depth-mask
diagnostic, and a 10-minute sustained run are confirmed on PSP Slim/6.6.1.
PSP-1000 coverage and broader/longer coverage remain, deliberately deferred
until the game structure grows beyond the current asset viewer.

Last completed: `RE-271` — exhaustive fighter physical matrix; found and
fixed `draw_triangles`'s static-buffer GE race (same class `draw_line_strip`
was already fixed for); 10-minute sustained-run evidence.

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed. No dedicated capture
scenes exist yet for full stage/effect coverage or runs longer than 10
minutes. None of these block R2.2 (closed) — they gate the *physical* R2
matrix only, and are deliberately deferred until the game structure grows
beyond the asset viewer.

Current verification baseline: `cargo fmt --all --check` clean; `cargo test
--workspace` 616 passed; all 22 deterministic goldens exact; effects 46/46
manager objects, 35/35 transform + 24/26 material animations, 160/160
particle scripts; 109 billboards, 0 anomalies; 11/11 framebuffer transitions;
`romtool texgen --verify` pass; plain feature-free `cargo psp --release`
pass; strict Clippy not clean (3 pre-existing `needless_range_loop` lints in
`coord.rs`/`matanim.rs`/`objanim.rs`, unrelated to this work).

Required next action: resume the physical R2 matrix if PSP hardware is
available (PSP-1000 coverage, broader stage/effect scenes, longer runs);
otherwise record the concrete access/PSP-1000-memory blocker. Do not start
R3 or combat before R2's physical matrix is closed.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-269, RE-270, RE-271 (see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–271). Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), [docs/psplink.md](docs/psplink.md)

Current build: RE-271 code. Pack `a79b0aa9...5935f` (28,531.3 KiB). Plain
feature-free EBOOT `66ce9869...a5336`.
