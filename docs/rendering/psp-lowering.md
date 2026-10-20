# PSP Lowering

Part of [rendering.md](../rendering.md).

## Submission order

- Build-time merge joins only adjacent primitives with the same material; `A B A` order preserved (RE-252).
- Texture cache key covers every conversion input (RE-253).
- Order fidelity costs extra draw calls; batching is an optimization lead (`PLAN.md`, PSP performance).

## Guard-band clipping

- GE drops a whole triangle when a vertex in front of the near plane projects past its 4096-pixel range; RSP clips.
- Such triangles clipped on CPU against |x|,|y| <= 2w and z >= -w (RE-519).

## Shade-only combiners

- Colour formula with no texel (SHADE times constants) → texture whitened by romtool, alpha kept (RE-520).
  - why: GE Modulate always multiplies the texel colour.
- Unlit untextured prim-alpha surfaces keep their SETPRIMCOLOR alpha (RE-521).

## GE state cache

- `DrawState` caches GE state between draws.
- Every draw path touching GU state directly calls `DrawState::invalidate_all()` (RE-118, RE-254).
  - New out-of-band draw paths must too.
- Texture-function cache distinguishes "unknown" from "known `Modulate`" (RE-300).

## Optimization

- Batching, state sorting, further caching: wait for profiling of real matches on a PSP (`PLAN.md` milestone `MS8`).
- Only on state that passed its correctness checks ([D-036](../decisions/D-036.md)).
