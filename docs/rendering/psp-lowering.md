# PSP Lowering

Part of [docs/rendering.md](../rendering.md). Describes the current model; see `docs/evidence/re/` for how it was established.

## Renderer evolution

Per plan §32:

* **Renderer 0** — triangle testbed. ✅ Done; runs at a locked 60 FPS.
* **Renderer 1** — SSB stage geometry. ✅ Done. Stages render from the pack
  with their collision polylines overlaid (`docs/images/m4-stage-collision.png`).
* **Renderer 2** — complete static materials/textures. ✅ Textures, CLUTs,
  per-node matrices, runtime fighter lighting, costume-light colours and
  recovered `MObj` state draw on device; R2.2's correction matrix passes.
* **Renderer 3** — transparency, framebuffer effects and particles are
  complete for R1 renderer scope. Shadows and real UI remain unimplemented;
  the developer overlay still uses `sceGuDebugFlush` (RE-014).
* **Renderer 4** — batching, state sorting, caching. **Not before the game is
  visibly running**, and not before the state being merged/sorted/cached has
  passed its own correctness gate (D-036, `PLAN.md` R0.16/R2.2). RE-252's
  build-time merge is now adjacent-only and preserves submission order.
  Further batching/state sorting remains an R3 optimization after the
  physical R2 matrix closes.

`PLAN.md` R0.18 tracks a systematic comparison against `sf64-psp` and
`oot-PSP` (both PSP targets, so their `sceGu` usage is directly comparable)
beyond the ad hoc BattleShip cross-references already scattered through this
document and `docs/reverse-engineering.md`. Treat all three as technical
references, not authorities, per D-037.


### Primitive submission order

Status: COMPLETE

RE-122 fixed texture-state loss; RE-252 replaces global material regrouping with adjacent-run merging, preserving `A B A` order. RE-253 independently widens the texture cache key to every conversion input

**Remaining work:** The measured +113 draw calls are an R3 optimization lead; fidelity-preserving order is the baseline


### PSP GE state cache

Status: COMPLETE

RE-118 fixed the overlay bypass; RE-254 inventories every direct GU mutation and adds `DrawState::invalidate_all()` at all out-of-band draw paths. RE-261's matrix found no leak

**Remaining work:** None for renderer correctness; future draw paths must use the same invalidation rule
