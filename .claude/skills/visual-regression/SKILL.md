---
name: visual-regression
description: Golden screenshots, PPSSPPHeadless, deterministic captures, screenshot comparisons, or regression scenes. Activates when adding, refreshing, or debugging a deterministic rendering-regression scene.
---

# Visual regression

Use this skill (PPSSPPHeadless) for visual/rendering testing — deterministic
screenshots and golden comparisons. For real-hardware validation and crash
capture use [psp-hardware](../psp-hardware/SKILL.md) (PSPLink) instead; for
live original-N64 behavior use [n64-emulator](../n64-emulator/SKILL.md)
(headless Mupen64Plus) instead.

1. Load general methodology first:
   [docs/visual-regression/README.md](../../docs/visual-regression/README.md)
   — PPSSPPHeadless setup, golden rules (what a golden does/doesn't prove),
   capture procedure, test matrix, physical-PSP distinction.
2. Load only the specific scene file you need from
   `docs/visual-regression/scenes/` — do not load every scene by default.
3. If a scene's golden changes, explain the semantic delta (differing-pixel
   count, what changed and why) before accepting the refresh — an
   unexplained golden change is a red flag, not a pass.
4. A camera photograph of a physical PSP screen is qualitative evidence only
   and must not be passed to the exact PPSSPP pixel comparator.
5. PPSSPP software rendering is the deterministic golden source; PPSSPP
   hardware-backend and physical-PSP captures are separate, non-deterministic
   evidence tiers — do not conflate a PPSSPP-software pass with physical
   confirmation.
