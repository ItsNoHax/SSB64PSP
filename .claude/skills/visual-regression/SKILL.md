---
name: visual-regression
description: Golden screenshots, PPSSPPHeadless, deterministic captures, screenshot comparisons, or regression scenes. Activates when adding, refreshing, or debugging a deterministic rendering-regression scene.
---

# Visual regression

- Scope: PPSSPPHeadless deterministic screenshots, golden comparisons
- Physical PSP: [psp-hardware](../psp-hardware/SKILL.md)
- Original N64: [n64-emulator](../n64-emulator/SKILL.md)

1. Read [docs/visual-regression/README.md](../../../docs/visual-regression/README.md): setup, capture commands, scene table (feature, golden, evidence), known failing goldens, physical-PSP staging
2. Changed golden → explain semantic delta before `rebaseline`: differing-pixel count, what changed, why
   - unexplained change = failure
   - rules: AGENTS.md § Constraints (hashes only, [D-051](../../../docs/decisions/D-051.md)), § Git (rebaselined rows in commit body)
3. Local PNGs: gitignored directory tests/golden/local (plain text, not a tracked path)
   - `tools/golden.sh baseline` recreates them from a passing commit, for difference masks

Tiers:

| Tier | Status |
|---|---|
| PPSSPP software rendering | deterministic golden source |
| PPSSPP hardware backend | separate, non-deterministic |
| Physical-PSP capture | separate, non-deterministic |
| Camera photo of a PSP screen | qualitative; never to the exact comparator |

Worktrees and bisects:
- No `rom/` in a worktree → set `SSB64_ROM` (absolute) or the tablegen build panics; `assets/generated/ssb64.pak` needed (build or symlink main's)
- Golden result = hash equality only. Before batch 95 a `pass` row with a differing hash still matched when the local PNG diff said 0 px: that diff honoured alpha, so RGB changes under alpha 0 (viewer scenes, 99.6 % alpha 0) were invisible. "Varying hash, 0 px" is never nondeterminism
- Bisecting a golden: build each commit's own pack (`romtool pack` at that commit); a newer pack under older code gives a meaningless third hash. Before 74c63e22 also run `tools/gen-*.py --refs refs/ssb-decomp-re` (`--out` for anim-table, computer-scripts)
