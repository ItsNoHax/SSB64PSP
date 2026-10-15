# PSP Lowering

Part of [rendering.md](../rendering.md).

## Submission order

Status: complete.

The build-time merge joins only adjacent primitives with the same material,
preserving `A B A` order (RE-252). The texture cache key covers every
conversion input (RE-253). Order fidelity costs about 113 extra draw calls,
an optimization lead (`PLAN.md`, PSP performance).

## Guard-band clipping

Status: complete for movie scenes.

The GE drops a whole triangle when a vertex in front of the near plane
projects past its 4096-pixel range; the RSP clips. Triangles that need it
are clipped on the CPU against |x|,|y| <= 2w and z >= -w (RE-519).

## Shade-only combiners

Status: complete.

A colour formula with no texel (SHADE times constants) lowers to a texture
whitened by romtool with its alpha kept, since the GE's Modulate always
multiplies the texel colour (RE-520). Unlit untextured prim-alpha surfaces
keep their SETPRIMCOLOR alpha (RE-521).

## GE state cache

Status: complete.

`DrawState` caches GE state between draws. Every draw path that touches GU
state directly calls `DrawState::invalidate_all()` (RE-118, RE-254). The
texture-function cache distinguishes "unknown" from "known `Modulate`"
(RE-300). New out-of-band draw paths must follow the same rule.

## Optimization

Batching, state sorting and further caching wait for profiling of real
matches on a PSP (`PLAN.md` milestone `MS8`), and only on state that has passed its correctness checks
([D-036](../decisions/D-036.md)).
