# Scene 9 — Metal texgen, camera-rotated (RE-237)

Part of [docs/visual-regression/README.md](../README.md).

## Ninth deterministic test scene (RE-237)

`regression_capture_scene9` selects the same ordinary-texgen graph as scenes
6/7 (file 117, `0x1B10`), but frames it with a real, rotated view matrix
(35 degrees yaw, 20 degrees pitch, `sceGumMatrixMode(View)` via
`gpu.set_view`) instead of the object viewer's usual identity-view/push-the-
object-back placement, and sets `draw_state.texgen_basis` from that same
camera's `right`/`up` — mirroring `stage_view`'s own real-camera branch
(RE-131/RE-214) rather than a second convention. This is `PLAN.md` R2.1/T9's
camera-rotation case: `R2.1`/T3 (RE-227) built `quantize_lookat_basis` but no
existing scene had ever fed it a non-identity basis before this one.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene9
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-metal-texgen-camera-rotated.png ~/ppsspp-test/screenshot.png
```

Deterministic across two headless rebuilds (identical sha256). Visually
distinct from scene 6's own golden (74,848 differing pixels against it) —
the same crystal cluster viewed from a materially different angle, with a
different reflected-facet colour pattern, confirming the camera basis
actually drives the generated texture coordinates rather than being inert.

Physical PSP hardware verification (RE-237): same PSP Slim, 6.61, ARK/
Infinity, PSPLink v3.2.1 hardware as scenes 6–8. `ldstart`ed with `exlist`
empty and `main_thread` alive; native capture (2x-upscaled) diffed against
this golden at 19,304 differing pixels — the smallest noise-floor gap of any
texgen scene measured this way so far. Hashes recorded in RE-237.

RE-237 also ran a `texgen_normal_diagnostic_*` case (`_6`, `[73,-41,99]` raw
`Normal` mode) on this same physical hardware for the first time — previously
these seven cases (`R2.1`/T2, RE-226) had only run under PPSSPP headless.
The real-hardware capture decoded to exactly the predicted `(179, 99)` texel,
matching both the prediction and the headless measurement precisely, not
just to the same order of magnitude.
