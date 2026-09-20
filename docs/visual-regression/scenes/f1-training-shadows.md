# F1 Training fighter shadows

`regression_capture_shadows` freezes Training at tick 22. The stationary
dummy is grounded while the player is rising from the scripted C-Up jump, so
the capture exercises two independent shadows and airborne floor projection.
Dream Land's raised platform is also a representative separate supporting
floor. The source geometry/material are independently traced in RE-302; the
golden guards the PSP submission and GE state from regressions.

Run:

```bash
tools/run-ppsspp-headless.sh --crate psp-game --feature regression_capture_shadows
tools/compare-screenshot.sh tests/golden/f1-training-shadows.png \
  /home/alberto/ppsspp-headless-test/screenshot.png
```
