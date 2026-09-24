# PSP Lowering

Part of [rendering.md](../rendering.md).

## Submission order

Status: complete.

The build-time merge joins only adjacent primitives with the same material,
preserving `A B A` order (RE-252). The texture cache key covers every
conversion input (RE-253). Order fidelity costs about 113 extra draw calls,
an optimization lead for `P5`.

## GE state cache

Status: complete.

`DrawState` caches GE state between draws. Every draw path that touches GU
state directly calls `DrawState::invalidate_all()` (RE-118, RE-254). The
texture-function cache distinguishes "unknown" from "known `Modulate`"
(RE-300). New out-of-band draw paths must follow the same rule.

## Optimization

Batching, state sorting and further caching wait for `P5` profiling of real
matches, and only on state that has passed its correctness checks
([D-036](../decisions/D-036.md)).
