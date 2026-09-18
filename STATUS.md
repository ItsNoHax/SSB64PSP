# Current State

Milestone: `R2 — Physical PSP Rendering Validation`
Primary task: complete the remaining physical-hardware matrix
Task state: `IN_PROGRESS`

Current objective: **RE-287 (this session): all 37 new stage goldens from
RE-286 are now physically confirmed on real PSP hardware**, closing
`STATUS.md`'s own previously-recorded required-next-action item (1). One
batched PSPLink pass (PSP Slim, firmware 6.61, ARK/Infinity, PSPLink v3.2.1)
built each of the 37 stage indices in turn
(`SSB64_STAGE_INDEX=<n> cargo psp --release --features
regression_capture_stage_index`), loaded over `host0:`, waited past the
tick-240 deterministic freeze, checked `exlist`, captured a native
480x272 BMP via `scrshot`, then `kill`+`reset` before the next stage (this
project's own established default). All 37 loads were exception-free; every
capture was a full-size, non-blank frame (360-6,040 unique colors, well
above a locked-screen/blank floor); 8 of the 37 were converted and visually
spot-checked pixel-for-pixel against their PPSSPP golden (Beta Dream Land,
Zebes, Test Stage, Mushroom Kingdom, Hyrule Castle, the Break-the-Targets
Mario map, and Race to the Finish), all exact matches modulo the known
PSPLink debug overlay. One tooling bug was found and fixed in the batch
script itself during setup (an absolute-path `scrshot` target silently
failed since `host0:` only maps the served repo root, not the real host
filesystem — caught immediately by the first two test captures being
empty, before it could taint any evidence); no production source changed.
Plain feature-free EBOOT rebuilt after the batch and confirmed byte-
identical to RE-286's own recorded hash
(`2dbce3ee1c718a5ebf374ef65291499d08d1429f49abc000a89dbc97b4818f47`),
confirming no source drift; pack unchanged
(`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`).
**Stage coverage is now 41/41 on both PPSSPP software rendering and
physical PSP hardware** (4 pre-existing + these 37). See RE-287 for full
detail, including the index-to-ROM-file-id mapping (`file = 255 + index`)
and the per-stage capture manifest.

Prior objective: `RE-286` closed software-side stage-golden coverage (all
41/41 stage entries have a deterministic PPSSPP golden), explicitly
deferring physical confirmation of the 37 new ones to this session's
batched pass (now done, see above). `RE-283` (the reopened fighter-texture
quality gate) is fully traced — all nine user-reported items are accounted
for: eight fixed and confirmed on both PPSSPP and physical PSP hardware
(Fox, Mario/Luigi, Samus, Link's boot, Link's shin cuff, Yoshi, Captain
Falcon, Ness), and Pikachu/Kirby traced to no reproducible defect (including
all their non-default costumes, checked clean).

Current blocker(s): PSP-1000's 32 MiB RAM can't use `MEMSIZE=1`, so pack
compatibility there is unresolved rather than assumed — neither the Slim nor
the PSP-3000 tested so far is in that RAM class; no PSP-1000 unit is
available in this environment, so **this item is parked, not pursued, per
explicit instruction.** A second physical unit at the 30-minute
sustained-run duration remains open (only one unit/duration tier tested so
far, RE-284) — deliberately deferred until the game structure grows beyond
the current asset viewer. Kirby's per-copy-ability hat graphs also remain
untested (separate small scene graphs, not reachable from the base body,
and not reachable in normal play either since combat/copy-abilities are not
implemented yet) — not a standing R2 blocker. None of these three items
were in scope for this session and none are newly opened by it. The
USB-permission issue that blocked physical work earlier in this chain's
history was resolved by reconnecting the PSP (new bus address, correct
`0666` device-node permissions) — not a standing blocker, but worth a
replug first if a future session hits "Permission error while opening the
USB device" again before assuming the udev rule itself needs reinstalling.
If `pspsh -e ver` returns "connection refused" despite the PSP showing
`054c:01c9` on USB and PSPLink visibly launched, start
`usbhostfs_pc -v "$PWD"` first — it bridges the USB link `pspsh` actually
connects to; this isn't spelled out in the `psp-hardware` Skill's own
connectivity-check step. **New note (RE-287): `scrshot host0:<path>` only
resolves under the repo root `usbhostfs_pc` was launched from — an
absolute host path after `host0:` silently fails to write while the PSP
still prints a success-looking `frame_addr ... output host0:/...` line.
Always use a path relative to the served root, then move the file out
afterward if it needs to live elsewhere.**

Required next action: The stage-coverage physical matrix (`STATUS.md`'s
former item (1)) is now closed — RE-287 confirms all 41 stages on real
hardware. Remaining R2 coverage work, in rough priority order: (1) a second
physical unit for the long-duration sustained-run check (parity with the
existing 30-minute single-unit sample from RE-284), and (2) lastly (gated
on copy-ability gameplay existing at all) Kirby's per-ability hat graphs.
PSP-1000 confirmation stays parked (no unit available, out of scope per
explicit instruction). Neither remaining item blocks R2.2 (already closed).
Do not start R3 or combat before R2's physical matrix is closed — with the
stage matrix now done, the two remaining items above are the only things
left gating that closure.

Relevant PLAN task: [plans/rendering/R2.md](plans/rendering/R2.md)
Relevant evidence: RE-260, RE-262, RE-264, RE-267, RE-269, RE-270, RE-271,
RE-272, RE-273, RE-274, RE-275, RE-276, RE-277, RE-278, RE-279, RE-280,
RE-281, RE-282, RE-283, RE-284, RE-285, RE-286, RE-287 (see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–287). Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-287 (this session) is hardware-evidence-only — no
production source changed. On top of RE-286's 37-stage-golden source
changes (`regression_capture_stage_index` Cargo feature,
`SSB64_STAGE_INDEX` wiring in `psp/src/main.rs`) and RE-283's eighteenth
follow-up fix (Ness shoulder-strap). Plain feature-free EBOOT rebuilt after
this session's batch and reconfirmed byte-identical to RE-286's own hash
(no drift):
`2dbce3ee1c718a5ebf374ef65291499d08d1429f49abc000a89dbc97b4818f47`. Pack
unchanged this session (no asset-pipeline code touched):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`
(28,572.0 KiB).
