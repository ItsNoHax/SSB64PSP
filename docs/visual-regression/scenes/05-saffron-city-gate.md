# Scene 5 — Saffron City animated gate (RE-205)

Part of [docs/visual-regression/README.md](../README.md).

## Fifth deterministic test scene (RE-205)

`PLAN.md` R2's "stage animation works" row needed a scene that isolates
*visibly* animated stage geometry within the frozen tick-240 window — none
of the first four did (Dream Land, scene 1's stage, has no `anim_joints` at
all per RE-051; scenes 2-4 use the object viewer on a single static graph).

`regression_capture_scene5` overrides `stage_index` to 9, Saffron City
(file 112), whose gate RE-142/RE-143 already proved moves under real joint
animation (an independent 836-pixel PPSSPP before/after diff). Unlike
scenes 2-4 it stays in the default `stage_view` — the same whole-stage,
face-on framing scene 1 uses already fits Saffron City's full geometry,
gate included, at the debug viewer's default zoom, so no new camera code
was needed. Reuses the existing tick-240 freeze (extended to recognise this
feature) and HUD suppression.

Finding this candidate used a new persistent example,
`crates/ssb-rom/examples/stage_animation_amplitude.rs`, which ranks every
animated, mesh-bearing, non-billboard stage node by translation/rotation
amplitude over 240 frames — run it to see the full ranking across all 35
animated stages.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene5
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-saffron-city-gate.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-205): built and `ldstart`ed under
PSPLink the same way as the other four scenes (`docs/psplink.md`). This
surfaced a real hardware-only fault — a third `1.0 / payload` speculative-
division FPU trap in `objanim.rs`'s `StageJoint::apply`, the same class
`f111892` already fixed in `figatree.rs`/`matanim.rs` — fixed with the same
`reciprocal_or_one` guard. After the fix: zero exceptions, native capture
matches this golden (upscaled 2x nearest-neighbour) with only the expected
edge-antialiasing band and known overlay regions differing, the same result
shape RE-203 found for the other four scenes.
That physical comparison predates RE-264's corrected directional-light state;
the current golden's 12 changed Mario pixels still require an R2 hardware
re-capture.
