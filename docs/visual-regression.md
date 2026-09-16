# Visual Regression Methodology (R0.17)

Compatibility pointer. The full deterministic visual-regression methodology
(PPSSPPHeadless setup, capture procedure, test matrix, methodology-validation
evidence) now lives at
[docs/visual-regression/README.md](visual-regression/README.md). Per-scene
capture procedures live under
[docs/visual-regression/scenes/](visual-regression/scenes/) — load only the
specific scene you need, not the whole corpus.

This is the deterministic, repeatable visual-regression procedure `PLAN.md`
R0.17 requires. Screenshots taken ad hoc during individual `RE-`
investigations remain valid evidence for the specific claims they were taken
for, but they are not a substitute for this: a fixed scene that can be
re-captured and diffed automatically as the renderer changes. The automated
capture runner is PPSSPPHeadless; PPSSPP is not physical PSP proof (see
`docs/porting-status.md` "Known gaps").
